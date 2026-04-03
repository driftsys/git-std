//! Project manifest ecosystem.
//!
//! Supports `project.toml`, `project.json`, and `project.yaml` — the
//! driftsys project manifest format. No ecosystem tooling; always uses
//! native string manipulation. No lock file to sync.

use std::path::Path;

use standard_version::{
    ProjectJsonVersionFile, ProjectTomlVersionFile, ProjectYamlVersionFile, VersionFile,
    VersionFileError,
};

use super::{Ecosystem, SyncOutcome, WriteOutcome, native_write};

pub struct Project;

impl Ecosystem for Project {
    fn name(&self) -> &'static str {
        "project"
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("project.toml").exists()
            || root.join("project.json").exists()
            || root.join("project.yaml").exists()
    }

    fn version_files(&self) -> &[&str] {
        &["project.toml", "project.json", "project.yaml"]
    }

    fn write_version(&self, root: &Path, new_version: &str) -> WriteOutcome {
        let engines: [&dyn VersionFile; 3] = [
            &ProjectTomlVersionFile,
            &ProjectJsonVersionFile,
            &ProjectYamlVersionFile,
        ];
        let mut all_results = Vec::new();
        for engine in engines {
            if let WriteOutcome::Fallback { results } = native_write(root, engine, new_version) {
                all_results.extend(results);
            }
        }
        if all_results.is_empty() {
            WriteOutcome::NotDetected
        } else {
            WriteOutcome::Fallback {
                results: all_results,
            }
        }
    }

    fn sync_lock(&self, _root: &Path) -> Vec<SyncOutcome> {
        vec![SyncOutcome::NoLockFile]
    }

    fn version_file_engine(&self) -> Option<Box<dyn VersionFile>> {
        Some(Box::new(ProjectMultiVersionFile))
    }
}

/// Combined version file engine used for dry-run detection across all three
/// project manifest formats.
///
/// Detection and reading dispatch by content, since TOML, JSON, and YAML
/// patterns are mutually exclusive.
struct ProjectMultiVersionFile;

impl VersionFile for ProjectMultiVersionFile {
    fn name(&self) -> &str {
        "project"
    }

    fn filenames(&self) -> &[&str] {
        &["project.toml", "project.json", "project.yaml"]
    }

    fn detect(&self, content: &str) -> bool {
        ProjectTomlVersionFile.detect(content)
            || ProjectJsonVersionFile.detect(content)
            || ProjectYamlVersionFile.detect(content)
    }

    fn read_version(&self, content: &str) -> Option<String> {
        ProjectTomlVersionFile
            .read_version(content)
            .or_else(|| ProjectJsonVersionFile.read_version(content))
            .or_else(|| ProjectYamlVersionFile.read_version(content))
    }

    fn write_version(&self, content: &str, new_version: &str) -> Result<String, VersionFileError> {
        if ProjectTomlVersionFile.detect(content) {
            return ProjectTomlVersionFile.write_version(content, new_version);
        }
        if ProjectJsonVersionFile.detect(content) {
            return ProjectJsonVersionFile.write_version(content, new_version);
        }
        if ProjectYamlVersionFile.detect(content) {
            return ProjectYamlVersionFile.write_version(content, new_version);
        }
        Err(VersionFileError::NoVersionField)
    }
}
