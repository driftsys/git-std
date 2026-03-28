//! Python ecosystem — uv / poetry.
//!
//! Package manager is auto-detected via lock file presence.

use std::path::Path;

use standard_version::PyprojectVersionFile;

use super::{Ecosystem, SyncOutcome, WriteOutcome, cmd, native_write, try_sync};
use crate::ui;

pub struct Python;

/// Detected Python package manager.
enum Pm {
    Uv,
    Poetry,
    Unknown,
}

fn detect_pm(root: &Path) -> Pm {
    if root.join("uv.lock").exists() {
        Pm::Uv
    } else if root.join("poetry.lock").exists() {
        Pm::Poetry
    } else {
        Pm::Unknown
    }
}

impl Ecosystem for Python {
    fn name(&self) -> &'static str {
        "python"
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("pyproject.toml").exists()
    }

    fn version_files(&self) -> &[&str] {
        &["pyproject.toml"]
    }

    fn write_version(&self, root: &Path, new_version: &str) -> WriteOutcome {
        match detect_pm(root) {
            Pm::Poetry => try_poetry_version(root, new_version),
            _ => native_write(root, &PyprojectVersionFile, new_version),
        }
    }

    fn sync_lock(&self, root: &Path) -> Vec<SyncOutcome> {
        match detect_pm(root) {
            Pm::Uv => vec![try_sync(root, "uv.lock", "uv", &["lock"])],
            Pm::Poetry => vec![try_sync(
                root,
                "poetry.lock",
                "poetry",
                &["lock", "--no-update"],
            )],
            Pm::Unknown => vec![SyncOutcome::NoLockFile],
        }
    }
}

fn try_poetry_version(root: &Path, new_version: &str) -> WriteOutcome {
    match cmd::run_tool(root, "poetry", &["version", new_version]) {
        Ok(status) if status.success() => WriteOutcome::CliModified {
            files: vec![root.join("pyproject.toml")],
        },
        _ => {
            ui::warning(
                "poetry version not available \u{2014} using built-in version update",
            );
            native_write(root, &PyprojectVersionFile, new_version)
        }
    }
}
