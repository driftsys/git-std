use standard_githooks::{HookCommand, Prefix};

use crate::app::OutputFormat;
use crate::ui;

use super::output::print_failure_hints;
use super::{failure, stash};

pub(super) struct HookRunSetup {
    pub(super) staged_files: Vec<String>,
    pub(super) file_list: Option<Vec<String>>,
    pub(super) use_stash_dance: bool,
    pub(super) staged_rename_targets: Vec<String>,
    pub(super) staged_deletions: Vec<String>,
    pub(super) hook_stash: Option<String>,
}

pub(super) fn prepare(
    hook: &str,
    commands: &[HookCommand],
    format: OutputFormat,
) -> Result<HookRunSetup, i32> {
    let staged_files = if hook == "pre-commit" {
        report(format, stash::fetch_staged("ACMR"))?
    } else {
        Vec::new()
    };
    let file_list = if commands.iter().any(|command| command.glob.is_some()) {
        Some(if hook == "pre-commit" {
            staged_files.clone()
        } else {
            report(format, stash::fetch_tracked_files())?
        })
    } else {
        None
    };

    let has_fix_commands = commands.iter().any(|command| command.prefix == Prefix::Fix);
    let use_stash_dance = hook == "pre-commit" && has_fix_commands;
    if hook != "pre-commit" && has_fix_commands && format != OutputFormat::Json {
        ui::warning("~ prefix is only supported in pre-commit — treating as !");
    }

    if use_stash_dance && report(format, stash::has_staged_submodules())? {
        let code = failure::emit(format, "fix mode (~) does not support submodule entries");
        if format != OutputFormat::Json {
            ui::hint(
                "remove ~ prefix from commands in .githooks/pre-commit.hooks, \
                 or unstage the submodule",
            );
        }
        return Err(code);
    }

    let staged_rename_targets = if use_stash_dance {
        report(format, stash::fetch_staged_rename_targets())?
    } else {
        Vec::new()
    };
    let staged_rename_sources = if use_stash_dance {
        report(format, stash::fetch_staged_rename_sources())?
    } else {
        Vec::new()
    };
    let mut staged_deletions = if use_stash_dance {
        report(format, stash::fetch_staged("D"))?
    } else {
        Vec::new()
    };
    for source in staged_rename_sources {
        if !staged_deletions.contains(&source) {
            staged_deletions.push(source);
        }
    }
    if use_stash_dance && !stash::unstage_renames(&staged_rename_targets) {
        let code = failure::emit(format, "failed to unstage renames before stash dance");
        if format != OutputFormat::Json {
            print_failure_hints(hook);
        }
        return Err(code);
    }

    let hook_stash = if use_stash_dance {
        match stash::stash_push() {
            Ok(stash) => stash,
            Err(message) => {
                return Err(fail_after_rename_unstage(
                    format,
                    &staged_rename_targets,
                    message,
                ));
            }
        }
    } else {
        None
    };
    if let Some(ref stash_sha) = hook_stash
        && !stash::stash_apply(stash_sha)
    {
        let code = fail_after_rename_unstage(
            format,
            &staged_rename_targets,
            "stash apply failed — working tree has conflicting unstaged changes",
        );
        if format != OutputFormat::Json {
            ui::hint(&format!(
                "your original changes are preserved in the stash ({stash_sha}) — resolve \
                 the conflict, inspect it with `git stash show -p {stash_sha}`, then drop it \
                 from `git stash list` once recovered"
            ));
            print_failure_hints(hook);
        }
        return Err(code);
    }

    Ok(HookRunSetup {
        staged_files,
        file_list,
        use_stash_dance,
        staged_rename_targets,
        staged_deletions,
        hook_stash,
    })
}

fn report<T>(format: OutputFormat, result: Result<T, String>) -> Result<T, i32> {
    result.map_err(|message| failure::emit(format, message))
}

fn fail_after_rename_unstage(
    format: OutputFormat,
    rename_targets: &[String],
    original: impl Into<String>,
) -> i32 {
    let original = original.into();
    let message = match stash::restage_renames(rename_targets) {
        Ok(()) => original,
        Err(rollback) => {
            format!("{original}; failed to restore staged renames after setup failure: {rollback}")
        }
    };
    failure::emit(format, message)
}
