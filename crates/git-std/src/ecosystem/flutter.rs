//! Flutter/Dart ecosystem.
//!
//! No version-write CLI exists; always uses native string manipulation.
//! No lock file to sync.

use std::path::Path;

use standard_version::{PubspecVersionFile, VersionFile};

use super::{Ecosystem, SyncOutcome, WriteOutcome, native_write};

pub struct Flutter;

impl Ecosystem for Flutter {
    fn name(&self) -> &'static str {
        "flutter"
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("pubspec.yaml").exists()
    }

    fn version_files(&self) -> &[&str] {
        &["pubspec.yaml"]
    }

    fn write_version(&self, root: &Path, new_version: &str) -> WriteOutcome {
        native_write(root, &PubspecVersionFile, new_version)
    }

    fn sync_lock(&self, _root: &Path) -> Vec<SyncOutcome> {
        vec![SyncOutcome::NoLockFile]
    }

    fn version_file_engine(&self) -> Option<Box<dyn VersionFile>> {
        Some(Box::new(PubspecVersionFile))
    }
}
