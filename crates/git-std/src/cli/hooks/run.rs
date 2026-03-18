use std::process::Command;

use yansi::Paint;

use standard_githooks::{HookCommand, HookMode, Prefix, default_mode, substitute_msg};

use crate::ui;

use super::read_and_parse_hooks;

/// The result of executing a single hook command.
struct CommandResult {
    /// The exit code (0 = success).
    exit_code: Option<i32>,
    /// Whether this command was advisory.
    advisory: bool,
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
/// Prints a pending indicator before spawning the command. On a TTY the
/// pending line is overwritten in place with the final result; on a
/// non-TTY the result is printed on a new line below it.
///
/// Returns the [`CommandResult`] and whether the command failed (non-advisory).
fn execute_and_print(
    cmd: &HookCommand,
    msg_path: &str,
    index: usize,
    total: usize,
) -> (CommandResult, bool) {
    let command_text = substitute_msg(&cmd.command, msg_path);
    let is_advisory = cmd.prefix == Prefix::Advisory;
    let display = format_display(&command_text, cmd.glob.as_deref());

    // Show the pending indicator before spawning.
    ui::pending(index, total, &display);

    // Execute via sh -c
    let status = Command::new("sh").arg("-c").arg(&command_text).status();

    let exit_code = match status {
        Ok(s) => s.code(),
        Err(_) => Some(127),
    };

    let success = exit_code == Some(0);

    // On a TTY, move the cursor back to the start of the pending line and
    // clear it so the result line overwrites it cleanly.
    if ui::is_tty() && yansi::is_enabled() {
        eprint!("\r\x1b[K");
    }

    // Print the result line
    if success {
        ui::result_line(&format!("{} {}", ui::pass(), display));
    } else if is_advisory {
        let info = match exit_code {
            Some(code) => format!("(advisory, exit {code})"),
            None => "(advisory, killed)".to_string(),
        };
        ui::result_line(&format!("{} {} {}", ui::warn(), display, info.yellow()));
    } else {
        let info = match exit_code {
            Some(code) => format!("(exit {code})"),
            None => "(killed)".to_string(),
        };
        ui::result_line(&format!("{} {} {}", ui::fail(), display, info.red()));
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
        ui::result_line(&format!(
            "{} hooks skipped (GIT_STD_SKIP_HOOKS)",
            ui::warn()
        ));
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
    let total = commands.len();
    let mut index: usize = 0;

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

        let (result, failed) = execute_and_print(cmd, msg_path, index, total);
        index += 1;
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
                ui::info(&format!(
                    "{} remaining {} skipped (fail-fast)",
                    remaining,
                    if remaining == 1 {
                        "command"
                    } else {
                        "commands"
                    },
                ));
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
        ui::info(&parts.join(", "));
    }

    if has_failure { 1 } else { 0 }
}
