//! Plain `VERSION` file engine.
//!
//! Implements [`VersionFile`] for projects that store the version string as
//! the sole content of a `VERSION` file.

use crate::version_file::{VersionFile, VersionFileError};

/// Version file engine for plain `VERSION` files.
///
/// Expects the file to contain nothing but a version string (optionally
/// followed by a trailing newline).
#[derive(Debug, Clone, Copy)]
pub struct PlainVersionFile;

/// Maximum length (in bytes) for a `VERSION` file to be considered valid.
const MAX_VERSION_LEN: usize = 64;

impl VersionFile for PlainVersionFile {
    fn name(&self) -> &str {
        "VERSION"
    }

    fn filenames(&self) -> &[&str] {
        &["VERSION"]
    }

    fn detect(&self, content: &str) -> bool {
        let trimmed = content.trim();
        if trimmed.is_empty() || trimmed.len() > MAX_VERSION_LEN {
            return false;
        }
        // Reject content with multiple lines (not a plain version file).
        if trimmed.contains('\n') {
            return false;
        }
        // Require at least one dot (version-like: X.Y or X.Y.Z).
        if !trimmed.contains('.') {
            return false;
        }
        // Reject content with characters unlikely in a version string.
        trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '+')
    }

    fn read_version(&self, content: &str) -> Option<String> {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return None;
        }
        Some(trimmed.to_string())
    }

    fn write_version(&self, _content: &str, new_version: &str) -> Result<String, VersionFileError> {
        Ok(format!("{new_version}\n"))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- detect ---

    #[test]
    fn detect_positive_semver() {
        assert!(PlainVersionFile.detect("1.2.3\n"));
    }

    #[test]
    fn detect_positive_prerelease() {
        assert!(PlainVersionFile.detect("1.0.0-rc.1\n"));
    }

    #[test]
    fn detect_positive_build_metadata() {
        assert!(PlainVersionFile.detect("1.0.0+build.42\n"));
    }

    #[test]
    fn detect_negative_empty() {
        assert!(!PlainVersionFile.detect(""));
        assert!(!PlainVersionFile.detect("  \n"));
    }

    #[test]
    fn detect_negative_multiline() {
        assert!(!PlainVersionFile.detect("1.0.0\nsome other stuff\n"));
    }

    #[test]
    fn detect_negative_binary_garbage() {
        assert!(!PlainVersionFile.detect("\x00\x01\x02\x03"));
    }

    #[test]
    fn detect_negative_too_long() {
        let long = "a".repeat(MAX_VERSION_LEN + 1);
        assert!(!PlainVersionFile.detect(&long));
    }

    #[test]
    fn detect_negative_special_characters() {
        assert!(!PlainVersionFile.detect("1.0.0; rm -rf /\n"));
    }

    #[test]
    fn detect_negative_bare_word() {
        assert!(!PlainVersionFile.detect("latest\n"));
        assert!(!PlainVersionFile.detect("stable\n"));
    }

    // --- read_version ---

    #[test]
    fn read_version_basic() {
        assert_eq!(
            PlainVersionFile.read_version("1.2.3\n"),
            Some("1.2.3".to_string()),
        );
    }

    #[test]
    fn read_version_with_whitespace() {
        assert_eq!(
            PlainVersionFile.read_version("  1.2.3  \n"),
            Some("1.2.3".to_string()),
        );
    }

    #[test]
    fn read_version_empty() {
        assert_eq!(PlainVersionFile.read_version(""), None);
        assert_eq!(PlainVersionFile.read_version("  \n"), None);
    }

    // --- write_version ---

    #[test]
    fn write_version_overwrites_entirely() {
        let result = PlainVersionFile.write_version("1.2.3\n", "2.0.0").unwrap();
        assert_eq!(result, "2.0.0\n");
    }

    #[test]
    fn write_version_always_has_trailing_newline() {
        let result = PlainVersionFile.write_version("1.2.3", "2.0.0").unwrap();
        assert_eq!(result, "2.0.0\n");
    }

    // --- integration ---

    #[test]
    fn integration_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("VERSION");
        std::fs::write(&path, "1.2.3\n").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(PlainVersionFile.detect(&content));
        assert_eq!(
            PlainVersionFile.read_version(&content),
            Some("1.2.3".to_string()),
        );

        let updated = PlainVersionFile.write_version(&content, "3.0.0").unwrap();
        std::fs::write(&path, &updated).unwrap();

        let final_content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            PlainVersionFile.read_version(&final_content),
            Some("3.0.0".to_string()),
        );
    }
}
