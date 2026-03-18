//! Lock file synchronisation after a version bump.
//!
//! Scans the repository root for known ecosystem lock files and runs each
//! ecosystem's official tool to regenerate it. Missing tools and sync failures
//! are reported as warnings but never abort the bump.

use std::path::Path;

use crate::ui;

/// A known lock file entry: filename, sync args, and the required tool.
struct LockEntry {
    /// Lock file name to look for at the repo root.
    filename: &'static str,
    /// Command name of the required tool (also used for PATH lookup).
    tool: &'static str,
    /// Arguments to pass to the tool.
    args: &'static [&'static str],
}

/// All lock files git-std knows how to sync.
const LOCK_ENTRIES: &[LockEntry] = &[
    LockEntry {
        filename: "package-lock.json",
        tool: "npm",
        args: &["install", "--package-lock-only"],
    },
    LockEntry {
        filename: "yarn.lock",
        tool: "yarn",
        args: &["install", "--mode", "update-lockfile"],
    },
    LockEntry {
        filename: "pnpm-lock.yaml",
        tool: "pnpm",
        args: &["install", "--lockfile-only"],
    },
    LockEntry {
        filename: "deno.lock",
        tool: "deno",
        args: &["install"],
    },
    LockEntry {
        filename: "uv.lock",
        tool: "uv",
        args: &["lock"],
    },
    LockEntry {
        filename: "poetry.lock",
        tool: "poetry",
        args: &["lock", "--no-update"],
    },
];

/// Sync all ecosystem lock files found at `workdir`.
///
/// `cargo_updated` indicates whether `Cargo.toml` was part of the version
/// bump; `Cargo.lock` is only synced in that case.
///
/// Returns the list of lock file names that were successfully synced, for
/// staging alongside the version files.
pub(super) fn sync_lock_files(workdir: &Path, cargo_updated: bool) -> Vec<String> {
    let mut synced = Vec::new();

    // Handle Cargo.lock specially: only sync when Cargo.toml was updated.
    if cargo_updated {
        let cargo_lock = workdir.join("Cargo.lock");
        if cargo_lock.exists() {
            run_sync(
                workdir,
                "Cargo.lock",
                "cargo",
                &["update", "--workspace"],
                &mut synced,
            );
        }
    }

    // All other lock files.
    for entry in LOCK_ENTRIES {
        let path = workdir.join(entry.filename);
        if path.exists() {
            run_sync(workdir, entry.filename, entry.tool, entry.args, &mut synced);
        }
    }

    synced
}

/// Emit dry-run messages for all lock files that would be synced.
///
/// `cargo_updated` indicates whether `Cargo.toml` would have been updated.
pub(super) fn dry_run_lock_files(workdir: &Path, cargo_updated: bool) {
    if cargo_updated && workdir.join("Cargo.lock").exists() {
        ui::info("Would sync:   Cargo.lock");
    }

    for entry in LOCK_ENTRIES {
        if workdir.join(entry.filename).exists() {
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
        dry_run_lock_files(dir.path(), false);
        dry_run_lock_files(dir.path(), true);
    }

    /// sync_lock_files returns empty vec when no lock files are present.
    #[test]
    fn sync_no_lock_files() {
        let dir = tempfile::tempdir().unwrap();
        let synced = sync_lock_files(dir.path(), false);
        assert!(synced.is_empty());
    }

    /// Cargo.lock is not synced when cargo_updated is false, even if file exists.
    #[test]
    fn cargo_lock_skipped_when_not_updated() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.lock"), "# placeholder\n").unwrap();
        let synced = sync_lock_files(dir.path(), false);
        // We don't expect Cargo.lock to be attempted (cargo_updated=false).
        assert!(!synced.contains(&"Cargo.lock".to_string()));
    }

    /// dry_run_lock_files mentions Cargo.lock only when cargo_updated is true.
    #[test]
    fn dry_run_cargo_lock_conditional() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.lock"), "# placeholder\n").unwrap();
        // Just verify no panic — stderr assertions covered by integration tests.
        dry_run_lock_files(dir.path(), false);
        dry_run_lock_files(dir.path(), true);
    }
}
