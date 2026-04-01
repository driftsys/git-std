//! Cargo.toml version file engine.
//!
//! Implements [`VersionFile`] for Rust's `Cargo.toml` manifest, detecting and
//! rewriting the `version` field inside the `[package]` section (regular crate)
//! or the `[workspace.package]` section (workspace manifest) while preserving
//! formatting.

use crate::toml_helpers;
use crate::version_file::{VersionFile, VersionFileError};

/// TOML section header for a regular crate manifest.
const PACKAGE_SECTION: &str = "[package]";

/// TOML section header for a Cargo workspace manifest.
const WORKSPACE_PACKAGE_SECTION: &str = "[workspace.package]";

/// Version file engine for `Cargo.toml`.
///
/// Handles both regular crates (`[package]`) and workspace manifests
/// (`[workspace.package]`), trying `[package]` first.
#[derive(Debug, Clone, Copy)]
pub struct CargoVersionFile;

impl CargoVersionFile {
    /// Return the section that contains the version field, if any.
    fn active_section(content: &str) -> Option<&'static str> {
        if toml_helpers::detect_version_in_section(content, PACKAGE_SECTION) {
            Some(PACKAGE_SECTION)
        } else if toml_helpers::detect_version_in_section(content, WORKSPACE_PACKAGE_SECTION) {
            Some(WORKSPACE_PACKAGE_SECTION)
        } else {
            None
        }
    }
}

impl VersionFile for CargoVersionFile {
    fn name(&self) -> &str {
        "Cargo.toml"
    }

    fn filenames(&self) -> &[&str] {
        &["Cargo.toml"]
    }

    fn detect(&self, content: &str) -> bool {
        Self::active_section(content).is_some()
    }

    fn read_version(&self, content: &str) -> Option<String> {
        let section = Self::active_section(content)?;
        toml_helpers::read_version_in_section(content, section)
    }

    fn write_version(&self, content: &str, new_version: &str) -> Result<String, VersionFileError> {
        let section = Self::active_section(content).ok_or(VersionFileError::NoVersionField)?;
        toml_helpers::write_version_in_section(content, section, new_version)
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

    // --- workspace.package ---

    const WORKSPACE_TOML: &str = r#"[workspace]
members = ["app"]
resolver = "2"

[workspace.package]
version = "0.10.0"
edition = "2021"

[workspace.dependencies]
anyhow = "1"
"#;

    #[test]
    fn detect_workspace_package_version() {
        assert!(CargoVersionFile.detect(WORKSPACE_TOML));
    }

    #[test]
    fn read_version_workspace_package() {
        assert_eq!(
            CargoVersionFile.read_version(WORKSPACE_TOML),
            Some("0.10.0".to_string()),
        );
    }

    #[test]
    fn write_version_workspace_package() {
        let result = CargoVersionFile
            .write_version(WORKSPACE_TOML, "0.11.0")
            .unwrap();
        assert!(result.contains("version = \"0.11.0\""));
        // [workspace.dependencies] versions untouched.
        assert!(result.contains("anyhow = \"1\""));
        // members and resolver untouched.
        assert!(result.contains("members = [\"app\"]"));
    }

    #[test]
    fn package_section_takes_priority_over_workspace_package() {
        // A crate Cargo.toml that happens to also mention [workspace.package]
        // should use [package] version, not [workspace.package].
        let content = "[package]\nname = \"x\"\nversion = \"1.0.0\"\n\n[workspace.package]\nversion = \"2.0.0\"\n";
        assert_eq!(
            CargoVersionFile.read_version(content),
            Some("1.0.0".to_string()),
        );
    }
}
