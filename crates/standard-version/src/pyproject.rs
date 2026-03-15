//! pyproject.toml version file engine.
//!
//! Implements [`VersionFile`] for Python's `pyproject.toml` manifest, detecting
//! and rewriting the `version` field inside the `[project]` section while
//! preserving formatting.

use crate::version_file::{VersionFile, VersionFileError};

/// Version file engine for `pyproject.toml`.
#[derive(Debug, Clone, Copy)]
pub struct PyprojectVersionFile;

impl VersionFile for PyprojectVersionFile {
    fn name(&self) -> &str {
        "pyproject.toml"
    }

    fn filenames(&self) -> &[&str] {
        &["pyproject.toml"]
    }

    fn detect(&self, content: &str) -> bool {
        let mut in_project = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed == "[project]" {
                in_project = true;
            } else if trimmed.starts_with('[') {
                in_project = false;
            }
            if in_project && trimmed.starts_with("version") && trimmed.contains('=') {
                return true;
            }
        }
        false
    }

    fn read_version(&self, content: &str) -> Option<String> {
        let mut in_project = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed == "[project]" {
                in_project = true;
            } else if trimmed.starts_with('[') {
                in_project = false;
            }
            if in_project
                && trimmed.starts_with("version")
                && let Some(eq_pos) = trimmed.find('=')
            {
                let value = trimmed[eq_pos + 1..].trim();
                // Strip surrounding quotes.
                let version = value.trim_matches('"');
                return Some(version.to_string());
            }
        }
        None
    }

    fn write_version(&self, content: &str, new_version: &str) -> Result<String, VersionFileError> {
        let mut in_project = false;
        let mut result = String::new();
        let mut replaced = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed == "[project]" {
                in_project = true;
            } else if trimmed.starts_with('[') {
                in_project = false;
            }

            if in_project
                && !replaced
                && trimmed.starts_with("version")
                && let Some(eq_pos) = line.find('=')
            {
                let prefix = &line[..=eq_pos];
                result.push_str(prefix);
                result.push_str(&format!(" \"{new_version}\""));
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

    const BASIC_PYPROJECT: &str = r#"[project]
name = "my-package"
version = "0.1.0"
description = "A test package"
"#;

    const MULTI_SECTION_PYPROJECT: &str = r#"[project]
name = "my-package"
version = "0.1.0"
description = "A test package"

[tool.poetry]
version = "0.1.0"
"#;

    // --- detect ---

    #[test]
    fn detect_with_project_version() {
        assert!(PyprojectVersionFile.detect(BASIC_PYPROJECT));
    }

    #[test]
    fn detect_without_project_section() {
        let content = "[tool.poetry]\nversion = \"1.0.0\"\n";
        assert!(!PyprojectVersionFile.detect(content));
    }

    #[test]
    fn detect_project_without_version() {
        let content = "[project]\nname = \"x\"\n\n[tool.poetry]\nversion = \"1.0.0\"\n";
        assert!(!PyprojectVersionFile.detect(content));
    }

    // --- read_version ---

    #[test]
    fn read_version_basic() {
        assert_eq!(
            PyprojectVersionFile.read_version(BASIC_PYPROJECT),
            Some("0.1.0".to_string()),
        );
    }

    #[test]
    fn read_version_no_project() {
        let content = "[tool.poetry]\nversion = \"1.0.0\"\n";
        assert_eq!(PyprojectVersionFile.read_version(content), None);
    }

    // --- write_version ---

    #[test]
    fn write_version_basic() {
        let result = PyprojectVersionFile
            .write_version(BASIC_PYPROJECT, "1.0.0")
            .unwrap();
        assert!(result.contains("version = \"1.0.0\""));
        assert!(result.contains("name = \"my-package\""));
        assert!(result.contains("description = \"A test package\""));
    }

    #[test]
    fn write_version_only_in_project_section() {
        let result = PyprojectVersionFile
            .write_version(MULTI_SECTION_PYPROJECT, "2.0.0")
            .unwrap();
        assert!(result.contains("version = \"2.0.0\""));
        // [tool.poetry] version untouched — count occurrences.
        let count = result.matches("version = \"0.1.0\"").count();
        assert_eq!(count, 1, "tool.poetry version should remain 0.1.0");
    }

    #[test]
    fn write_version_no_field_returns_error() {
        let content = "[project]\nname = \"x\"\n";
        let err = PyprojectVersionFile.write_version(content, "1.0.0");
        assert!(err.is_err());
    }

    #[test]
    fn write_version_preserves_no_trailing_newline() {
        let content = "[project]\nname = \"x\"\nversion = \"0.1.0\"";
        let result = PyprojectVersionFile
            .write_version(content, "0.2.0")
            .unwrap();
        assert!(!result.ends_with('\n'));
        assert!(result.contains("version = \"0.2.0\""));
    }

    #[test]
    fn integration_with_tempdir() {
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        let pyproject = dir.path().join("pyproject.toml");
        fs::write(
            &pyproject,
            r#"[project]
name = "example"
version = "0.1.0"
requires-python = ">=3.8"

[tool.setuptools]
packages = ["example"]
"#,
        )
        .unwrap();

        let content = fs::read_to_string(&pyproject).unwrap();
        assert!(PyprojectVersionFile.detect(&content));
        assert_eq!(
            PyprojectVersionFile.read_version(&content),
            Some("0.1.0".to_string()),
        );

        let updated = PyprojectVersionFile
            .write_version(&content, "2.0.0")
            .unwrap();
        fs::write(&pyproject, &updated).unwrap();

        let on_disk = fs::read_to_string(&pyproject).unwrap();
        assert!(on_disk.contains("version = \"2.0.0\""));
        assert!(on_disk.contains("name = \"example\""));
        assert!(on_disk.contains("requires-python = \">=3.8\""));
    }
}
