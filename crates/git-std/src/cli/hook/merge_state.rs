use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

const MERGE_FILES: &[&str] = &[
    "MERGE_HEAD",
    "MERGE_MSG",
    "MERGE_MODE",
    "AUTO_MERGE",
    "MERGE_AUTOSTASH",
    "MERGE_RR",
    "SQUASH_MSG",
];

/// Preserve the current worktree's merge metadata around an internal stash operation.
pub(super) fn preserve<T>(operation: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let output = Command::new("git")
        .args(["rev-parse", "--absolute-git-dir"])
        .output()
        .map_err(|error| format!("failed to locate merge metadata: {error}"))?;
    if !output.status.success() {
        return Err("failed to locate merge metadata".to_owned());
    }
    let directory = String::from_utf8(output.stdout)
        .map_err(|error| format!("invalid Git directory path: {error}"))?;
    preserve_in(
        Path::new(directory.trim_end_matches(['\r', '\n'])),
        operation,
    )
}

fn preserve_in<T>(
    directory: &Path,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let mut state = MergeState::capture(directory)?;
    let result = operation();
    match (result, state.restore(true)) {
        (result, Ok(())) => result,
        (Ok(_), Err(restore)) => Err(restore),
        (Err(original), Err(restore)) => Err(format!("{original}; {restore}")),
    }
}

struct MergeState {
    directory: PathBuf,
    backup: Option<PathBuf>,
    moved: Vec<&'static str>,
}

impl MergeState {
    fn capture(directory: &Path) -> Result<Self, String> {
        let mut present = Vec::new();
        for &name in MERGE_FILES {
            let path = directory.join(name);
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.is_file() => present.push(name),
                Ok(_) => {
                    return Err(format!(
                        "merge metadata is not a regular file: {}",
                        path.display()
                    ));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                Err(error) => return Err(format!("failed to inspect {}: {error}", path.display())),
            }
        }
        let mut state = Self {
            directory: directory.to_owned(),
            backup: None,
            moved: Vec::new(),
        };
        if present.is_empty() {
            return Ok(state);
        }
        // Keep the directory on abnormal termination and on any restoration failure.
        let backup = tempfile::Builder::new()
            .prefix("git-std-merge-state-")
            .tempdir_in(directory)
            .map_err(|error| format!("failed to protect merge metadata: {error}"))?
            .keep();
        state.backup = Some(backup.clone());
        for name in present {
            if let Err(error) = fs::rename(directory.join(name), backup.join(name)) {
                let original = format!("failed to protect {name}: {error}");
                return Err(match state.restore(false) {
                    Ok(()) => original,
                    Err(restore) => format!("{original}; {restore}"),
                });
            }
            state.moved.push(name);
        }
        Ok(state)
    }

    fn restore(&mut self, did_run: bool) -> Result<(), String> {
        let Some(backup) = &self.backup else {
            return Ok(());
        };
        let mut failures = Vec::new();
        // Stash apply can write AUTO_MERGE even when the original merge had none.
        if did_run
            && !self.moved.contains(&"AUTO_MERGE")
            && let Err(error) = remove_generated_auto_merge(&self.directory.join("AUTO_MERGE"))
        {
            failures.push(error);
        }
        for &name in &self.moved {
            let target = self.directory.join(name);
            let result = (|| {
                if did_run && name == "AUTO_MERGE" {
                    remove_generated_auto_merge(&target)?;
                }
                match fs::symlink_metadata(&target) {
                    Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                    Ok(_) => return Err(format!("refusing to replace {}", target.display())),
                    Err(error) => {
                        return Err(format!("failed to inspect {}: {error}", target.display()));
                    }
                }
                fs::rename(backup.join(name), &target)
                    .map_err(|error| format!("failed to restore {name}: {error}"))
            })();
            if let Err(error) = result {
                failures.push(error);
            }
        }
        if failures.is_empty()
            && let Err(error) = fs::remove_dir(backup)
        {
            failures.push(format!("failed to remove recovery directory: {error}"));
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "{}; merge metadata recovery directory: {}",
                failures.join("; "),
                backup.display()
            ))
        }
    }
}

fn remove_generated_auto_merge(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => fs::remove_file(path)
            .map_err(|error| format!("failed to remove generated AUTO_MERGE: {error}")),
        Ok(_) => Err(format!("refusing to replace {}", path.display())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("failed to inspect {}: {error}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_all_metadata_on_success_and_failure() {
        for fails in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let names = [
                "MERGE_HEAD",
                "MERGE_MSG",
                "MERGE_MODE",
                "AUTO_MERGE",
                "MERGE_AUTOSTASH",
                "MERGE_RR",
                "SQUASH_MSG",
            ];
            for name in names {
                fs::write(directory.path().join(name), name.as_bytes()).unwrap();
            }
            let result = preserve_in(directory.path(), || {
                for name in names {
                    assert!(!directory.path().join(name).exists());
                }
                if fails {
                    Err("operation failed".to_owned())
                } else {
                    Ok(())
                }
            });
            assert_eq!(result.is_err(), fails);
            for name in names {
                assert_eq!(
                    fs::read(directory.path().join(name)).unwrap(),
                    name.as_bytes()
                );
            }
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), names.len());
        }
    }

    #[test]
    fn absent_metadata_needs_no_backup() {
        let directory = tempfile::tempdir().unwrap();
        preserve_in(directory.path(), || {
            assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn invalid_metadata_is_rejected_before_moving_any_file() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("MERGE_HEAD"), "parent").unwrap();
        fs::create_dir(directory.path().join("MERGE_MSG")).unwrap();
        assert!(
            preserve_in(directory.path(), || -> Result<(), String> {
                panic!("must not run")
            })
            .is_err()
        );
        assert_eq!(
            fs::read_to_string(directory.path().join("MERGE_HEAD")).unwrap(),
            "parent"
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
    }

    #[test]
    fn collision_retains_backup_and_restores_other_files() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("MERGE_HEAD"), "original").unwrap();
        fs::write(directory.path().join("MERGE_MSG"), "message").unwrap();
        let error = preserve_in(directory.path(), || -> Result<(), String> {
            fs::write(directory.path().join("MERGE_HEAD"), "unexpected").unwrap();
            Err("operation failed".to_owned())
        })
        .unwrap_err();
        let backup = fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.is_dir())
            .unwrap();
        assert!(error.contains("operation failed"));
        assert!(error.contains(backup.to_str().unwrap()));
        assert_eq!(
            fs::read_to_string(backup.join("MERGE_HEAD")).unwrap(),
            "original"
        );
        assert_eq!(
            fs::read_to_string(directory.path().join("MERGE_HEAD")).unwrap(),
            "unexpected"
        );
        assert_eq!(
            fs::read_to_string(directory.path().join("MERGE_MSG")).unwrap(),
            "message"
        );
    }

    #[test]
    fn restores_auto_merge_over_generated_regular_file() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("AUTO_MERGE"), "original").unwrap();
        preserve_in(directory.path(), || {
            fs::write(directory.path().join("AUTO_MERGE"), "generated").unwrap();
            Ok(())
        })
        .unwrap();
        assert_eq!(
            fs::read_to_string(directory.path().join("AUTO_MERGE")).unwrap(),
            "original"
        );
    }

    #[test]
    fn removes_generated_auto_merge_when_original_merge_had_none() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("MERGE_HEAD"), "parent").unwrap();
        preserve_in(directory.path(), || {
            fs::write(directory.path().join("AUTO_MERGE"), "generated").unwrap();
            Ok(())
        })
        .unwrap();
        assert!(!directory.path().join("AUTO_MERGE").exists());
        assert_eq!(
            fs::read_to_string(directory.path().join("MERGE_HEAD")).unwrap(),
            "parent"
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_metadata_symlink_without_touching_its_target() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target");
        fs::write(&target, "untouched").unwrap();
        std::os::unix::fs::symlink(&target, directory.path().join("MERGE_HEAD")).unwrap();
        assert!(
            preserve_in(directory.path(), || -> Result<(), String> {
                panic!("must not run")
            })
            .is_err()
        );
        assert_eq!(fs::read_to_string(target).unwrap(), "untouched");
    }

    #[test]
    fn refuses_auto_merge_directory_and_preserves_backup() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("AUTO_MERGE"), "original").unwrap();
        let error = preserve_in(directory.path(), || {
            fs::create_dir(directory.path().join("AUTO_MERGE")).unwrap();
            Ok(())
        })
        .unwrap_err();
        assert!(error.contains("AUTO_MERGE"));
        let backup = fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("git-std-merge-state-")
            })
            .unwrap();
        assert_eq!(
            fs::read_to_string(backup.join("AUTO_MERGE")).unwrap(),
            "original"
        );
    }
}
