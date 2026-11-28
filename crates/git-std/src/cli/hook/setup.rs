use standard_githooks::{HookCommand, Prefix};

use crate::app::OutputFormat;
use crate::ui;

use super::output::print_failure_hints;
use super::{case_collision, failure, stash};

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

    if use_stash_dance && let Err(message) = stash::ensure_utf8_staged_paths() {
        let code = failure::emit(format, message);
        if format != OutputFormat::Json {
            ui::hint(
                "rename the file, or remove the ~ prefix from commands in \
                 .githooks/pre-commit.hooks",
            );
            print_failure_hints(hook);
        }
        return Err(code);
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
    if use_stash_dance && !case_collision::ignore_case_is_true() {
        let root = crate::git::workdir(&std::env::current_dir().unwrap_or_default())
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let mismatches = case_collision::directory_mismatches(&root, &staged_rename_targets);
        if let Some(first) = mismatches.first() {
            return Err(report_directory_mismatches(
                format,
                hook,
                &root,
                &mismatches,
                first,
            ));
        }
    }
    let unstageable_rename_targets =
        case_collision::without_colliding_renames(&staged_rename_targets, &staged_rename_sources);
    let colliding_rename_targets =
        case_collision::colliding_rename_targets(&staged_rename_targets, &staged_rename_sources);
    let colliding_rename_sources =
        case_collision::colliding_rename_sources(&staged_rename_targets, &staged_rename_sources);
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
    if use_stash_dance && !stash::unstage_renames(&unstageable_rename_targets) {
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
        && let Err(message) = case_collision::remove_work_tree_files(&colliding_rename_sources)
            .and_then(|()| stash::stash_apply(stash_sha))
            .and_then(|()| case_collision::restore_target_names(&colliding_rename_targets))
    {
        let code = fail_after_rename_unstage(format, &staged_rename_targets, message);
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

/// Quote a path for a POSIX shell.
fn shell_quote(path: &str) -> String {
    format!("'{}'", path.replace('\'', "'\\''"))
}

/// Build the shell command that gives a directory on disk the case of the
/// index, through a temporary name.
///
/// The temporary name is the first of `<disk>.git-std-tmp`,
/// `<disk>.git-std-tmp-2`, and so on that does not exist under `root`. A
/// dangling symlink counts as existing, because `mv` would move the directory
/// into the link target or fail.
fn move_directory_command(root: &std::path::Path, disk: &str, index: &str) -> String {
    let mut temporary = format!("{disk}.git-std-tmp");
    let mut counter = 1;
    while root.join(&temporary).symlink_metadata().is_ok() {
        counter += 1;
        temporary = format!("{disk}.git-std-tmp-{counter}");
    }
    let temporary = shell_quote(&temporary);
    format!(
        "mv -- {disk} {temporary} && mv -- {temporary} {index}",
        disk = shell_quote(disk),
        index = shell_quote(index)
    )
}

/// Stop before the hook changes anything: the stash cannot hold a staged
/// rename whose directory has another case on disk (#609).
fn report_directory_mismatches(
    format: OutputFormat,
    hook: &str,
    root: &std::path::Path,
    mismatches: &[case_collision::DirectoryMismatch],
    first: &case_collision::DirectoryMismatch,
) -> i32 {
    let paths: Vec<&str> = mismatches
        .iter()
        .map(|mismatch| mismatch.path.as_str())
        .collect();
    let code = failure::emit(
        format,
        format!(
            "fix mode (~) cannot stash a staged rename into a directory whose case differs on \
             disk: {}",
            paths.join(", ")
        ),
    );
    if format != OutputFormat::Json {
        ui::hint(&format!(
            "the index has `{}` and the work tree has `{}` — they are one directory on this \
             filesystem",
            first.index_directory, first.disk_directory
        ));
        ui::hint(&format!(
            "to give the directory on disk the case of the index: {}",
            move_directory_command(root, &first.disk_directory, &first.index_directory)
        ));
        ui::hint(&format!(
            "if `git status` still lists files under `{old}`, stage them under `{new}` (for \
             example with `git mv`, or `git rm --cached` and `git add` for each file), then \
             review `git diff --cached`",
            old = first.disk_directory,
            new = first.index_directory
        ));
        print_failure_hints(hook);
    }
    code
}

#[cfg(test)]
mod tests {
    use super::{move_directory_command, shell_quote};

    #[test]
    fn shell_quote_keeps_a_single_quote_inside_the_word() {
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }

    fn run(root: &std::path::Path, command: &str) {
        let status = std::process::Command::new("sh")
            .current_dir(root)
            .args(["-c", command])
            .status()
            .unwrap();
        assert!(status.success(), "{command}");
    }

    fn listing(root: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn move_directory_command_renames_a_directory_with_a_space_and_a_quote() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("my dir's")).unwrap();
        std::fs::write(root.path().join("my dir's/file.txt"), "x").unwrap();
        let command = move_directory_command(root.path(), "my dir's", "MY DIR's");
        assert_eq!(
            command,
            "mv -- 'my dir'\\''s' 'my dir'\\''s.git-std-tmp' && \
             mv -- 'my dir'\\''s.git-std-tmp' 'MY DIR'\\''s'"
        );
        run(root.path(), &command);
        assert_eq!(listing(root.path()), ["MY DIR's"]);
    }

    #[test]
    fn move_directory_command_handles_a_leading_dash() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("-dir")).unwrap();
        run(
            root.path(),
            &move_directory_command(root.path(), "-dir", "-DIR"),
        );
        assert_eq!(listing(root.path()), ["-DIR"]);
    }

    #[test]
    fn move_directory_command_handles_a_nested_path() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("a/dir")).unwrap();
        run(
            root.path(),
            &move_directory_command(root.path(), "a/dir", "a/DIR"),
        );
        assert_eq!(listing(&root.path().join("a")), ["DIR"]);
    }

    #[test]
    fn move_directory_command_skips_temporary_names_that_exist() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("dir")).unwrap();
        std::fs::create_dir(root.path().join("dir.git-std-tmp")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("missing", root.path().join("dir.git-std-tmp-2")).unwrap();
        let command = move_directory_command(root.path(), "dir", "DIR");
        #[cfg(unix)]
        assert!(command.contains("'dir.git-std-tmp-3'"), "{command}");
        run(root.path(), &command);
        let names = listing(root.path());
        assert!(names.contains(&"DIR".to_string()), "{names:?}");
        assert!(names.contains(&"dir.git-std-tmp".to_string()), "{names:?}");
        let existing = root.path().join("dir.git-std-tmp");
        assert!(existing.read_dir().unwrap().next().is_none());
    }
}
