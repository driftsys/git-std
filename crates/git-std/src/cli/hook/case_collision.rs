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
/// `core.ignorecase`. Only ASCII-safe paths are handled: Git quotes
/// non-ASCII paths in this output, which this function does not decode
/// (tracked in #606).
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

#[cfg(test)]
mod tests {
    use super::{
        colliding_rename_sources, remove_work_tree_files, restore_target_names,
        without_colliding_renames,
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
