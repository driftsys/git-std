//! Rust ecosystem — Cargo.

use std::path::Path;

use standard_version::CargoVersionFile;

use super::{Ecosystem, SyncOutcome, WriteOutcome, cmd, native_write, try_sync};
use crate::ui;

pub struct Rust;

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
                // cargo set-version may modify multiple Cargo.toml files.
                // Report the root; the caller stages all modified files.
                WriteOutcome::CliModified {
                    files: vec![root.join("Cargo.toml")],
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
}
