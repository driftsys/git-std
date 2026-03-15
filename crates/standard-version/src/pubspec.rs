//! `pubspec.yaml` version file engine.
//!
//! Implements [`VersionFile`] for Flutter/Dart's `pubspec.yaml` manifest,
//! detecting and rewriting the top-level `version:` field. When the existing
//! version carries a `+N` build number suffix, the suffix is incremented
//! automatically.

use crate::version_file::{VersionFile, VersionFileError};

/// Version file engine for `pubspec.yaml`.
#[derive(Debug, Clone, Copy)]
pub struct PubspecVersionFile;

impl VersionFile for PubspecVersionFile {
    fn name(&self) -> &str {
        "pubspec.yaml"
    }

    fn filenames(&self) -> &[&str] {
        &["pubspec.yaml"]
    }

    fn detect(&self, content: &str) -> bool {
        content
            .lines()
            .any(|line| line.starts_with("version:") && line.len() > "version:".len())
    }

    fn read_version(&self, content: &str) -> Option<String> {
        for line in content.lines() {
            if let Some(value) = line.strip_prefix("version:") {
                let value = value.trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        None
    }

    fn write_version(&self, content: &str, new_version: &str) -> Result<String, VersionFileError> {
        let mut result = String::new();
        let mut replaced = false;

        for line in content.lines() {
            if !replaced && let Some(old_value) = line.strip_prefix("version:") {
                let old_value = old_value.trim();
                let new_value = if let Some(pos) = old_value.find('+') {
                    // Existing build number — increment it.
                    let build_str = &old_value[pos + 1..];
                    let build_num: u64 = build_str.parse().unwrap_or(0);
                    format!("{new_version}+{}", build_num + 1)
                } else {
                    new_version.to_string()
                };
                result.push_str(&format!("version: {new_value}"));
                result.push('\n');
                replaced = true;
                continue;
            }
            result.push_str(line);
            result.push('\n');
        }

        if !replaced {
            return Err(VersionFileError::NoVersionField);
        }

        // Preserve original trailing-newline behaviour.
        if !content.ends_with('\n') && result.ends_with('\n') {
            result.pop();
        }

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const BASIC_PUBSPEC: &str = "\
name: my_app
version: 1.2.3
description: A sample app
";

    const PUBSPEC_WITH_BUILD: &str = "\
name: my_app
version: 1.2.3+42
description: A sample app
";

    // --- detect ---

    #[test]
    fn detect_positive() {
        assert!(PubspecVersionFile.detect(BASIC_PUBSPEC));
    }

    #[test]
    fn detect_positive_with_build_number() {
        assert!(PubspecVersionFile.detect(PUBSPEC_WITH_BUILD));
    }

    #[test]
    fn detect_negative_no_version() {
        let content = "name: my_app\ndescription: A sample app\n";
        assert!(!PubspecVersionFile.detect(content));
    }

    #[test]
    fn detect_negative_version_in_middle_of_line() {
        let content = "name: my_app\n# version: 1.0.0\ndescription: foo\n";
        assert!(!PubspecVersionFile.detect(content));
    }

    // --- read_version ---

    #[test]
    fn read_version_basic() {
        assert_eq!(
            PubspecVersionFile.read_version(BASIC_PUBSPEC),
            Some("1.2.3".to_string()),
        );
    }

    #[test]
    fn read_version_with_build_number() {
        assert_eq!(
            PubspecVersionFile.read_version(PUBSPEC_WITH_BUILD),
            Some("1.2.3+42".to_string()),
        );
    }

    #[test]
    fn read_version_missing() {
        let content = "name: my_app\n";
        assert_eq!(PubspecVersionFile.read_version(content), None);
    }

    // --- write_version ---

    #[test]
    fn write_version_basic() {
        let result = PubspecVersionFile
            .write_version(BASIC_PUBSPEC, "2.0.0")
            .unwrap();
        assert!(result.contains("version: 2.0.0"));
        assert!(!result.contains('+'));
        assert!(result.contains("name: my_app"));
    }

    #[test]
    fn write_version_increments_build_number() {
        let result = PubspecVersionFile
            .write_version(PUBSPEC_WITH_BUILD, "2.0.0")
            .unwrap();
        assert!(result.contains("version: 2.0.0+43"));
        assert!(result.contains("name: my_app"));
    }

    #[test]
    fn write_version_no_field_returns_error() {
        let content = "name: my_app\n";
        let err = PubspecVersionFile.write_version(content, "1.0.0");
        assert!(err.is_err());
    }

    #[test]
    fn write_version_preserves_no_trailing_newline() {
        let content = "name: my_app\nversion: 0.1.0";
        let result = PubspecVersionFile.write_version(content, "0.2.0").unwrap();
        assert!(!result.ends_with('\n'));
        assert!(result.contains("version: 0.2.0"));
    }

    // --- integration ---

    #[test]
    fn integration_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pubspec.yaml");
        std::fs::write(&path, BASIC_PUBSPEC).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(PubspecVersionFile.detect(&content));
        assert_eq!(
            PubspecVersionFile.read_version(&content),
            Some("1.2.3".to_string()),
        );

        let updated = PubspecVersionFile.write_version(&content, "3.0.0").unwrap();
        std::fs::write(&path, &updated).unwrap();

        let final_content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            PubspecVersionFile.read_version(&final_content),
            Some("3.0.0".to_string()),
        );
    }

    #[test]
    fn integration_roundtrip_with_build_number() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pubspec.yaml");
        std::fs::write(&path, PUBSPEC_WITH_BUILD).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let updated = PubspecVersionFile.write_version(&content, "3.0.0").unwrap();
        std::fs::write(&path, &updated).unwrap();

        let final_content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            PubspecVersionFile.read_version(&final_content),
            Some("3.0.0+43".to_string()),
        );
    }
}
