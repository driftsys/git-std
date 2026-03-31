//! Gradle ecosystem.
//!
//! No version-write CLI exists; always uses native string manipulation.
//! No lock file to sync.

use std::path::Path;

use standard_version::{GradleVersionFile, VersionFile};

use super::{Ecosystem, SyncOutcome, WriteOutcome, native_write};

pub struct Gradle;

impl Ecosystem for Gradle {
    fn name(&self) -> &'static str {
        "gradle"
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("gradle.properties").exists()
    }

    fn version_files(&self) -> &[&str] {
        &["gradle.properties"]
    }

    fn write_version(&self, root: &Path, new_version: &str) -> WriteOutcome {
        native_write(root, &GradleVersionFile, new_version)
    }

    fn sync_lock(&self, _root: &Path) -> Vec<SyncOutcome> {
        vec![SyncOutcome::NoLockFile]
    }

    fn version_file_engine(&self) -> Option<Box<dyn VersionFile>> {
        Some(Box::new(GradleVersionFile))
    }
}
