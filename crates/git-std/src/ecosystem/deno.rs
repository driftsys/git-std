//! Deno ecosystem.
//!
//! No version-write CLI exists; always uses native string manipulation.

use std::path::Path;

use standard_version::{DenoVersionFile, VersionFile};

use super::{Ecosystem, SyncOutcome, WriteOutcome, native_write, try_sync};

pub struct Deno;

impl Ecosystem for Deno {
    fn name(&self) -> &'static str {
        "deno"
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("deno.json").exists() || root.join("deno.jsonc").exists()
    }

    fn version_files(&self) -> &[&str] {
        &["deno.json", "deno.jsonc"]
    }

    fn write_version(&self, root: &Path, new_version: &str) -> WriteOutcome {
        native_write(root, &DenoVersionFile, new_version)
    }

    fn sync_lock(&self, root: &Path) -> Vec<SyncOutcome> {
        vec![try_sync(
            root,
            "deno.lock",
            "deno",
            &["install", "--frozen=false"],
        )]
    }

    fn lock_files(&self) -> &[&str] {
        &["deno.lock"]
    }

    fn version_file_engine(&self) -> Option<Box<dyn VersionFile>> {
        Some(Box::new(DenoVersionFile))
    }
}
