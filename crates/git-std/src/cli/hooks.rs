use std::path::Path;
use std::process::Command;

use yansi::Paint;

use standard_githooks::Prefix;
use standard_githooks::run::{HookMode, default_mode, substitute_msg};

/// The result of executing a single hook command.
struct CommandResult {
    /// The exit code (0 = success).
    exit_code: Option<i32>,
    /// Whether this command was advisory.
    advisory: bool,
}

/// Run the `hooks run <hook>` subcommand. Returns the process exit code.
///
/// Reads `.githooks/<hook>.hooks`, parses commands, executes them
/// according to the hook's default mode and per-command prefix
/// overrides, and prints a summary.
pub fn run(hook: &str, args: &[String]) -> i32 {
    let hooks_file = format!(".githooks/{hook}.hooks");
    let path = Path::new(&hooks_file);

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: cannot read {hooks_file}: {e}");
            return 2;
        }
    };

    let commands = standard_githooks::parse(&content);
    if commands.is_empty() {
        return 0;
    }

    let mode = default_mode(hook);

    // Determine the msg_path from args (first argument after --)
    let msg_path = args.first().map(|s| s.as_str()).unwrap_or("");

    // Collect file list for glob filtering (lazy — only fetched if needed).
    let file_list: Option<Vec<String>> = if commands.iter().any(|c| c.glob.is_some()) {
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

        // Apply {msg} substitution
        let command_text = substitute_msg(&cmd.command, msg_path);

        // Determine the effective mode for this command
        let effective_mode = match cmd.prefix {
            Prefix::FailFast => HookMode::FailFast,
            Prefix::Advisory => HookMode::Collect, // advisory always runs
            Prefix::Default => mode,
        };
        let is_advisory = cmd.prefix == Prefix::Advisory;

        // Execute via sh -c
        let status = Command::new("sh").arg("-c").arg(&command_text).status();

        let exit_code = match status {
            Ok(s) => s.code(),
            Err(_) => Some(127),
        };

        let success = exit_code == Some(0);

        // Build display text (show glob if present)
        let display = if let Some(ref glob) = cmd.glob {
            format!("{command_text} ({glob})")
        } else {
            command_text.clone()
        };

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
            has_failure = true;
        }

        results.push(CommandResult {
            exit_code,
            advisory: is_advisory,
        });

        // In fail-fast mode, abort on first non-advisory failure
        if !success && !is_advisory && effective_mode == HookMode::FailFast {
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
