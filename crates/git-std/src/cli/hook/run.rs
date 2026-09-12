use yansi::Paint;

use standard_githooks::{HookCommand, HookMode, Prefix, default_mode, substitute_msg};

use crate::app::OutputFormat;
use crate::ui;

use super::output::{CommandExecutionJson, emit_json_result, format_display, print_failure_hints};
use super::read_and_parse_hooks_for;
use super::setup::HookRunSetup;
use super::{failure, setup, stash};

/// The result of executing a single hook command.
struct CommandResult {
    /// The exit code (0 = success).
    exit_code: Option<i32>,
    /// Whether this command was advisory.
    advisory: bool,
}

/// Execute a single hook command, optionally printing its result line.
///
/// When `quiet` is true, the command runs silently (for JSON output mode).
/// Otherwise animates a spinner while the command runs and prints the result.
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
    quiet: bool,
) -> (CommandResult, bool) {
    let command_text = substitute_msg(&cmd.command, msg_path);
    let is_advisory = cmd.prefix == Prefix::Advisory;
    let display = format_display(&command_text, cmd.glob.as_deref());

    // Run the command, capturing output only on TTY (to show on failure).
    // On non-TTY (tests, CI), let output inherit so it's visible.
    let (exit_code, captured) = if !quiet && ui::is_tty() {
        // TTY: use spinner and capture output to show only on failure
        ui::spin_while(&display, || {
            super::exec_sh_capture(&command_text, staged_files)
        })
    } else if !quiet {
        // Non-TTY: show pending, let output inherit, print result
        ui::pending_non_tty(&display);
        let code = super::exec_sh(&command_text, staged_files);
        (code, String::new())
    } else {
        // JSON / quiet mode: capture child streams so stdout remains one JSON document.
        super::exec_sh_capture(&command_text, staged_files)
    };

    let success = exit_code == Some(0);

    // Print the result line and dump captured output on failure/advisory.
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

        // Dump captured output below the result line on failure or advisory.
        if !success && !captured.is_empty() {
            ui::blank();
            for line in captured.lines() {
                ui::detail(line);
            }
            ui::blank();
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
            return emit_json_result(hook, &[], false);
        } else {
            ui::info(&format!(
                "{} hooks skipped (GIT_STD_SKIP_HOOKS)",
                ui::warn()
            ));
        }
        return 0;
    }

    let hooks_dir = match super::hooks_dir_for(format) {
        Ok(d) => d,
        Err(code) => return code,
    };

    let commands = match read_and_parse_hooks_for(&hooks_dir, hook, format) {
        Ok(c) => c,
        Err(code) => return code,
    };
    if commands.is_empty() {
        if format == OutputFormat::Json {
            return emit_json_result(hook, &[], false);
        }
        return 0;
    }

    let mode = default_mode(hook);

    // Determine the msg_path from args (first argument after --)
    let msg_path = args.first().map(|s| s.as_str()).unwrap_or("");

    let HookRunSetup {
        staged_files,
        file_list,
        use_stash_dance,
        staged_rename_targets,
        staged_deletions,
        hook_stash,
    } = match setup::prepare(hook, &commands, format) {
        Ok(setup) => setup,
        Err(code) => return code,
    };

    let mut results: Vec<CommandResult> = Vec::new();
    let mut json_results: Vec<CommandExecutionJson> = Vec::new();
    let mut has_failure = false;

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

        let (result, failed) = execute_and_print(&resolved_cmd, msg_path, &staged_files, is_json);
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
                let restaged = stash::restage_files(&staged_files)
                    .and_then(|()| stash::restage_deletions(&staged_deletions));
                if let Err(message) = restaged {
                    // Already returning 1 for the fail-fast failure, but
                    // ensure the stash is cleaned up before returning.
                    if let Some(ref stash_sha) = hook_stash
                        && let Err(code) = failure::drop_stash(format, stash_sha)
                    {
                        return code;
                    }
                    let code = failure::emit(format, message);
                    if format != OutputFormat::Json {
                        ui::blank();
                        print_failure_hints(hook);
                    }
                    return code;
                }
                if let Some(ref stash_sha) = hook_stash
                    && let Err(code) = failure::drop_stash(format, stash_sha)
                {
                    return code;
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
        let restaged = stash::restage_files(&staged_files)
            .and_then(|()| stash::restage_deletions(&staged_deletions));
        if let Err(message) = restaged {
            if let Some(ref stash_sha) = hook_stash
                && let Err(code) = failure::drop_stash(format, stash_sha)
            {
                return code;
            }
            let code = failure::emit(format, message);
            if format != OutputFormat::Json {
                print_failure_hints(hook);
            }
            return code;
        }

        if let Some(ref stash_sha) = hook_stash {
            // Warn about any unstaged files that the formatter also touched.
            // These are files in `git diff --name-only` that were NOT in
            // the original staged set.
            let now_unstaged = match stash::fetch_unstaged_files() {
                Ok(files) => files,
                Err(message) => {
                    return cleanup_after_inspection_failure(
                        format,
                        message,
                        stash_sha,
                        &staged_rename_targets,
                    );
                }
            };
            for file in &now_unstaged {
                if format != OutputFormat::Json && !staged_files.contains(file) {
                    ui::warning(&format!("{file}: unstaged changes were also formatted"));
                }
            }

            if let Err(code) = failure::drop_stash(format, stash_sha) {
                return code;
            }
        }

        // Re-stage renamed files after the stash dance completes.
        // They were unstaged before to prevent stash corruption (#387).
        if let Err(message) = stash::restage_renames(&staged_rename_targets) {
            let code = failure::emit(format, message);
            if format != OutputFormat::Json {
                print_failure_hints(hook);
            }
            return code;
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

fn cleanup_after_inspection_failure(
    format: OutputFormat,
    original: String,
    stash_sha: &str,
    rename_targets: &[String],
) -> i32 {
    let mut failures = vec![original];
    if let Err(message) = stash::stash_drop(stash_sha) {
        failures.push(message);
    }
    if let Err(message) = stash::restage_renames(rename_targets) {
        failures.push(message);
    }
    failure::emit(format, failures.join("; "))
}
