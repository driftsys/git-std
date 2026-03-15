//! `gradle.properties` version file engine.
//!
//! Implements [`VersionFile`] for Android/Gradle projects that store version
//! information as `VERSION_NAME=` in `gradle.properties`. When a
//! `VERSION_CODE=N` line is also present, the integer value is incremented
//! automatically.

use crate::version_file::{VersionFile, VersionFileError};

/// Version file engine for `gradle.properties`.
#[derive(Debug, Clone, Copy)]
pub struct GradleVersionFile;

impl VersionFile for GradleVersionFile {
    fn name(&self) -> &str {
        "gradle.properties"
    }

    fn filenames(&self) -> &[&str] {
        &["gradle.properties"]
    }

    fn detect(&self, content: &str) -> bool {
        content
            .lines()
            .any(|line| line.starts_with("VERSION_NAME="))
    }

    fn read_version(&self, content: &str) -> Option<String> {
        for line in content.lines() {
            if let Some(value) = line.strip_prefix("VERSION_NAME=") {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
        None
    }

    fn write_version(&self, content: &str, new_version: &str) -> Result<String, VersionFileError> {
        let mut result = String::new();
        let mut replaced = false;

        for line in content.lines() {
            if !replaced && line.starts_with("VERSION_NAME=") {
                result.push_str(&format!("VERSION_NAME={new_version}"));
                result.push('\n');
                replaced = true;
                continue;
            }

            if let Some(old_code) = line.strip_prefix("VERSION_CODE=") {
                let old_code = old_code.trim();
                let code_num: u64 = old_code.parse().unwrap_or(0);
                result.push_str(&format!("VERSION_CODE={}", code_num + 1));
                result.push('\n');
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

    fn extra_info(&self, old_content: &str, new_content: &str) -> Option<String> {
        let old_code = extract_version_code(old_content);
        let new_code = extract_version_code(new_content);

        match (old_code, new_code) {
            (Some(old), Some(new)) => Some(format!("VERSION_CODE: {old} \u{2192} {new}")),
            _ => None,
        }
    }
}

/// Extract the `VERSION_CODE` integer value from file content.
fn extract_version_code(content: &str) -> Option<u64> {
    for line in content.lines() {
        if let Some(value) = line.strip_prefix("VERSION_CODE=") {
            return value.trim().parse().ok();
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const BASIC_GRADLE: &str = "\
VERSION_NAME=1.2.3
VERSION_CODE=42
org.gradle.jvmargs=-Xmx2048m
";

    const GRADLE_NO_CODE: &str = "\
VERSION_NAME=1.2.3
org.gradle.jvmargs=-Xmx2048m
";

    // --- detect ---

    #[test]
    fn detect_positive() {
        assert!(GradleVersionFile.detect(BASIC_GRADLE));
    }

    #[test]
    fn detect_negative() {
        let content = "org.gradle.jvmargs=-Xmx2048m\n";
        assert!(!GradleVersionFile.detect(content));
    }

    #[test]
    fn detect_negative_version_name_in_comment() {
        let content = "# VERSION_NAME=1.0.0\norg.gradle.jvmargs=-Xmx2048m\n";
        assert!(!GradleVersionFile.detect(content));
    }

    // --- read_version ---

    #[test]
    fn read_version_basic() {
        assert_eq!(
            GradleVersionFile.read_version(BASIC_GRADLE),
            Some("1.2.3".to_string()),
        );
    }

    #[test]
    fn read_version_missing() {
        let content = "org.gradle.jvmargs=-Xmx2048m\n";
        assert_eq!(GradleVersionFile.read_version(content), None);
    }

    // --- write_version ---

    #[test]
    fn write_version_updates_name_and_code() {
        let result = GradleVersionFile
            .write_version(BASIC_GRADLE, "2.0.0")
            .unwrap();
        assert!(result.contains("VERSION_NAME=2.0.0"));
        assert!(result.contains("VERSION_CODE=43"));
        assert!(result.contains("org.gradle.jvmargs=-Xmx2048m"));
    }

    #[test]
    fn write_version_no_code_stays_without() {
        let result = GradleVersionFile
            .write_version(GRADLE_NO_CODE, "2.0.0")
            .unwrap();
        assert!(result.contains("VERSION_NAME=2.0.0"));
        assert!(!result.contains("VERSION_CODE"));
    }

    #[test]
    fn write_version_no_field_returns_error() {
        let content = "org.gradle.jvmargs=-Xmx2048m\n";
        let err = GradleVersionFile.write_version(content, "1.0.0");
        assert!(err.is_err());
    }

    #[test]
    fn write_version_preserves_no_trailing_newline() {
        let content = "VERSION_NAME=0.1.0";
        let result = GradleVersionFile.write_version(content, "0.2.0").unwrap();
        assert!(!result.ends_with('\n'));
        assert!(result.contains("VERSION_NAME=0.2.0"));
    }

    // --- extra_info ---

    #[test]
    fn extra_info_reports_version_code_change() {
        let old = BASIC_GRADLE;
        let new_content = GradleVersionFile.write_version(old, "2.0.0").unwrap();
        let info = GradleVersionFile.extra_info(old, &new_content);
        assert_eq!(info, Some("VERSION_CODE: 42 \u{2192} 43".to_string()));
    }

    #[test]
    fn extra_info_none_when_no_version_code() {
        let old = GRADLE_NO_CODE;
        let new_content = GradleVersionFile.write_version(old, "2.0.0").unwrap();
        let info = GradleVersionFile.extra_info(old, &new_content);
        assert_eq!(info, None);
    }

    // --- integration ---

    #[test]
    fn integration_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gradle.properties");
        std::fs::write(&path, BASIC_GRADLE).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(GradleVersionFile.detect(&content));
        assert_eq!(
            GradleVersionFile.read_version(&content),
            Some("1.2.3".to_string()),
        );

        let updated = GradleVersionFile.write_version(&content, "3.0.0").unwrap();
        std::fs::write(&path, &updated).unwrap();

        let final_content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            GradleVersionFile.read_version(&final_content),
            Some("3.0.0".to_string()),
        );
        assert!(final_content.contains("VERSION_CODE=43"));
    }

    #[test]
    fn integration_roundtrip_no_code() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gradle.properties");
        std::fs::write(&path, GRADLE_NO_CODE).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let updated = GradleVersionFile.write_version(&content, "3.0.0").unwrap();
        std::fs::write(&path, &updated).unwrap();

        let final_content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            GradleVersionFile.read_version(&final_content),
            Some("3.0.0".to_string()),
        );
        assert!(!final_content.contains("VERSION_CODE"));
    }
}
