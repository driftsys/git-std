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

/// Fetch the list of staged file paths (for pre-commit stash dance and $@ passing).
///
/// Returns all staged files (added, copied, modified, renamed) relative to the
/// working tree root. Returns an empty vec on failure.
fn fetch_staged_files() -> Vec<String> {
    match Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=ACMR"])
        .output()
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(String::from)
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Fetch the list of unstaged (working-tree-modified) file paths.
///
/// Returns file paths that differ between index and working tree.
fn fetch_unstaged_files() -> Vec<String> {
    match Command::new("git").args(["diff", "--name-only"]).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(String::from)
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Run `git stash push --quiet`. Returns `true` if the stash was created
/// successfully (something was stashed), `false` otherwise (nothing to stash
/// or git error).
fn stash_push() -> bool {
    Command::new("git")
        .args(["stash", "push", "--quiet"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run `git stash apply --quiet`. Warns on failure.
fn stash_apply() {
    let ok = Command::new("git")
        .args(["stash", "apply", "--quiet"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        ui::warning("git stash apply failed — working tree may be inconsistent");
    }
}

/// Run `git stash drop --quiet`. Warns on failure.
fn stash_drop() {
    let ok = Command::new("git")
        .args(["stash", "drop", "--quiet"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        ui::warning("git stash drop failed — stash entry may remain");
    }
}

/// Re-stage the given files after a formatter has run.
///
/// Runs `git add -- <files>` to pick up any formatting changes.
fn restage_files(files: &[String]) {
    if files.is_empty() {
        return;
    }
    let mut cmd = Command::new("git");
    cmd.arg("add").arg("--");
    for f in files {
        cmd.arg(f);
    }
    if let Err(e) = cmd.status() {
        ui::warning(&format!("git add failed after fix-mode formatting: {e}"));
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
/// `staged_files` is passed as `$@` to the shell command (positional
/// parameters). For `pre-commit` this is the list of staged files; for
/// other hooks it is an empty slice.
///
/// Returns the [`CommandResult`] and whether the command failed (non-advisory).
fn execute_and_print(
    cmd: &HookCommand,
    msg_path: &str,
    staged_files: &[String],
    index: usize,
    total: usize,
) -> (CommandResult, bool) {
    let command_text = substitute_msg(&cmd.command, msg_path);
    let is_advisory = cmd.prefix == Prefix::Advisory;
    let display = format_display(&command_text, cmd.glob.as_deref());

    // Show the pending indicator before spawning.
    ui::pending(index, total, &display);

    // Execute via sh -c <script> _ <arg1> <arg2>...
    // The `_` becomes $0 (conventional placeholder), staged_files become $@.
    let status = Command::new("sh")
        .arg("-c")
        .arg(&command_text)
        .arg("_") // $0
        .args(staged_files) // $@
        .status();

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

    // For pre-commit: fetch staged files for $@ passing and stash dance.
    let staged_files: Vec<String> = if hook == "pre-commit" {
        fetch_staged_files()
    } else {
        Vec::new()
    };

    // Determine whether we need the stash dance.
    // Only applies for pre-commit hooks that contain at least one `~` command.
    let has_fix_commands = commands.iter().any(|c| c.prefix == Prefix::Fix);
    let use_stash_dance = hook == "pre-commit" && has_fix_commands;

    // For non-pre-commit hooks, warn about `~` commands and treat them as `!`.
    if hook != "pre-commit" && has_fix_commands {
        ui::warning("~ prefix is only supported in pre-commit — treating as !");
    }

    // Perform the stash dance if needed.
    // stash_active tracks whether a stash entry was actually created.
    let stash_active = if use_stash_dance {
        stash_push()
        // If stash_push returns false (nothing to stash or error), we skip
        // the stash dance but still run commands normally.
    } else {
        false
    };

    if use_stash_dance && stash_active {
        // Restore staged + unstaged content to the working tree so formatters
        // can see the full context.
        stash_apply();
    }

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

        // Resolve `~` prefix:
        // - In pre-commit with stash dance: treat as FailFast for pass/fail logic.
        // - In other hooks: already warned above, treat as FailFast.
        let effective_prefix = if cmd.prefix == Prefix::Fix {
            Prefix::FailFast
        } else {
            cmd.prefix
        };

        // Determine the effective mode for this command.
        let effective_mode = match effective_prefix {
            Prefix::FailFast => HookMode::FailFast,
            Prefix::Advisory => HookMode::Collect, // advisory always runs
            Prefix::Default => mode,
            // Fix is resolved to FailFast above, so this arm is unreachable,
            // but the compiler requires exhaustiveness.
            Prefix::Fix => HookMode::FailFast,
        };

        // Build a temporary cmd view with the resolved prefix for execute_and_print.
        let resolved_cmd = HookCommand {
            prefix: effective_prefix,
            command: cmd.command.clone(),
            glob: cmd.glob.clone(),
        };

        let (result, failed) =
            execute_and_print(&resolved_cmd, msg_path, &staged_files, index, total);
        index += 1;
        if failed {
            has_failure = true;
        }

        results.push(result);

        // In fail-fast mode, abort on first non-advisory failure
        if failed && effective_mode == HookMode::FailFast {
            // Re-stage formatted files and clean up stash before returning.
            if use_stash_dance {
                restage_files(&staged_files);
                if stash_active {
                    stash_drop();
                }
            }

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

    // Complete the fix-mode finalisation after all commands have run.
    if use_stash_dance {
        // Re-stage the originally-staged files (picks up formatter changes).
        // This always runs when fix-mode is active, whether or not a stash
        // was created (no stash means no unstaged changes to protect, but
        // re-staging is still needed to pick up formatter output).
        restage_files(&staged_files);

        if stash_active {
            // Warn about any unstaged files that the formatter also touched.
            // These are files in `git diff --name-only` that were NOT in
            // the original staged set.
            let now_unstaged = fetch_unstaged_files();
            for file in &now_unstaged {
                if !staged_files.contains(file) {
                    ui::warning(&format!("{file}: unstaged changes were also formatted"));
                }
            }

            stash_drop();
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
