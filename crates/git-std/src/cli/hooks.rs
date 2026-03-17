use std::process::Command;

use inquire::MultiSelect;
use yansi::Paint;

use standard_githooks::{
    HookCommand, HookMode, KNOWN_HOOKS, Prefix, default_mode, generate_hooks_template,
    generate_shim, substitute_msg,
};

use crate::ui;

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
            ui::error(&format!("cannot read {hooks_file}: {e}"));
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
        eprintln!("{INDENT}{} {}", ui::pass(), display, INDENT = ui::INDENT);
    } else if is_advisory {
        let info = match exit_code {
            Some(code) => format!("(advisory, exit {code})"),
            None => "(advisory, killed)".to_string(),
        };
        eprintln!(
            "{INDENT}{} {} {}",
            ui::warn(),
            display,
            info.yellow(),
            INDENT = ui::INDENT
        );
    } else {
        let info = match exit_code {
            Some(code) => format!("(exit {code})"),
            None => "(killed)".to_string(),
        };
        eprintln!(
            "{INDENT}{} {} {}",
            ui::fail(),
            display,
            info.red(),
            INDENT = ui::INDENT
        );
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
        eprintln!(
            "{INDENT}{} hooks skipped (GIT_STD_SKIP_HOOKS)",
            ui::warn(),
            INDENT = ui::INDENT
        );
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

    // Collect file list for glob filtering (lazy -- only fetched if needed).
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
                ui::blank();
                eprintln!(
                    "{INDENT}{} remaining {} skipped (fail-fast)",
                    remaining,
                    if remaining == 1 {
                        "command"
                    } else {
                        "commands"
                    },
                    INDENT = ui::INDENT,
                );
            }
            ui::blank();
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
        ui::blank();
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
        eprintln!("{INDENT}{}", parts.join(", "), INDENT = ui::INDENT);
    }

    if has_failure { 1 } else { 0 }
}

/// Returns true if a hook's shim is currently active (named exactly as the hook).
fn is_enabled(hooks_dir: &std::path::Path, hook_name: &str) -> bool {
    hooks_dir.join(hook_name).exists()
}

/// Run the `hooks install` subcommand. Returns the process exit code.
///
/// - Sets core.hooksPath
/// - Creates .githooks/ directory
/// - Writes .hooks template for every known hook (skips existing)
/// - Prompts which hooks to enable (interactive multi-select)
/// - Writes active shims for selected hooks, .off shims for the rest
pub fn install() -> i32 {
    // 1. Set core.hooksPath
    let status = Command::new("git")
        .args(["config", "core.hooksPath", ".githooks"])
        .status();

    match status {
        Ok(s) if s.success() => {
            eprintln!(
                "{INDENT}{}  core.hooksPath \u{2192} .githooks",
                ui::pass(),
                INDENT = ui::INDENT,
            );
        }
        _ => {
            ui::error("failed to set core.hooksPath");
            eprintln!("  hint: ensure you are inside a git repository and have write access");
            return 1;
        }
    }

    // 2. Ensure .githooks/ exists
    let hooks_dir = std::path::Path::new(".githooks");
    if !hooks_dir.exists()
        && let Err(e) = std::fs::create_dir_all(hooks_dir)
    {
        ui::error(&format!("cannot create .githooks/: {e}"));
        return 1;
    }

    // 3. Write .hooks templates for every known hook (skip if already exists)
    for hook_name in KNOWN_HOOKS {
        let template_path = hooks_dir.join(format!("{hook_name}.hooks"));
        if !template_path.exists() {
            let content = generate_hooks_template(hook_name);
            if let Err(e) = std::fs::write(&template_path, &content) {
                ui::error(&format!("cannot write {}: {e}", template_path.display()));
                return 1;
            }
        }
    }

    // 4. Determine which hooks to enable — via env var (for tests/CI) or interactive prompt
    let default_enabled = ["pre-commit", "commit-msg"];

    let env_enable = std::env::var("GIT_STD_HOOKS_ENABLE").ok();
    let selected: Vec<&str> = if let Some(ref val) = env_enable {
        // Comma-separated list of hook names, or "all" or "none"
        match val.to_lowercase().as_str() {
            "all" => KNOWN_HOOKS.to_vec(),
            "none" => vec![],
            _ => val
                .split(',')
                .map(|s| s.trim())
                .filter(|s| KNOWN_HOOKS.contains(s))
                .collect(),
        }
    } else {
        let options: Vec<&str> = KNOWN_HOOKS.to_vec();
        match MultiSelect::new("Which hooks do you want to enable?", options)
            .with_default(
                &KNOWN_HOOKS
                    .iter()
                    .enumerate()
                    .filter(|(_, h)| default_enabled.contains(h))
                    .map(|(i, _)| i)
                    .collect::<Vec<_>>(),
            )
            .prompt()
        {
            Ok(s) => s,
            Err(_) => {
                ui::error("install cancelled");
                return 1;
            }
        }
    };

    ui::blank();

    // 5. Write shims — active for selected, .off for the rest
    for hook_name in KNOWN_HOOKS {
        let shim_content = generate_shim(hook_name);
        let enabled = selected.contains(hook_name);

        let active_path = hooks_dir.join(hook_name);
        let off_path = hooks_dir.join(format!("{hook_name}.off"));

        // Remove stale counterpart
        if enabled {
            let _ = std::fs::remove_file(&off_path);
        } else {
            let _ = std::fs::remove_file(&active_path);
        }

        let shim_path = if enabled { &active_path } else { &off_path };

        if let Err(e) = std::fs::write(shim_path, &shim_content) {
            ui::error(&format!("cannot write {}: {e}", shim_path.display()));
            return 1;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            if let Err(e) = std::fs::set_permissions(shim_path, perms) {
                ui::error(&format!(
                    "cannot set permissions on {}: {e}",
                    shim_path.display()
                ));
                return 1;
            }
        }

        let status_label = if enabled {
            "enabled ".green().to_string()
        } else {
            "disabled".dim().to_string()
        };

        eprintln!(
            "{INDENT}{}  {hook_name:<22} {status_label}",
            ui::pass(),
            INDENT = ui::INDENT,
        );
    }

    0
}

/// Run the `hooks enable <hook>` subcommand. Returns the process exit code.
pub fn enable(hook_name: &str) -> i32 {
    if !KNOWN_HOOKS.contains(&hook_name) {
        ui::error(&format!(
            "unknown hook '{hook_name}' — known hooks: {}",
            KNOWN_HOOKS.join(", ")
        ));
        return 1;
    }

    let hooks_dir = std::path::Path::new(".githooks");
    let active_path = hooks_dir.join(hook_name);
    let off_path = hooks_dir.join(format!("{hook_name}.off"));

    if active_path.exists() {
        eprintln!(
            "{INDENT}{} {hook_name} is already enabled",
            ui::warn(),
            INDENT = ui::INDENT
        );
        return 0;
    }

    if !off_path.exists() {
        ui::error(&format!(
            "{hook_name}.off not found — run 'git std hooks install' first"
        ));
        return 1;
    }

    if let Err(e) = std::fs::rename(&off_path, &active_path) {
        ui::error(&format!("cannot enable {hook_name}: {e}"));
        return 1;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        let _ = std::fs::set_permissions(&active_path, perms);
    }

    eprintln!(
        "{INDENT}{}  {hook_name} enabled",
        ui::pass(),
        INDENT = ui::INDENT
    );
    0
}

/// Run the `hooks disable <hook>` subcommand. Returns the process exit code.
pub fn disable(hook_name: &str) -> i32 {
    if !KNOWN_HOOKS.contains(&hook_name) {
        ui::error(&format!(
            "unknown hook '{hook_name}' — known hooks: {}",
            KNOWN_HOOKS.join(", ")
        ));
        return 1;
    }

    let hooks_dir = std::path::Path::new(".githooks");
    let active_path = hooks_dir.join(hook_name);
    let off_path = hooks_dir.join(format!("{hook_name}.off"));

    if off_path.exists() {
        eprintln!(
            "{INDENT}{} {hook_name} is already disabled",
            ui::warn(),
            INDENT = ui::INDENT
        );
        return 0;
    }

    if !active_path.exists() {
        ui::error(&format!(
            "{hook_name} not found — run 'git std hooks install' first"
        ));
        return 1;
    }

    if let Err(e) = std::fs::rename(&active_path, &off_path) {
        ui::error(&format!("cannot disable {hook_name}: {e}"));
        return 1;
    }

    eprintln!(
        "{INDENT}{}  {hook_name} disabled",
        ui::pass(),
        INDENT = ui::INDENT
    );
    0
}

/// Run the `hooks list` subcommand. Returns the process exit code.
///
/// Shows all known hooks with enabled/disabled status and their commands.
pub fn list() -> i32 {
    let hooks_dir = std::path::Path::new(".githooks");

    if !hooks_dir.exists() {
        eprintln!(
            "{INDENT}no hooks installed — run 'git std hooks install'",
            INDENT = ui::INDENT
        );
        return 0;
    }

    for (i, hook_name) in KNOWN_HOOKS.iter().enumerate() {
        if i > 0 {
            println!();
        }

        let enabled = is_enabled(hooks_dir, hook_name);
        let status_label = if enabled {
            "enabled".green().to_string()
        } else {
            "disabled".dim().to_string()
        };

        let template_path = hooks_dir.join(format!("{hook_name}.hooks"));
        let commands: Vec<HookCommand> = if template_path.exists() {
            read_and_parse_hooks(hook_name).unwrap_or_default()
        } else {
            vec![]
        };

        let mode = default_mode(hook_name);
        let mode_label = match mode {
            HookMode::Collect => "collect",
            HookMode::FailFast => "fail-fast",
        };

        println!(
            "{INDENT}{hook_name} ({mode_label}) [{status_label}]:",
            INDENT = ui::INDENT
        );

        if commands.is_empty() {
            println!("{INDENT}  (no commands)", INDENT = ui::INDENT);
        } else {
            for cmd in &commands {
                let prefix_char = match cmd.prefix {
                    Prefix::FailFast => "!",
                    Prefix::Advisory => "?",
                    Prefix::Default => " ",
                };

                let display = if let Some(ref glob) = cmd.glob {
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

                println!("{INDENT}{display}", INDENT = ui::INDENT);
            }
        }
    }

    0
}
