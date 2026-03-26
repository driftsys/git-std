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

/// Fetch all tracked files for glob filtering in non-pre-commit hooks.
///
/// Returns file paths from `git ls-files`. Only called when at least one
/// command has a glob pattern and the hook is not `pre-commit` (pre-commit
/// reuses the already-fetched staged files instead).
fn fetch_tracked_files() -> Option<Vec<String>> {
    match Command::new("git").args(["ls-files"]).output() {
        Ok(o) => Some(
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(String::from)
                .collect(),
        ),
        Err(_) => None,
    }
}

/// Fetch staged file paths matching the given `--diff-filter`.
///
/// Returns file paths from `git diff --cached --name-only --diff-filter=<filter>`
/// relative to the working tree root. Returns an empty vec on failure.
fn fetch_staged(filter: &str) -> Vec<String> {
    match Command::new("git")
        .args([
            "diff",
            "--cached",
            "--name-only",
            &format!("--diff-filter={filter}"),
        ])
        .output()
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(String::from)
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Re-apply staged deletions after the stash dance.
///
/// Runs `git rm --cached --quiet -- <files>` to restore the deletion state
/// in the index without touching the working tree. This undoes the effect
/// of `stash apply` which restores deleted files.
///
/// Returns `true` on success, `false` if the command fails. A failure means
/// the user's `git rm` intent would be silently lost — callers must treat
/// this as a fatal error.
fn restage_deletions(files: &[String]) -> bool {
    if files.is_empty() {
        return true;
    }
    let mut cmd = Command::new("git");
    cmd.args(["rm", "--cached", "--quiet", "--force", "--"]);
    for f in files {
        cmd.arg(f);
    }
    match cmd.status() {
        Ok(s) if s.success() => true,
        Ok(s) => {
            let code = s.code().unwrap_or(-1);
            ui::error(&format!(
                "git rm --cached failed (exit {code}) — staged deletions may be lost"
            ));
            false
        }
        Err(e) => {
            ui::error(&format!(
                "git rm --cached failed after fix-mode stash dance: {e}"
            ));
            false
        }
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

/// Check whether any staged entries are submodules (mode `160000`).
///
/// Parses `git diff --cached --diff-filter=ACMR --raw` and looks for the
/// submodule file mode. Returns `true` if at least one submodule entry is
/// staged.
fn has_staged_submodules() -> bool {
    let output = Command::new("git")
        .args(["diff", "--cached", "--diff-filter=ACMR", "--raw"])
        .output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).contains(" 160000 "),
        Err(_) => false,
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

/// Run `git stash apply --quiet`. Returns `true` on success, `false` on
/// failure (e.g. merge conflicts).
fn stash_apply() -> bool {
    Command::new("git")
        .args(["stash", "apply", "--quiet"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
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
/// Files that no longer exist on disk are skipped with a warning to
/// prevent a formatter-caused deletion from being silently staged (#279).
///
/// Returns `true` on success, `false` if the command fails. A failure means
/// formatted changes would be silently lost — callers must treat this as a
/// fatal error.
fn restage_files(files: &[String]) -> bool {
    if files.is_empty() {
        return true;
    }
    let mut existing: Vec<&String> = Vec::new();
    for f in files {
        if std::path::Path::new(f).exists() {
            existing.push(f);
        } else {
            ui::warning(&format!("{f} was deleted by formatter — skipping restage"));
        }
    }
    if existing.is_empty() {
        return true;
    }
    let mut cmd = Command::new("git");
    cmd.arg("add").arg("--");
    for f in &existing {
        cmd.arg(f);
    }
    match cmd.status() {
        Ok(s) if s.success() => true,
        Ok(s) => {
            let code = s.code().unwrap_or(-1);
            ui::error(&format!(
                "git add failed (exit {code}) — formatted changes may be lost"
            ));
            false
        }
        Err(e) => {
            ui::error(&format!("git add failed after fix-mode formatting: {e}"));
            false
        }
    }
}

/// Print contextual hints after a hook failure.
///
/// Shows how to skip the current hook, how to skip all hooks, and how
/// to disable a specific command in the `.hooks` file.
fn print_failure_hints(hook: &str) {
    let skip_flag = match hook {
        "pre-commit" | "commit-msg" => "git commit --no-verify",
        "pre-push" => "git push --no-verify",
        _ => &format!(
            "GIT_STD_SKIP_HOOKS=1 git {}",
            hook.trim_start_matches("pre-").trim_start_matches("post-")
        ),
    };
    ui::hint(&format!("to skip this hook:    {skip_flag}"));
    ui::hint("to skip all hooks:    GIT_STD_SKIP_HOOKS=1 git ...");
    ui::hint(&format!(
        "to disable a command: comment it out in .githooks/{hook}.hooks"
    ));
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

    // For pre-commit: fetch staged files for $@ passing, stash dance, and glob filtering.
    let staged_files: Vec<String> = if hook == "pre-commit" {
        fetch_staged("ACMR")
    } else {
        Vec::new()
    };

    // Collect file list for glob filtering (lazy -- only fetched if needed).
    // For pre-commit, reuse the already-fetched staged files to avoid a duplicate git call.
    let file_list: Option<Vec<String>> = if commands.iter().any(|c| c.glob.is_some()) {
        if hook == "pre-commit" {
            Some(staged_files.clone())
        } else {
            fetch_tracked_files()
        }
    } else {
        None
    };

    // Determine whether we need the stash dance.
    // Only applies for pre-commit hooks that contain at least one `~` command.
    let has_fix_commands = commands.iter().any(|c| c.prefix == Prefix::Fix);
    let use_stash_dance = hook == "pre-commit" && has_fix_commands;

    // For non-pre-commit hooks, warn about `~` commands and treat them as `!`.
    if hook != "pre-commit" && has_fix_commands {
        ui::warning("~ prefix is only supported in pre-commit — treating as !");
    }

    // Reject submodule entries when fix mode is active (#283).
    // The stash dance does not handle submodule state correctly.
    if use_stash_dance && has_staged_submodules() {
        ui::error("fix mode (~) does not support submodule entries");
        ui::hint(
            "remove ~ prefix from commands in .githooks/pre-commit.hooks, \
             or unstage the submodule",
        );
        return 1;
    }

    // Capture files staged for deletion before the stash dance.
    // The stash dance restores deleted files to disk; we must re-delete them
    // in the index afterwards to preserve the user's `git rm` intent (#268).
    let staged_deletions: Vec<String> = if use_stash_dance {
        fetch_staged("D")
    } else {
        Vec::new()
    };

    // Perform the stash dance if needed.
    // stash_active tracks whether a stash entry was actually created.
    let stash_active = if use_stash_dance {
        stash_push()
        // If stash_push returns false (nothing to stash or error), we skip
        // the stash dance but still run commands normally.
    } else {
        false
    };

    if use_stash_dance && stash_active && !stash_apply() {
        ui::error("stash apply failed — working tree has conflicting unstaged changes");
        ui::hint("commit or stash your unstaged changes first, then retry");
        stash_drop();
        print_failure_hints(hook);
        return 1;
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
            Prefix::Fix => unreachable!("Fix prefix resolved to FailFast above"),
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
                if !restage_files(&staged_files) || !restage_deletions(&staged_deletions) {
                    // Already returning 1 for the fail-fast failure, but
                    // ensure the stash is cleaned up before returning.
                    if stash_active {
                        stash_drop();
                    }
                    ui::blank();
                    print_failure_hints(hook);
                    return 1;
                }
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
            print_failure_hints(hook);
            return 1;
        }
    }

    // Complete the fix-mode finalisation after all commands have run.
    if use_stash_dance {
        // Re-stage the originally-staged files (picks up formatter changes).
        // This always runs when fix-mode is active, whether or not a stash
        // was created (no stash means no unstaged changes to protect, but
        // re-staging is still needed to pick up formatter output).
        if !restage_files(&staged_files) || !restage_deletions(&staged_deletions) {
            if stash_active {
                stash_drop();
            }
            print_failure_hints(hook);
            return 1;
        }

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

    if has_failure {
        print_failure_hints(hook);
        1
    } else {
        0
    }
}
