//! Rust ecosystem — Cargo.

use std::path::Path;
use std::process::Command;

use standard_version::CargoVersionFile;

use super::{Ecosystem, SyncOutcome, WriteOutcome, cmd, native_write, try_sync};
use crate::ui;

pub struct Rust;

/// Use `git diff --name-only` to discover all files modified by the CLI tool.
fn git_modified_files(root: &Path) -> Vec<std::path::PathBuf> {
    let output = Command::new("git")
        .args(["diff", "--name-only"])
        .current_dir(root)
        .output();

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| root.join(l))
            .collect(),
        _ => vec![root.join("Cargo.toml")],
    }
}

impl Ecosystem for Rust {
    fn name(&self) -> &'static str {
        "rust"
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("Cargo.toml").exists()
    }

    fn version_files(&self) -> &[&str] {
        &["Cargo.toml"]
    }

    fn write_version(&self, root: &Path, new_version: &str) -> WriteOutcome {
        // Try `cargo set-version --workspace <V>` first.
        match cmd::run_tool(root, "cargo", &["set-version", "--workspace", new_version]) {
            Ok(status) if status.success() => {
                // cargo set-version may modify multiple Cargo.toml files
                // in a workspace. Discover all modified files via git.
                WriteOutcome::CliModified {
                    files: git_modified_files(root),
                }
            }
            _ => {
                ui::warning(
                    "cargo set-version not available \u{2014} using built-in version update \
                     (install cargo-edit for full workspace dependency propagation)",
                );
                native_write(root, &CargoVersionFile, new_version)
            }
        }
    }

    fn sync_lock(&self, root: &Path) -> Vec<SyncOutcome> {
        vec![try_sync(root, "Cargo.lock", "cargo", &["update", "--workspace"])]
    }

    fn lock_files(&self) -> &[&str] {
        &["Cargo.lock"]
    }
}
