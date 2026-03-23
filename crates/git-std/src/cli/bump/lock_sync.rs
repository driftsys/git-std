//! Lock file synchronisation after a version bump.
//!
//! Scans the repository root for known ecosystem lock files and runs each
//! ecosystem's official tool to regenerate it. Missing tools and sync failures
//! are reported as warnings but never abort the bump.

use std::path::Path;

use crate::ui;

/// A known lock file entry: filename, the version file that triggers sync,
/// and the tool + args used to regenerate the lock file.
struct LockEntry {
    /// Lock file name to look for at the repo root.
    filename: &'static str,
    /// Version file name that must have been updated to trigger this sync.
    /// Matched via `ends_with` so `crates/foo/Cargo.toml` matches `"Cargo.toml"`.
    trigger: &'static str,
    /// Command name of the required tool (also used for PATH lookup).
    tool: &'static str,
    /// Arguments to pass to the tool.
    args: &'static [&'static str],
}

/// All lock files git-std knows how to sync.
const LOCK_ENTRIES: &[LockEntry] = &[
    LockEntry {
        filename: "Cargo.lock",
        trigger: "Cargo.toml",
        tool: "cargo",
        args: &["update", "--workspace"],
    },
    LockEntry {
        filename: "package-lock.json",
        trigger: "package.json",
        tool: "npm",
        args: &["install", "--package-lock-only"],
    },
    LockEntry {
        filename: "yarn.lock",
        trigger: "package.json",
        tool: "yarn",
        args: &["install", "--mode", "update-lockfile"],
    },
    LockEntry {
        filename: "pnpm-lock.yaml",
        trigger: "package.json",
        tool: "pnpm",
        args: &["install", "--lockfile-only"],
    },
    LockEntry {
        filename: "deno.lock",
        trigger: "deno.json",
        tool: "deno",
        args: &["install"],
    },
    LockEntry {
        filename: "uv.lock",
        trigger: "pyproject.toml",
        tool: "uv",
        args: &["lock"],
    },
    LockEntry {
        filename: "poetry.lock",
        trigger: "pyproject.toml",
        tool: "poetry",
        args: &["lock", "--no-update"],
    },
];

/// Sync ecosystem lock files found at `workdir` whose trigger was updated.
///
/// `updated_names` lists the version file names that were part of the bump
/// (e.g. `"Cargo.toml"`, `"crates/foo/Cargo.toml"`, `"package.json"`).
/// A lock entry is only synced when at least one updated name ends with the
/// entry's trigger AND the lock file exists on disk.
///
/// Returns the list of lock file names that were successfully synced, for
/// staging alongside the version files.
pub(super) fn sync_lock_files(workdir: &Path, updated_names: &[&str]) -> Vec<String> {
    let mut synced = Vec::new();

    for entry in LOCK_ENTRIES {
        let triggered = updated_names
            .iter()
            .any(|name| name.ends_with(entry.trigger));
        if triggered && workdir.join(entry.filename).exists() {
            run_sync(workdir, entry.filename, entry.tool, entry.args, &mut synced);
        }
    }

    synced
}

/// Emit dry-run messages for all lock files that would be synced.
///
/// `updated_names` lists the version file names that would have been updated.
pub(super) fn dry_run_lock_files(workdir: &Path, updated_names: &[&str]) {
    for entry in LOCK_ENTRIES {
        let triggered = updated_names
            .iter()
            .any(|name| name.ends_with(entry.trigger));
        if triggered && workdir.join(entry.filename).exists() {
            ui::info(&format!("Would sync:   {}", entry.filename));
        }
    }
}

/// Try to run `tool args` in `workdir`. On success, push `filename` to
/// `synced` and print an info message. On failure (tool absent or non-zero
/// exit), print a warning and skip staging.
fn run_sync(workdir: &Path, filename: &str, tool: &str, args: &[&str], synced: &mut Vec<String>) {
    match std::process::Command::new(tool)
        .args(args)
        .current_dir(workdir)
        .status()
    {
        Err(_) => {
            // `Err` means the tool binary was not found (or could not be
            // spawned). Treat as "tool not on PATH".
            ui::warning(&format!(
                "{filename} found but {tool} is not on PATH — lock file not synced"
            ));
        }
        Ok(status) if status.success() => {
            ui::info(&format!("Synced:  {filename}"));
            synced.push(filename.to_string());
        }
        Ok(status) => {
            let code = status.code().unwrap_or(-1);
            ui::warning(&format!("{filename} sync failed (exit {code})"));
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// dry_run_lock_files emits no output for an empty directory.
    #[test]
    fn dry_run_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        // Should not panic or emit anything meaningful — we can't assert stderr
        // easily in unit tests, but we verify it doesn't crash.
        dry_run_lock_files(dir.path(), &[]);
        dry_run_lock_files(dir.path(), &["Cargo.toml"]);
    }

    /// sync_lock_files returns empty vec when no lock files are present.
    #[test]
    fn sync_no_lock_files() {
        let dir = tempfile::tempdir().unwrap();
        let synced = sync_lock_files(dir.path(), &[]);
        assert!(synced.is_empty());
    }

    /// Cargo.lock is not synced when Cargo.toml was not updated, even if file exists.
    #[test]
    fn cargo_lock_skipped_when_not_updated() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.lock"), "# placeholder\n").unwrap();
        let synced = sync_lock_files(dir.path(), &[]);
        // We don't expect Cargo.lock to be attempted (no trigger matched).
        assert!(!synced.contains(&"Cargo.lock".to_string()));
    }

    /// dry_run_lock_files mentions Cargo.lock only when Cargo.toml is in updated_names.
    #[test]
    fn dry_run_cargo_lock_conditional() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.lock"), "# placeholder\n").unwrap();
        // Just verify no panic — stderr assertions covered by integration tests.
        dry_run_lock_files(dir.path(), &[]);
        dry_run_lock_files(dir.path(), &["Cargo.toml"]);
    }

    /// Lock files are not synced when their trigger version file was not updated.
    #[test]
    fn lock_file_skipped_when_trigger_not_updated() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("uv.lock"), "# placeholder\n").unwrap();
        // No pyproject.toml in updated_names → uv.lock should not be synced.
        let synced = sync_lock_files(dir.path(), &["Cargo.toml"]);
        assert!(!synced.contains(&"uv.lock".to_string()));
    }
}
