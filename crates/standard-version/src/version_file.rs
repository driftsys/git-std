//! Version file detection and updating.
//!
//! Provides the [`VersionFile`] trait for ecosystem-specific version file
//! engines, and the [`update_version_files`] function that discovers and
//! updates version files at a repository root.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::cargo::CargoVersionFile;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors that can occur when reading or writing version files.
#[derive(Debug)]
pub enum VersionFileError {
    /// The expected file was not found on disk.
    FileNotFound(PathBuf),
    /// The file does not contain a version field this engine can handle.
    NoVersionField,
    /// Writing the updated content back to disk failed.
    WriteFailed(std::io::Error),
    /// Reading the file from disk failed.
    ReadFailed(std::io::Error),
}

impl fmt::Display for VersionFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileNotFound(p) => write!(f, "file not found: {}", p.display()),
            Self::NoVersionField => write!(f, "no version field found"),
            Self::WriteFailed(e) => write!(f, "write failed: {e}"),
            Self::ReadFailed(e) => write!(f, "read failed: {e}"),
        }
    }
}

impl std::error::Error for VersionFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::WriteFailed(e) | Self::ReadFailed(e) => Some(e),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// A version file engine that can detect, read, and write a version field
/// inside a specific file format (e.g. `Cargo.toml`, `package.json`).
pub trait VersionFile {
    /// Human-readable name (e.g. `"Cargo.toml"`).
    fn name(&self) -> &str;

    /// Filenames to look for at the repository root.
    fn filenames(&self) -> &[&str];

    /// Check if `content` contains a version field this engine handles.
    fn detect(&self, content: &str) -> bool;

    /// Extract the current version string from file content.
    fn read_version(&self, content: &str) -> Option<String>;

    /// Return updated file content with `new_version` replacing the old value.
    fn write_version(&self, content: &str, new_version: &str) -> Result<String, VersionFileError>;
}

// ---------------------------------------------------------------------------
// UpdateResult
// ---------------------------------------------------------------------------

/// The outcome of updating a single version file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateResult {
    /// Absolute path to the file that was updated.
    pub path: PathBuf,
    /// Human-readable engine name (e.g. `"Cargo.toml"`).
    pub name: String,
    /// Version string before the update.
    pub old_version: String,
    /// Version string after the update.
    pub new_version: String,
    /// Optional extra info (e.g. `"VERSION_CODE: 42 → 43"`).
    pub extra: Option<String>,
}

// ---------------------------------------------------------------------------
// CustomVersionFile (placeholder for story #103)
// ---------------------------------------------------------------------------

/// A user-defined version file matched by path and regex.
///
/// The regex engine itself will be implemented in story #103. This struct
/// is defined now so that the public API signature of
/// [`update_version_files`] is stable.
#[derive(Debug, Clone)]
pub struct CustomVersionFile {
    /// Path to the file, relative to the repository root.
    pub path: PathBuf,
    /// Regex pattern whose first capture group contains the version string.
    pub pattern: String,
}

// ---------------------------------------------------------------------------
// update_version_files
// ---------------------------------------------------------------------------

/// Discover and update version files at `root`.
///
/// Iterates all built-in version file engines (currently only
/// [`CargoVersionFile`]) and, for each file that is detected, replaces the
/// version string with `new_version`. Updated content is written back to
/// disk.
///
/// `_custom_files` is accepted for forward compatibility (story #103) but
/// is not yet processed.
///
/// # Errors
///
/// Returns a [`VersionFileError`] if a detected file cannot be read or
/// written.
pub fn update_version_files(
    root: &Path,
    new_version: &str,
    _custom_files: &[CustomVersionFile],
) -> Result<Vec<UpdateResult>, VersionFileError> {
    let engines: Vec<Box<dyn VersionFile>> = vec![Box::new(CargoVersionFile)];

    let mut results = Vec::new();

    for engine in &engines {
        for filename in engine.filenames() {
            let path = root.join(filename);
            if !path.exists() {
                continue;
            }

            let content = fs::read_to_string(&path).map_err(VersionFileError::ReadFailed)?;

            if !engine.detect(&content) {
                continue;
            }

            let old_version = match engine.read_version(&content) {
                Some(v) => v,
                None => continue,
            };

            let updated = engine.write_version(&content, new_version)?;
            fs::write(&path, &updated).map_err(VersionFileError::WriteFailed)?;

            results.push(UpdateResult {
                path,
                name: engine.name().to_string(),
                old_version,
                new_version: new_version.to_string(),
                extra: None,
            });
        }
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn update_version_files_updates_cargo_toml() {
        let dir = tempfile::tempdir().unwrap();
        let cargo_toml = dir.path().join("Cargo.toml");
        fs::write(
            &cargo_toml,
            r#"[package]
name = "example"
version = "0.1.0"
edition = "2024"
"#,
        )
        .unwrap();

        let results = update_version_files(dir.path(), "2.0.0", &[]).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].old_version, "0.1.0");
        assert_eq!(results[0].new_version, "2.0.0");
        assert_eq!(results[0].name, "Cargo.toml");
        assert_eq!(results[0].path, cargo_toml);

        let on_disk = fs::read_to_string(&cargo_toml).unwrap();
        assert!(on_disk.contains("version = \"2.0.0\""));
    }

    #[test]
    fn update_version_files_skips_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        // No Cargo.toml present.
        let results = update_version_files(dir.path(), "1.0.0", &[]).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn update_version_files_skips_undetected() {
        let dir = tempfile::tempdir().unwrap();
        let cargo_toml = dir.path().join("Cargo.toml");
        // File exists but has no [package] section.
        fs::write(&cargo_toml, "[dependencies]\nfoo = \"1\"\n").unwrap();

        let results = update_version_files(dir.path(), "1.0.0", &[]).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn error_display() {
        let err = VersionFileError::NoVersionField;
        assert_eq!(err.to_string(), "no version field found");

        let err = VersionFileError::FileNotFound(PathBuf::from("/tmp/gone"));
        assert!(err.to_string().contains("/tmp/gone"));
    }
}
