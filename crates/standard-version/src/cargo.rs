//! Cargo.toml version file engine.
//!
//! Implements [`VersionFile`] for Rust's `Cargo.toml` manifest, detecting and
//! rewriting the `version` field inside the `[package]` section while
//! preserving formatting.

use crate::version_file::{VersionFile, VersionFileError};

/// Version file engine for `Cargo.toml`.
#[derive(Debug, Clone, Copy)]
pub struct CargoVersionFile;

impl VersionFile for CargoVersionFile {
    fn name(&self) -> &str {
        "Cargo.toml"
    }

    fn filenames(&self) -> &[&str] {
        &["Cargo.toml"]
    }

    fn detect(&self, content: &str) -> bool {
        let mut in_package = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed == "[package]" {
                in_package = true;
            } else if trimmed.starts_with('[') {
                in_package = false;
            }
            if in_package && trimmed.starts_with("version") && trimmed.contains('=') {
                return true;
            }
        }
        false
    }

    fn read_version(&self, content: &str) -> Option<String> {
        let mut in_package = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed == "[package]" {
                in_package = true;
            } else if trimmed.starts_with('[') {
                in_package = false;
            }
            if in_package
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
        let mut in_package = false;
        let mut result = String::new();
        let mut replaced = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed == "[package]" {
                in_package = true;
            } else if trimmed.starts_with('[') {
                in_package = false;
            }

            if in_package
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

    const BASIC_TOML: &str = r#"[package]
name = "my-crate"
version = "0.1.0"
edition = "2021"
"#;

    const MULTI_SECTION_TOML: &str = r#"[package]
name = "my-crate"
version = "0.1.0"

[dependencies]
foo = { version = "1.0" }
"#;

    // --- detect ---

    #[test]
    fn detect_with_package_version() {
        assert!(CargoVersionFile.detect(BASIC_TOML));
    }

    #[test]
    fn detect_without_package_section() {
        let content = "[dependencies]\nfoo = \"1\"\n";
        assert!(!CargoVersionFile.detect(content));
    }

    #[test]
    fn detect_version_only_in_deps() {
        let content = "[package]\nname = \"x\"\n\n[dependencies]\nfoo = { version = \"1\" }\n";
        assert!(!CargoVersionFile.detect(content));
    }

    // --- read_version ---

    #[test]
    fn read_version_basic() {
        assert_eq!(
            CargoVersionFile.read_version(BASIC_TOML),
            Some("0.1.0".to_string()),
        );
    }

    #[test]
    fn read_version_no_package() {
        let content = "[dependencies]\nfoo = \"1\"\n";
        assert_eq!(CargoVersionFile.read_version(content), None);
    }

    // --- write_version ---

    #[test]
    fn write_version_basic() {
        let result = CargoVersionFile.write_version(BASIC_TOML, "1.0.0").unwrap();
        assert!(result.contains("version = \"1.0.0\""));
        assert!(result.contains("name = \"my-crate\""));
        assert!(result.contains("edition = \"2021\""));
    }

    #[test]
    fn write_version_only_in_package_section() {
        let result = CargoVersionFile
            .write_version(MULTI_SECTION_TOML, "2.0.0")
            .unwrap();
        assert!(result.contains("version = \"2.0.0\""));
        // Dependency version untouched.
        assert!(result.contains("foo = { version = \"1.0\" }"));
    }

    #[test]
    fn write_version_no_field_returns_error() {
        let content = "[package]\nname = \"x\"\n";
        let err = CargoVersionFile.write_version(content, "1.0.0");
        assert!(err.is_err());
    }

    #[test]
    fn write_version_preserves_no_trailing_newline() {
        let content = "[package]\nname = \"x\"\nversion = \"0.1.0\"";
        let result = CargoVersionFile.write_version(content, "0.2.0").unwrap();
        assert!(!result.ends_with('\n'));
        assert!(result.contains("version = \"0.2.0\""));
    }
}
