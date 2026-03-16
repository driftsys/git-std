use std::process::Command;

use yansi::Paint;

use standard_githooks::{
    HookCommand, HookMode, Prefix, default_mode, generate_shim, substitute_msg,
};

/// The result of executing a single hook command.
struct CommandResult {
    /// The exit code (0 = success).
    exit_code: Option<i32>,
    /// Whether this command was advisory.
    advisory: bool,
}

/// Read and parse the `.githooks/<hook>.hooks` file.
///
/// Returns `Ok(commands)` on success, or `Err(exit_code)` if the file
/// cannot be read.
fn read_and_parse_hooks(hook_name: &str) -> Result<Vec<HookCommand>, i32> {
    let hooks_file = format!(".githooks/{hook_name}.hooks");
    let content = match std::fs::read_to_string(&hooks_file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: cannot read {hooks_file}: {e}");
            return Err(2);
        }
    };
    Ok(standard_githooks::parse(&content))
}

/// Fetch the file list for glob filtering.
///
/// For `pre-commit`, returns staged files; for other hooks, returns all
/// tracked files. Only called when at least one command has a glob pattern.
fn fetch_file_list(hook: &str) -> Option<Vec<String>> {
    let output = if hook == "pre-commit" {
        Command::new("git")
            .args(["diff", "--cached", "--name-only", "--diff-filter=ACMR"])
            .output()
    } else {
        Command::new("git").args(["ls-files"]).output()
    };
    match output {
        Ok(o) => Some(
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(String::from)
                .collect(),
        ),
        Err(_) => None,
    }
}

/// Format a command's display text, appending the glob pattern if present.
fn format_display(command_text: &str, glob: Option<&str>) -> String {
    match glob {
        Some(g) => format!("{command_text} ({g})"),
        None => command_text.to_string(),
    }
}

/// Execute a single hook command and print its result line.
///
/// Returns the [`CommandResult`] and whether the command failed (non-advisory).
fn execute_and_print(cmd: &HookCommand, msg_path: &str) -> (CommandResult, bool) {
    let command_text = substitute_msg(&cmd.command, msg_path);
    let is_advisory = cmd.prefix == Prefix::Advisory;

    // Execute via sh -c
    let status = Command::new("sh").arg("-c").arg(&command_text).status();

    let exit_code = match status {
        Ok(s) => s.code(),
        Err(_) => Some(127),
    };

    let success = exit_code == Some(0);
    let display = format_display(&command_text, cmd.glob.as_deref());

    // Print the result line
    if success {
        eprintln!("  {} {}", "\u{2713}".green(), display);
    } else if is_advisory {
        let info = match exit_code {
            Some(code) => format!("(advisory, exit {code})"),
            None => "(advisory, killed)".to_string(),
        };
        eprintln!("  {} {} {}", "\u{26a0}".yellow(), display, info.yellow());
    } else {
        let info = match exit_code {
            Some(code) => format!("(exit {code})"),
            None => "(killed)".to_string(),
        };
        eprintln!("  {} {} {}", "\u{2717}".red(), display, info.red());
    }

    let failed = !success && !is_advisory;

    (
        CommandResult {
            exit_code,
            advisory: is_advisory,
        },
        failed,
    )
}

/// Run the `hooks run <hook>` subcommand. Returns the process exit code.
///
/// Reads `.githooks/<hook>.hooks`, parses commands, executes them
/// according to the hook's default mode and per-command prefix
/// overrides, and prints a summary.
pub fn run(hook: &str, args: &[String]) -> i32 {
    // Allow skipping all hook execution via environment variable.
    if let Ok(val) = std::env::var("GIT_STD_SKIP_HOOKS")
        && (val == "1" || val.eq_ignore_ascii_case("true"))
    {
        eprintln!("  \u{26a0} hooks skipped (GIT_STD_SKIP_HOOKS)");
        return 0;
    }

    let commands = match read_and_parse_hooks(hook) {
        Ok(c) => c,
        Err(code) => return code,
    };
    if commands.is_empty() {
        return 0;
    }

    let mode = default_mode(hook);

    // Determine the msg_path from args (first argument after --)
    let msg_path = args.first().map(|s| s.as_str()).unwrap_or("");

    // Collect file list for glob filtering (lazy — only fetched if needed).
    let file_list: Option<Vec<String>> = if commands.iter().any(|c| c.glob.is_some()) {
        fetch_file_list(hook)
    } else {
        None
    };

    let mut results: Vec<CommandResult> = Vec::new();
    let mut has_failure = false;

    for cmd in &commands {
        // Glob filtering: skip command if glob doesn't match any files.
        if let Some(ref glob) = cmd.glob
            && let Some(ref files) = file_list
        {
            let refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
            if !standard_githooks::matches_any(glob, &refs) {
                continue;
            }
        }

        // Determine the effective mode for this command
        let effective_mode = match cmd.prefix {
            Prefix::FailFast => HookMode::FailFast,
            Prefix::Advisory => HookMode::Collect, // advisory always runs
            Prefix::Default => mode,
        };

        let (result, failed) = execute_and_print(cmd, msg_path);
        if failed {
            has_failure = true;
        }

        results.push(result);

        // In fail-fast mode, abort on first non-advisory failure
        if failed && effective_mode == HookMode::FailFast {
            // Print remaining commands as skipped
            let remaining = commands.len() - results.len();
            if remaining > 0 {
                eprintln!();
                eprintln!(
                    "  {} remaining {} skipped (fail-fast)",
                    remaining,
                    if remaining == 1 {
                        "command"
                    } else {
                        "commands"
                    }
                );
            }
            eprintln!();
            return 1;
        }
    }

    // Print summary
    let failed_count = results
        .iter()
        .filter(|r| r.exit_code != Some(0) && !r.advisory)
        .count();
    let advisory_count = results
        .iter()
        .filter(|r| r.exit_code != Some(0) && r.advisory)
        .count();

    if failed_count > 0 || advisory_count > 0 {
        eprintln!();
        let mut parts = Vec::new();
        if failed_count > 0 {
            parts.push(format!("{failed_count} failed"));
        }
        if advisory_count > 0 {
            parts.push(format!(
                "{advisory_count} advisory {}",
                if advisory_count == 1 {
                    "warning"
                } else {
                    "warnings"
                }
            ));
        }
        eprintln!("  {}", parts.join(", "));
    }

    if has_failure { 1 } else { 0 }
}

/// Scan `.githooks/` for `*.hooks` files and return sorted hook names.
fn discover_hooks() -> Vec<String> {
    let hooks_dir = std::path::Path::new(".githooks");
    let entries = match std::fs::read_dir(hooks_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut hooks: Vec<String> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();
            let hook_name = name.strip_suffix(".hooks")?;
            Some(hook_name.to_string())
        })
        .collect();

    hooks.sort();
    hooks
}

/// Run the `hooks install` subcommand. Returns the process exit code.
///
/// Configures `core.hooksPath`, creates the `.githooks/` directory,
/// and writes shim scripts for each `.hooks` file found.
pub fn install() -> i32 {
    // 1. Set core.hooksPath
    let status = Command::new("git")
        .args(["config", "core.hooksPath", ".githooks"])
        .status();

    match status {
        Ok(s) if s.success() => {
            eprintln!(
                "  {}  core.hooksPath \u{2192} .githooks",
                "\u{2713}".green()
            );
        }
        _ => {
            eprintln!("error: failed to set core.hooksPath");
            return 1;
        }
    }

    // 2. Ensure .githooks/ exists
    let hooks_dir = std::path::Path::new(".githooks");
    if !hooks_dir.exists()
        && let Err(e) = std::fs::create_dir_all(hooks_dir)
    {
        eprintln!("error: cannot create .githooks/: {e}");
        return 1;
    }

    // 3. Write shims for each .hooks file
    let hooks = discover_hooks();

    if hooks.is_empty() {
        eprintln!();
        eprintln!("  no .hooks files found in .githooks/");
        return 0;
    }

    for hook_name in &hooks {
        let shim_content = generate_shim(hook_name);
        let shim_path = hooks_dir.join(hook_name);

        if let Err(e) = std::fs::write(&shim_path, &shim_content) {
            eprintln!("error: cannot write {}: {e}", shim_path.display());
            return 1;
        }

        // Set executable permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            if let Err(e) = std::fs::set_permissions(&shim_path, perms) {
                eprintln!(
                    "error: cannot set permissions on {}: {e}",
                    shim_path.display()
                );
                return 1;
            }
        }

        eprintln!(
            "  {}  .githooks/{:<18}\u{2192} git std hooks run {hook_name}",
            "\u{2713}".green(),
            hook_name,
        );
    }

    0
}

/// Run the `hooks list` subcommand. Returns the process exit code.
///
/// Reads all `.githooks/*.hooks` files, parses them, and prints
/// a human-readable summary of each hook and its commands.
pub fn list() -> i32 {
    let hooks = discover_hooks();

    if hooks.is_empty() {
        eprintln!("  no hooks configured");
        return 0;
    }

    for (i, hook_name) in hooks.iter().enumerate() {
        if i > 0 {
            println!();
        }

        let commands = match read_and_parse_hooks(hook_name) {
            Ok(c) => c,
            Err(code) => return code,
        };

        let mode = default_mode(hook_name);
        let mode_label = match mode {
            HookMode::Collect => "collect",
            HookMode::FailFast => "fail-fast",
        };

        println!("  {hook_name} ({mode_label} mode):");

        for cmd in &commands {
            let prefix_char = match cmd.prefix {
                Prefix::FailFast => "!",
                Prefix::Advisory => "?",
                Prefix::Default => " ",
            };

            let display = if let Some(ref glob) = cmd.glob {
                // Right-align glob after command with padding
                let cmd_part = format!("  {prefix_char} {}", cmd.command);
                let total_width = 50;
                let padding = if cmd_part.len() < total_width {
                    total_width - cmd_part.len()
                } else {
                    4
                };
                format!(
                    "  {prefix_char} {}{:width$}{glob}",
                    cmd.command,
                    "",
                    width = padding
                )
            } else {
                format!("  {prefix_char} {}", cmd.command)
            };

            println!("  {display}");
        }
    }

    0
}
