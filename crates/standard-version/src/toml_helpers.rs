//! Shared helpers for TOML section-based version scanning.
//!
//! Used by [`CargoVersionFile`](crate::cargo::CargoVersionFile) and
//! [`PyprojectVersionFile`](crate::pyproject::PyprojectVersionFile) to
//! avoid duplicating the line-level section detection logic.

use crate::version_file::VersionFileError;

/// Check whether `content` has a `version = "..."` field inside `[section]`.
///
/// Only matches the exact `version` key — dotted keys such as
/// `version.workspace = true` are intentionally ignored.
pub fn detect_version_in_section(content: &str, section: &str) -> bool {
    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == section {
            in_section = true;
        } else if trimmed.starts_with('[') {
            in_section = false;
        }
        if in_section && is_version_key(trimmed) {
            return true;
        }
    }
    false
}

/// Return `true` if `trimmed` is a `version = ...` key (not a dotted key like
/// `version.workspace = true`).
fn is_version_key(trimmed: &str) -> bool {
    // "version".len() == 7
    trimmed.starts_with("version") && trimmed[7..].trim_start().starts_with('=')
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
            && is_version_key(trimmed)
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
            && is_version_key(trimmed)
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

    // --- workspace inheritance (version.workspace = true) ---

    const MEMBER_INHERITS: &str = r#"[package]
name = "my-lib"
version.workspace = true
edition.workspace = true
"#;

    #[test]
    fn detect_does_not_match_workspace_inherit() {
        // version.workspace = true must NOT be treated as a pinned version field.
        assert!(!detect_version_in_section(MEMBER_INHERITS, "[package]"));
    }

    #[test]
    fn read_does_not_return_workspace_inherit() {
        assert_eq!(read_version_in_section(MEMBER_INHERITS, "[package]"), None);
    }

    #[test]
    fn write_does_not_corrupt_workspace_inherit() {
        // write should fail with NoVersionField, not silently corrupt the line.
        let err = write_version_in_section(MEMBER_INHERITS, "[package]", "1.0.0");
        assert!(err.is_err());
        // Also verify the line would not have been rewritten.
        if let Ok(result) = write_version_in_section(MEMBER_INHERITS, "[package]", "1.0.0") {
            assert!(
                result.contains("version.workspace = true"),
                "workspace inherit line must not be rewritten"
            );
        }
    }
}
