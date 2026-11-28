//! Rename targets and sources that differ only by case.
//!
//! On a case-insensitive filesystem such names are one file, which the
//! pre-commit stash dance must not split (#604).

/// Drop rename targets that collide with a rename source from the list that
/// gets unstaged.
///
/// A target collides when its lowercased path equals the lowercased path of
/// any staged rename source (#604). This covers a case-only rename
/// (`rules.md` to `RULES.md`, in either direction) and a swap across several
/// renames (`dir/a.txt` to `DIR/b.txt` with `dir/b.txt` to `DIR/a.txt`). On a
/// case-insensitive filesystem the target and the source are the same file
/// on disk, so unstaging the target makes it an untracked file that
/// `git stash apply` cannot restore over the tracked source. Such targets
/// stay in the index.
///
/// The check compares names only. It uses neither the filesystem nor
/// `core.ignorecase`. The callers read raw names with `-z`, so a non-ASCII
/// name arrives as written and is compared as written (#606).
pub(super) fn without_colliding_renames(targets: &[String], sources: &[String]) -> Vec<String> {
    let lowered_sources: Vec<String> = sources.iter().map(|source| source.to_lowercase()).collect();
    targets
        .iter()
        .filter(|target| !lowered_sources.contains(&target.to_lowercase()))
        .cloned()
        .collect()
}

/// Select the rename sources that collide with a rename target.
///
/// Uses the same name-only rule as [`without_colliding_renames`], seen from
/// the source side.
pub(super) fn colliding_rename_sources(targets: &[String], sources: &[String]) -> Vec<String> {
    let lowered_targets: Vec<String> = targets.iter().map(|target| target.to_lowercase()).collect();
    sources
        .iter()
        .filter(|source| lowered_targets.contains(&source.to_lowercase()))
        .cloned()
        .collect()
}

/// Remove the work tree copies of colliding rename sources after the stash
/// push.
///
/// `git stash push` resets the work tree to HEAD, so each source is back with
/// its committed content. When `core.ignorecase` is not true, Git then treats
/// the stashed target as an untracked file that would overwrite that copy,
/// and `git stash apply` aborts (#604). The copy holds only committed content,
/// so removing it loses nothing; the stash apply recreates the file under its
/// new name. A copy that is already gone is not an error.
pub(super) fn remove_work_tree_files(paths: &[String]) -> Result<(), String> {
    for path in paths {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("failed to prepare stash apply for {path}: {error}")),
        }
    }
    Ok(())
}

/// Select the rename targets that collide with a rename source.
///
/// Uses the same name-only rule as [`without_colliding_renames`], so these
/// are the targets that stay staged through the stash dance.
pub(super) fn colliding_rename_targets(targets: &[String], sources: &[String]) -> Vec<String> {
    let lowered_sources: Vec<String> = sources.iter().map(|source| source.to_lowercase()).collect();
    targets
        .iter()
        .filter(|target| lowered_sources.contains(&target.to_lowercase()))
        .cloned()
        .collect()
}

/// Give each colliding rename target its exact on-disk name after the stash
/// apply.
///
/// When the work tree holds an unstaged edit of the target, the stash
/// records the old name as a modified tracked file too, because the old name
/// resolves to the same file on a case-insensitive filesystem. The apply
/// then writes both names to one file and the old name wins. The directory
/// listing shows the exact name, so this renames the entry only when it
/// matches the target ignoring case, differs from the target, and the target
/// name itself is not listed. On a case-sensitive filesystem no such entry
/// exists and nothing changes.
pub(super) fn restore_target_names(targets: &[String]) -> Result<(), String> {
    for target in targets {
        let target_path = std::path::Path::new(target);
        let (Some(parent), Some(name)) = (target_path.parent(), target_path.file_name()) else {
            continue;
        };
        let parent = if parent.as_os_str().is_empty() {
            std::path::Path::new(".")
        } else {
            parent
        };
        let Ok(entries) = std::fs::read_dir(parent) else {
            continue;
        };
        let listed: Vec<std::ffi::OsString> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.file_name())
            .collect();
        if listed.iter().any(|entry| entry.as_os_str() == name) {
            continue;
        }
        let lowered = name.to_string_lossy().to_lowercase();
        if let Some(old) = listed
            .iter()
            .find(|entry| entry.to_string_lossy().to_lowercase() == lowered)
        {
            std::fs::rename(parent.join(old), target_path)
                .map_err(|error| format!("failed to restore the name of {target}: {error}"))?;
        }
    }
    Ok(())
}

/// A staged path whose directory has another case on disk (#609).
#[derive(Debug, PartialEq, Eq)]
pub(super) struct DirectoryMismatch {
    /// The staged path.
    pub(super) path: String,
    /// The directory as the index spells it, relative to the work tree root.
    pub(super) index_directory: String,
    /// The same directory as the work tree lists it.
    pub(super) disk_directory: String,
}

/// Check whether `core.ignorecase` is true in the current repository.
///
/// With it set, Git folds the directory case of an added path onto the case
/// already in the index, so the mismatch of #609 does not arise.
pub(super) fn ignore_case_is_true() -> bool {
    std::process::Command::new("git")
        .args(["config", "--type=bool", "--get", "core.ignorecase"])
        .output()
        .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).trim() == "true")
}

/// Select the staged paths with a directory that is an alias of the directory
/// on disk (#609).
///
/// `git mv dir/a.txt DIR/c.txt` records `DIR/c.txt` in the index while a
/// case-insensitive filesystem keeps the file in `dir/`. Git then lists the
/// work tree file as untracked, and `git stash apply` fails when the stash
/// holds the same file under two names. The hook cannot repair that state, so
/// it reports these paths before it changes anything.
///
/// `paths` are relative to `root`, the top level of the work tree. A
/// directory is an alias when its parent lists no entry with the exact name
/// but the path with the index case still resolves to a directory. The check
/// reads the listing, because `Path::exists` ignores case on some
/// filesystems. On a case-sensitive filesystem the path does not resolve, so
/// nothing is reported there.
///
/// The callers read raw names with `-z`, so a path with non-ASCII characters
/// is detected like any other (#606).
pub(super) fn directory_mismatches(
    root: &std::path::Path,
    paths: &[String],
) -> Vec<DirectoryMismatch> {
    paths
        .iter()
        .filter_map(|path| directory_mismatch(root, path))
        .collect()
}

fn directory_mismatch(root: &std::path::Path, path: &str) -> Option<DirectoryMismatch> {
    let directories = std::path::Path::new(path).parent()?;
    let mut current = root.to_path_buf();
    let mut index_directory = std::path::PathBuf::new();
    let mut disk_directory = std::path::PathBuf::new();
    for component in directories.components() {
        let std::path::Component::Normal(name) = component else {
            continue;
        };
        let listed: Vec<std::ffi::OsString> = std::fs::read_dir(&current)
            .ok()?
            .filter_map(Result::ok)
            .map(|entry| entry.file_name())
            .collect();
        current.push(name);
        index_directory.push(name);
        if listed.iter().any(|entry| entry.as_os_str() == name) {
            disk_directory.push(name);
            continue;
        }
        let lowered = name.to_string_lossy().to_lowercase();
        let on_disk = listed
            .iter()
            .find(|entry| entry.to_string_lossy().to_lowercase() == lowered)?;
        if !std::fs::metadata(&current).ok()?.is_dir() {
            return None;
        }
        disk_directory.push(on_disk);
        return Some(DirectoryMismatch {
            path: path.to_string(),
            index_directory: index_directory.to_string_lossy().into_owned(),
            disk_directory: disk_directory.to_string_lossy().into_owned(),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        DirectoryMismatch, colliding_rename_sources, directory_mismatches, remove_work_tree_files,
        restore_target_names, without_colliding_renames,
    };

    fn listing(dir: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn remove_work_tree_files_removes_files_and_ignores_missing_ones() {
        let dir = tempfile::tempdir().unwrap();
        let present = dir.path().join("present.txt");
        let missing = dir.path().join("missing.txt");
        std::fs::write(&present, "x").unwrap();
        let paths = vec![
            missing.to_string_lossy().into_owned(),
            present.to_string_lossy().into_owned(),
        ];
        assert_eq!(remove_work_tree_files(&paths), Ok(()));
        assert!(listing(dir.path()).is_empty());
    }

    #[test]
    fn restore_target_names_renames_an_entry_that_differs_only_by_case() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("rules.md"), "x").unwrap();
        let target = dir.path().join("RULES.md").to_string_lossy().into_owned();
        assert_eq!(restore_target_names(&[target]), Ok(()));
        assert_eq!(listing(dir.path()), ["RULES.md"]);
    }

    #[test]
    fn restore_target_names_keeps_a_correctly_named_entry() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("RULES.md"), "x").unwrap();
        let target = dir.path().join("RULES.md").to_string_lossy().into_owned();
        assert_eq!(restore_target_names(&[target]), Ok(()));
        assert_eq!(listing(dir.path()), ["RULES.md"]);
    }

    fn names(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    /// Whether `LOWER` names the directory `lower` on this filesystem.
    fn is_case_insensitive(root: &std::path::Path) -> bool {
        root.join("LOWER").is_dir()
    }

    #[test]
    fn directory_mismatch_names_the_index_and_disk_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("lower/inner")).unwrap();
        let found = directory_mismatches(
            dir.path(),
            &names(&["LOWER/file.txt", "lower/INNER/file.txt"]),
        );
        if is_case_insensitive(dir.path()) {
            assert_eq!(
                found,
                [
                    DirectoryMismatch {
                        path: "LOWER/file.txt".to_string(),
                        index_directory: "LOWER".to_string(),
                        disk_directory: "lower".to_string(),
                    },
                    DirectoryMismatch {
                        path: "lower/INNER/file.txt".to_string(),
                        index_directory: "lower/INNER".to_string(),
                        disk_directory: "lower/inner".to_string(),
                    },
                ]
            );
        } else {
            assert!(found.is_empty(), "a distinct directory is no alias");
        }
    }

    #[test]
    fn exact_missing_and_top_level_paths_are_no_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("lower/inner")).unwrap();
        let paths = names(&["lower/inner/file.txt", "missing/file.txt", "file.txt"]);
        assert!(directory_mismatches(dir.path(), &paths).is_empty());
    }

    #[test]
    fn a_file_with_the_name_of_the_directory_is_no_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("lower"), "x").unwrap();
        assert!(directory_mismatches(dir.path(), &names(&["LOWER/file.txt"])).is_empty());
    }

    #[test]
    fn colliding_renames_are_not_unstaged() {
        let targets = names(&["dir/RULES.md", "new.txt", "\u{c9}.md"]);
        let sources = names(&["dir/rules.md", "old.txt", "\u{e9}.md"]);
        assert_eq!(
            without_colliding_renames(&targets, &sources),
            names(&["new.txt"])
        );
    }

    #[test]
    fn swapped_renames_with_different_case_are_not_unstaged() {
        let targets = names(&["DIR/b.txt", "DIR/a.txt"]);
        let sources = names(&["dir/a.txt", "dir/b.txt"]);
        assert!(without_colliding_renames(&targets, &sources).is_empty());
    }

    #[test]
    fn upper_to_lower_rename_is_not_unstaged() {
        let targets = names(&["dir/rules.md"]);
        let sources = names(&["dir/RULES.md"]);
        assert!(without_colliding_renames(&targets, &sources).is_empty());
        assert_eq!(colliding_rename_sources(&targets, &sources), sources);
    }

    #[test]
    fn colliding_sources_cover_swaps_only() {
        let targets = names(&["DIR/b.txt", "DIR/a.txt", "x.txt"]);
        let sources = names(&["dir/a.txt", "dir/b.txt", "y.txt"]);
        assert_eq!(
            colliding_rename_sources(&targets, &sources),
            names(&["dir/a.txt", "dir/b.txt"])
        );
    }

    #[test]
    fn unrelated_renames_are_kept() {
        let targets = names(&["b.txt"]);
        let sources = names(&["a.txt"]);
        assert_eq!(without_colliding_renames(&targets, &sources), targets);
    }
}
