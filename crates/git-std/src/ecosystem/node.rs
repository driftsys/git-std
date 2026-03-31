//! Node ecosystem — npm / yarn / pnpm.
//!
//! Package manager is auto-detected via lock file presence.

use std::path::Path;

use standard_version::{JsonVersionFile, VersionFile};

use super::{Ecosystem, SyncOutcome, WriteOutcome, cmd, native_write, try_sync};
use crate::ui;

pub struct Node;

/// Detected Node package manager.
enum Pm {
    Npm,
    Yarn,
    Pnpm,
}

fn detect_pm(root: &Path) -> Pm {
    if root.join("pnpm-lock.yaml").exists() {
        Pm::Pnpm
    } else if root.join("yarn.lock").exists() {
        Pm::Yarn
    } else {
        Pm::Npm
    }
}

impl Ecosystem for Node {
    fn name(&self) -> &'static str {
        "node"
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("package.json").exists()
    }

    fn version_files(&self) -> &[&str] {
        &["package.json"]
    }

    fn write_version(&self, root: &Path, new_version: &str) -> WriteOutcome {
        match detect_pm(root) {
            Pm::Npm => try_npm_version(root, new_version),
            Pm::Yarn => try_yarn_version(root, new_version),
            Pm::Pnpm => {
                // pnpm has no version-write CLI.
                native_write(root, &JsonVersionFile, new_version)
            }
        }
    }

    fn sync_lock(&self, root: &Path) -> Vec<SyncOutcome> {
        match detect_pm(root) {
            Pm::Npm => vec![try_sync(
                root,
                "package-lock.json",
                "npm",
                &["install", "--package-lock-only"],
            )],
            Pm::Yarn => vec![try_sync(
                root,
                "yarn.lock",
                "yarn",
                &["install", "--mode", "update-lockfile"],
            )],
            Pm::Pnpm => vec![try_sync(
                root,
                "pnpm-lock.yaml",
                "pnpm",
                &["install", "--lockfile-only"],
            )],
        }
    }

    fn lock_files(&self) -> &[&str] {
        &["package-lock.json", "yarn.lock", "pnpm-lock.yaml"]
    }

    fn version_file_engine(&self) -> Option<Box<dyn VersionFile>> {
        Some(Box::new(JsonVersionFile))
    }
}

fn try_npm_version(root: &Path, new_version: &str) -> WriteOutcome {
    match cmd::run_tool(
        root,
        "npm",
        &["version", new_version, "--no-git-tag-version"],
    ) {
        Ok(status) if status.success() => WriteOutcome::CliModified {
            files: vec![root.join("package.json")],
        },
        _ => {
            ui::warning("npm version not available \u{2014} using built-in version update");
            native_write(root, &JsonVersionFile, new_version)
        }
    }
}

fn try_yarn_version(root: &Path, new_version: &str) -> WriteOutcome {
    match cmd::run_tool(
        root,
        "yarn",
        &[
            "version",
            "--new-version",
            new_version,
            "--no-git-tag-version",
        ],
    ) {
        Ok(status) if status.success() => WriteOutcome::CliModified {
            files: vec![root.join("package.json")],
        },
        _ => {
            ui::warning("yarn version not available \u{2014} using built-in version update");
            native_write(root, &JsonVersionFile, new_version)
        }
    }
}
