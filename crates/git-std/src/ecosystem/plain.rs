//! Plain VERSION file ecosystem.
//!
//! No ecosystem tooling; always uses native string manipulation.
//! No lock file to sync.

use std::path::Path;

use standard_version::{PlainVersionFile, VersionFile};

use super::{Ecosystem, SyncOutcome, WriteOutcome, native_write};

pub struct Plain;

impl Ecosystem for Plain {
    fn name(&self) -> &'static str {
        "plain"
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("VERSION").exists()
    }

    fn version_files(&self) -> &[&str] {
        &["VERSION"]
    }

    fn write_version(&self, root: &Path, new_version: &str) -> WriteOutcome {
        native_write(root, &PlainVersionFile, new_version)
    }

    fn sync_lock(&self, _root: &Path) -> Vec<SyncOutcome> {
        vec![SyncOutcome::NoLockFile]
    }

    fn version_file_engine(&self) -> Option<Box<dyn VersionFile>> {
        Some(Box::new(PlainVersionFile))
    }

    fn is_fallback(&self) -> bool {
        true
    }
}
