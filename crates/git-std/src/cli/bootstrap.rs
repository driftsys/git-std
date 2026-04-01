//! `git std bootstrap` — post-clone environment setup.
//!
//! Entry point: `run(dry_run)` — detect convention files and configure the local environment.

use std::path::Path;
use std::process::Command;

use crate::ui;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const LFS_INSTALL_URL: &str = "https://git-lfs.github.com";
const BOOTSTRAP_HOOKS_FILE: &str = ".githooks/bootstrap.hooks";

// ---------------------------------------------------------------------------
// git std bootstrap (run)
// ---------------------------------------------------------------------------

/// Run the built-in bootstrap checks and any custom `bootstrap.hooks`.
///
/// Returns the process exit code (`0` success, `1` failure).
pub fn run(dry_run: bool) -> i32 {
    let cwd = std::env::current_dir().unwrap_or_default();
    let root = match crate::git::workdir(&cwd) {
        Ok(r) => r,
        Err(_) => {
            crate::ui::error("not inside a git repository");
            return 1;
        }
    };

    let mut failed = false;

    // Tier 1 — built-in checks
    if !check_hooks_path(&root, dry_run) {
        failed = true;
    }
    if !check_lfs(&root, dry_run) {
        return 1; // hard failure — git-lfs missing
    }
    if !check_blame_ignore_revs(&root, dry_run) {
        failed = true;
    }

    // Tier 2 — custom bootstrap.hooks
    if root.join(BOOTSTRAP_HOOKS_FILE).exists() {
        if dry_run {
            ui::info(&format!("{}  custom bootstrap hooks executed", ui::pass()));
        } else {
            let code = super::hook::run("bootstrap", &[], crate::app::OutputFormat::Text);
            if code != 0 {
                failed = true;
            }
        }
    }

    if failed { 1 } else { 0 }
}

/// Detect `.githooks/` and set `core.hooksPath`.
fn check_hooks_path(root: &Path, dry_run: bool) -> bool {
    let hooks_dir = root.join(".githooks");
    if !hooks_dir.exists() {
        return true;
    }

    if dry_run {
        ui::info(&format!("{}  git hooks configured", ui::pass()));
        return true;
    }

    let status = Command::new("git")
        .args(["config", "core.hooksPath", ".githooks"])
        .status();

    match status {
        Ok(s) if s.success() => {
            ui::info(&format!("{}  git hooks configured", ui::pass()));
            true
        }
        _ => {
            ui::error("failed to set core.hooksPath");
            false
        }
    }
}

/// Detect `filter=lfs` in `.gitattributes` and run LFS setup.
///
/// Returns `false` only when LFS rules are detected but `git-lfs` is not
/// installed — this is a hard failure (exit 1).
fn check_lfs(root: &Path, dry_run: bool) -> bool {
    let attrs = root.join(".gitattributes");
    if !attrs.exists() {
        return true;
    }

    let content = match std::fs::read_to_string(&attrs) {
        Ok(c) => c,
        Err(e) => {
            ui::error(&format!("cannot read .gitattributes: {e}"));
            return false;
        }
    };

    if !content.lines().any(|line| line.contains("filter=lfs")) {
        return true;
    }

    // LFS rules detected — check if git-lfs is installed
    let lfs_available = Command::new("git")
        .args(["lfs", "version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !lfs_available {
        ui::error("git-lfs is required but not installed");
        ui::hint(&format!("install from {LFS_INSTALL_URL}"));
        return false;
    }

    if dry_run {
        ui::info(&format!("{}  LFS objects downloaded", ui::pass()));
        return true;
    }

    // Run git lfs install
    let install_ok = Command::new("git")
        .args(["lfs", "install"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !install_ok {
        ui::error("git lfs install failed");
        return false;
    }

    // Run git lfs pull
    let pull_ok = Command::new("git")
        .args(["lfs", "pull"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !pull_ok {
        ui::error("git lfs pull failed");
        return false;
    }

    ui::info(&format!("{}  LFS objects downloaded", ui::pass()));
    true
}

/// Detect `.git-blame-ignore-revs` and set `blame.ignoreRevsFile`.
fn check_blame_ignore_revs(root: &Path, dry_run: bool) -> bool {
    let path = root.join(".git-blame-ignore-revs");
    if !path.exists() {
        return true;
    }

    if dry_run {
        ui::info(&format!("{}  blame ignore revs configured", ui::pass()));
        return true;
    }

    let status = Command::new("git")
        .args(["config", "blame.ignoreRevsFile", ".git-blame-ignore-revs"])
        .status();

    match status {
        Ok(s) if s.success() => {
            ui::info(&format!("{}  blame ignore revs configured", ui::pass()));
            true
        }
        _ => {
            ui::error("failed to set blame.ignoreRevsFile");
            false
        }
    }
}
