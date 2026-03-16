//! Shared helpers for TOML section-based version scanning.
//!
//! Used by [`CargoVersionFile`](crate::cargo::CargoVersionFile) and
//! [`PyprojectVersionFile`](crate::pyproject::PyprojectVersionFile) to
//! avoid duplicating the line-level section detection logic.

use crate::version_file::VersionFileError;

/// Check whether `content` has a `version = "..."` field inside `[section]`.
pub fn detect_version_in_section(content: &str, section: &str) -> bool {
    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == section {
            in_section = true;
        } else if trimmed.starts_with('[') {
            in_section = false;
        }
        if in_section && trimmed.starts_with("version") && trimmed.contains('=') {
            return true;
        }
    }
    false
}

/// Extract the version string from a `version = "..."` field inside
/// `[section]`.
pub fn read_version_in_section(content: &str, section: &str) -> Option<String> {
    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == section {
            in_section = true;
        } else if trimmed.starts_with('[') {
            in_section = false;
        }
        if in_section
            && trimmed.starts_with("version")
            && let Some(eq_pos) = trimmed.find('=')
        {
            let value = trimmed[eq_pos + 1..].trim();
            let version = value.trim_matches('"');
            return Some(version.to_string());
        }
    }
    None
}

/// Replace the version value in a `version = "..."` field inside `[section]`,
/// preserving surrounding formatting.
pub fn write_version_in_section(
    content: &str,
    section: &str,
    new_version: &str,
) -> Result<String, VersionFileError> {
    let mut in_section = false;
    let mut result = String::new();
    let mut replaced = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == section {
            in_section = true;
        } else if trimmed.starts_with('[') {
            in_section = false;
        }

        if in_section
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

#[cfg(test)]
mod tests {
    use super::*;

    const TOML_WITH_PACKAGE: &str = r#"[package]
name = "my-crate"
version = "0.1.0"
edition = "2021"
"#;

    const TOML_WITH_PROJECT: &str = r#"[project]
name = "my-package"
version = "0.1.0"
description = "A test package"
"#;

    #[test]
    fn detect_package_section() {
        assert!(detect_version_in_section(TOML_WITH_PACKAGE, "[package]"));
    }

    #[test]
    fn detect_project_section() {
        assert!(detect_version_in_section(TOML_WITH_PROJECT, "[project]"));
    }

    #[test]
    fn detect_wrong_section() {
        assert!(!detect_version_in_section(TOML_WITH_PACKAGE, "[project]"));
    }

    #[test]
    fn read_package_version() {
        assert_eq!(
            read_version_in_section(TOML_WITH_PACKAGE, "[package]"),
            Some("0.1.0".to_string()),
        );
    }

    #[test]
    fn read_project_version() {
        assert_eq!(
            read_version_in_section(TOML_WITH_PROJECT, "[project]"),
            Some("0.1.0".to_string()),
        );
    }

    #[test]
    fn write_package_version() {
        let result = write_version_in_section(TOML_WITH_PACKAGE, "[package]", "1.0.0").unwrap();
        assert!(result.contains("version = \"1.0.0\""));
        assert!(result.contains("name = \"my-crate\""));
    }

    #[test]
    fn write_no_field_returns_error() {
        let content = "[package]\nname = \"x\"\n";
        let err = write_version_in_section(content, "[package]", "1.0.0");
        assert!(err.is_err());
    }
}
