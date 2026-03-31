//! Version file detection and updating.
//!
//! Provides the [`VersionFile`] trait for ecosystem-specific version file
//! engines, and the [`update_version_files`] / [`detect_version_files`]
//! functions (in the [`scan`](crate::scan) module) that discover and
//! update version files at a repository root.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors that can occur when reading or writing version files.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum VersionFileError {
    /// The expected file was not found on disk.
    #[error("file not found: {}", .0.display())]
    FileNotFound(PathBuf),
    /// The file does not contain a version field this engine can handle.
    #[error("no version field found")]
    NoVersionField,
    /// Writing the updated content back to disk failed.
    #[error("write failed: {0}")]
    WriteFailed(#[source] std::io::Error),
    /// Reading the file from disk failed.
    #[error("read failed: {0}")]
    ReadFailed(#[source] std::io::Error),
    /// A user-supplied regex pattern is invalid or has no capture groups.
    #[error("invalid regex: {0}")]
    InvalidRegex(String),
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

    /// Compare old and new file content and return optional extra information
    /// about side-effects (e.g. `VERSION_CODE` increment in gradle).
    ///
    /// The default implementation returns `None`.
    fn extra_info(&self, _old_content: &str, _new_content: &str) -> Option<String> {
        None
    }
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
// DetectedFile
// ---------------------------------------------------------------------------

/// Information about a detected version file (no writes performed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedFile {
    /// Absolute path to the file.
    pub path: PathBuf,
    /// Human-readable engine name (e.g. `"Cargo.toml"`).
    pub name: String,
    /// Current version string in the file.
    pub old_version: String,
}

// ---------------------------------------------------------------------------
// CustomVersionFile
// ---------------------------------------------------------------------------

/// A user-defined version file matched by path and regex.
///
/// Processed by [`RegexVersionFile`](crate::regex_engine::RegexVersionFile)
/// during [`update_version_files`](crate::scan::update_version_files).
#[derive(Debug, Clone)]
pub struct CustomVersionFile {
    /// Path to the file, relative to the repository root.
    pub path: PathBuf,
    /// Regex pattern whose first capture group contains the version string.
    pub pattern: String,
}

// Re-export scan functions so existing `use version_file::update_version_files`
// paths continue to work.
pub use crate::scan::{detect_version_files, update_version_files};
