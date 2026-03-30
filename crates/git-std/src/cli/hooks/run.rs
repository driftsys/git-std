use std::process::Command;

use serde::Serialize;
use yansi::Paint;

use standard_githooks::{HookCommand, HookMode, Prefix, default_mode, substitute_msg};

use crate::app::OutputFormat;
use crate::ui;

use super::read_and_parse_hooks;
use super::stash;

/// The result of executing a single hook command.
struct CommandResult {
    /// The exit code (0 = success).
    exit_code: Option<i32>,
    /// Whether this command was advisory.
    advisory: bool,
}

/// JSON output schema for a single executed command.
#[derive(Serialize)]
struct CommandExecutionJson {
    command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    glob: Option<String>,
    exit_code: Option<i32>,
    success: bool,
    advisory: bool,
    skipped: bool,
}

/// JSON output schema for the hooks run result.
#[derive(Serialize)]
struct HooksRunResultJson {
    hook: String,
    commands: Vec<CommandExecutionJson>,
    passed: usize,
    failed: usize,
    advisory_warnings: usize,
    skipped: usize,
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

/// Execute a single hook command, optionally printing its result line.
///
/// When `quiet` is true, the command runs silently (for JSON output mode).
/// Otherwise prints a pending indicator before spawning the command.
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
    quiet: bool,
) -> (CommandResult, bool) {
    let command_text = substitute_msg(&cmd.command, msg_path);
    let is_advisory = cmd.prefix == Prefix::Advisory;
    let display = format_display(&command_text, cmd.glob.as_deref());

    // Show the pending indicator before spawning.
    if !quiet {
        ui::pending(index, total, &display);
    }

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
    if !quiet && ui::is_tty() && yansi::is_enabled() {
        eprint!("\r\x1b[K");
    }

    // Print the result line
    if !quiet {
        if success {
            ui::info(&format!("{} {}", ui::pass(), display));
        } else if is_advisory {
            let info = match exit_code {
                Some(code) => format!("(advisory, exit {code})"),
                None => "(advisory, killed)".to_string(),
            };
            ui::info(&format!("{} {} {}", ui::warn(), display, info.yellow()));
        } else {
            let info = match exit_code {
                Some(code) => format!("(exit {code})"),
                None => "(killed)".to_string(),
            };
            ui::info(&format!("{} {} {}", ui::fail(), display, info.red()));
        }
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
pub fn run(hook: &str, args: &[String], format: OutputFormat) -> i32 {
    // Allow skipping all hook execution via environment variable.
    if let Ok(val) = std::env::var("GIT_STD_SKIP_HOOKS")
        && (val == "1" || val.eq_ignore_ascii_case("true"))
    {
        if format == OutputFormat::Json {
            let result = HooksRunResultJson {
                hook: hook.to_string(),
                commands: vec![],
                passed: 0,
                failed: 0,
                advisory_warnings: 0,
                skipped: 0,
            };
            println!("{}", serde_json::to_string(&result).unwrap());
        } else {
            ui::info(&format!(
                "{} hooks skipped (GIT_STD_SKIP_HOOKS)",
                ui::warn()
            ));
        }
        return 0;
    }

    let hooks_dir = match super::hooks_dir() {
        Ok(d) => d,
        Err(code) => return code,
    };

    let commands = match read_and_parse_hooks(&hooks_dir, hook) {
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
        stash::fetch_staged("ACMR")
    } else {
        Vec::new()
    };

    // Collect file list for glob filtering (lazy -- only fetched if needed).
    // For pre-commit, reuse the already-fetched staged files to avoid a duplicate git call.
    let file_list: Option<Vec<String>> = if commands.iter().any(|c| c.glob.is_some()) {
        if hook == "pre-commit" {
            Some(staged_files.clone())
        } else {
            stash::fetch_tracked_files()
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
    if use_stash_dance && stash::has_staged_submodules() {
        ui::error("fix mode (~) does not support submodule entries");
        ui::hint(
            "remove ~ prefix from commands in .githooks/pre-commit.hooks, \
             or unstage the submodule",
        );
        return 1;
    }

    // Temporarily unstage renames before the stash dance to prevent corruption
    // (#387). git stash apply incorrectly splits renames into separate staged
    // additions and unstaged deletions. We unstage them before stash, then
    // re-stage after, so they bypass the stash corruption entirely.
    let staged_rename_targets = if use_stash_dance {
        stash::fetch_staged_rename_targets()
    } else {
        Vec::new()
    };

    if use_stash_dance && !stash::unstage_renames(&staged_rename_targets) {
        ui::error("failed to unstage renames before stash dance");
        print_failure_hints(hook);
        return 1;
    }

    // Capture files staged for deletion before the stash dance.
    // The stash dance restores deleted files to disk; we must re-delete them
    // in the index afterwards to preserve the user's `git rm` intent (#268).
    let staged_deletions: Vec<String> = if use_stash_dance {
        stash::fetch_staged("D")
    } else {
        Vec::new()
    };

    // Perform the stash dance if needed.
    // stash_active tracks whether a stash entry was actually created.
    let stash_active = if use_stash_dance {
        stash::stash_push()
        // If stash_push returns false (nothing to stash or error), we skip
        // the stash dance but still run commands normally.
    } else {
        false
    };

    if use_stash_dance && stash_active && !stash::stash_apply() {
        ui::error("stash apply failed — working tree has conflicting unstaged changes");
        ui::hint("commit or stash your unstaged changes first, then retry");
        stash::stash_drop();
        print_failure_hints(hook);
        return 1;
    }

    let mut results: Vec<CommandResult> = Vec::new();
    let mut json_results: Vec<CommandExecutionJson> = Vec::new();
    let mut has_failure = false;
    let total = commands.len();
    let mut index: usize = 0;

    let is_json = format == OutputFormat::Json;

    for cmd in &commands {
        // Glob filtering: skip command if glob doesn't match any files.
        if let Some(ref glob) = cmd.glob
            && let Some(ref files) = file_list
        {
            let refs: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
            if !standard_githooks::matches_any(glob, &refs) {
                if is_json {
                    json_results.push(CommandExecutionJson {
                        command: cmd.command.clone(),
                        glob: cmd.glob.clone(),
                        exit_code: None,
                        success: false,
                        advisory: cmd.prefix == Prefix::Advisory,
                        skipped: true,
                    });
                }
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

        let (result, failed) = execute_and_print(
            &resolved_cmd,
            msg_path,
            &staged_files,
            index,
            total,
            is_json,
        );
        index += 1;
        if failed {
            has_failure = true;
        }

        if is_json {
            let command_text = substitute_msg(&cmd.command, msg_path);
            json_results.push(CommandExecutionJson {
                command: command_text,
                glob: cmd.glob.clone(),
                exit_code: result.exit_code,
                success: result.exit_code == Some(0),
                advisory: result.advisory,
                skipped: false,
            });
        }

        results.push(result);

        // In fail-fast mode, abort on first non-advisory failure
        if failed && effective_mode == HookMode::FailFast {
            // Re-stage formatted files and clean up stash before returning.
            if use_stash_dance {
                if !stash::restage_files(&staged_files)
                    || !stash::restage_deletions(&staged_deletions)
                {
                    // Already returning 1 for the fail-fast failure, but
                    // ensure the stash is cleaned up before returning.
                    if stash_active {
                        stash::stash_drop();
                    }
                    ui::blank();
                    print_failure_hints(hook);
                    return 1;
                }
                if stash_active {
                    stash::stash_drop();
                }
            }

            // Print remaining commands as skipped
            let remaining = commands.len() - results.len();
            if is_json {
                // Add remaining commands as skipped
                for remaining_cmd in commands.iter().skip(results.len()) {
                    let command_text = substitute_msg(&remaining_cmd.command, msg_path);
                    json_results.push(CommandExecutionJson {
                        command: command_text,
                        glob: remaining_cmd.glob.clone(),
                        exit_code: None,
                        success: false,
                        advisory: remaining_cmd.prefix == Prefix::Advisory,
                        skipped: true,
                    });
                }
                return emit_json_result(hook, &json_results, has_failure);
            }
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
        if !stash::restage_files(&staged_files) || !stash::restage_deletions(&staged_deletions) {
            if stash_active {
                stash::stash_drop();
            }
            print_failure_hints(hook);
            return 1;
        }

        if stash_active {
            // Warn about any unstaged files that the formatter also touched.
            // These are files in `git diff --name-only` that were NOT in
            // the original staged set.
            let now_unstaged = stash::fetch_unstaged_files();
            for file in &now_unstaged {
                if !staged_files.contains(file) {
                    ui::warning(&format!("{file}: unstaged changes were also formatted"));
                }
            }

            stash::stash_drop();
        }

        // Re-stage renamed files after the stash dance completes.
        // They were unstaged before to prevent stash corruption (#387).
        if !staged_rename_targets.is_empty() {
            let mut cmd = Command::new("git");
            cmd.args(["add", "--"]);
            for f in &staged_rename_targets {
                cmd.arg(f);
            }
            match cmd.status() {
                Ok(s) if !s.success() => {
                    ui::error("failed to re-stage renamed files after formatting");
                    print_failure_hints(hook);
                    return 1;
                }
                Err(e) => {
                    ui::error(&format!("failed to re-stage renamed files: {e}"));
                    print_failure_hints(hook);
                    return 1;
                }
                _ => {}
            }
        }
    }

    // Print summary
    if is_json {
        return emit_json_result(hook, &json_results, has_failure);
    }

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

fn emit_json_result(hook: &str, json_results: &[CommandExecutionJson], has_failure: bool) -> i32 {
    let passed = json_results
        .iter()
        .filter(|r| r.success && !r.skipped)
        .count();
    let failed = json_results
        .iter()
        .filter(|r| !r.success && !r.advisory && !r.skipped)
        .count();
    let advisory_warnings = json_results
        .iter()
        .filter(|r| !r.success && r.advisory && !r.skipped)
        .count();
    let skipped = json_results.iter().filter(|r| r.skipped).count();

    let result = HooksRunResultJson {
        hook: hook.to_string(),
        commands: json_results
            .iter()
            .map(|r| CommandExecutionJson {
                command: r.command.clone(),
                glob: r.glob.clone(),
                exit_code: r.exit_code,
                success: r.success,
                advisory: r.advisory,
                skipped: r.skipped,
            })
            .collect(),
        passed,
        failed,
        advisory_warnings,
        skipped,
    };
    println!("{}", serde_json::to_string(&result).unwrap());
    if has_failure { 1 } else { 0 }
}
