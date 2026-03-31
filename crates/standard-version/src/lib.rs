//! Semantic version bump calculation from conventional commits.
//!
//! Computes the next version from a list of parsed conventional commits and
//! bump rules. Also provides the [`VersionFile`] trait for ecosystem-specific
//! version file detection and updating, with built-in support for
//! `Cargo.toml` via [`CargoVersionFile`], `pyproject.toml` via
//! [`PyprojectVersionFile`], `package.json` via [`JsonVersionFile`],
//! `deno.json`/`deno.jsonc` via [`DenoVersionFile`], `pubspec.yaml` via
//! [`PubspecVersionFile`], `gradle.properties` via [`GradleVersionFile`],
//! and plain `VERSION` files via [`PlainVersionFile`].
//!
//! # Main entry points
//!
//! - [`determine_bump`] — analyse commits and return the bump level
//! - [`apply_bump`] — apply a bump level to a semver version
//! - [`apply_prerelease`] — bump with a pre-release tag (e.g. `rc.0`)
//! - [`replace_version_in_toml`] — update the version in a `Cargo.toml` string
//! - [`update_version_files`] — discover and update version files at a repo root
//!
//! # Example
//!
//! ```
//! use standard_version::{determine_bump, apply_bump, BumpLevel};
//!
//! let commits = vec![
//!     standard_commit::parse("feat: add login").unwrap(),
//!     standard_commit::parse("fix: handle timeout").unwrap(),
//! ];
//!
//! let level = determine_bump(&commits).unwrap();
//! assert_eq!(level, BumpLevel::Minor);
//!
//! let current = semver::Version::new(1, 2, 3);
//! let next = apply_bump(&current, level);
//! assert_eq!(next, semver::Version::new(1, 3, 0));
//! ```

pub mod bump;
pub mod calver;
pub mod cargo;
pub mod gradle;
pub mod json;
pub mod project;
pub mod pubspec;
pub mod pyproject;
pub mod regex_engine;
pub mod scan;
pub mod toml_helpers;
pub mod version_file;
pub mod version_plain;

pub use bump::{BumpLevel, BumpSummary, apply_bump, apply_prerelease, determine_bump, summarise};
pub use cargo::CargoVersionFile;
pub use gradle::GradleVersionFile;
pub use json::{DenoVersionFile, JsonVersionFile};
pub use project::{ProjectJsonVersionFile, ProjectTomlVersionFile, ProjectYamlVersionFile};
pub use pubspec::PubspecVersionFile;
pub use pyproject::PyprojectVersionFile;
pub use regex_engine::RegexVersionFile;
pub use version_file::{
    CustomVersionFile, DetectedFile, UpdateResult, VersionFile, VersionFileError,
    detect_version_files, update_version_files,
};
pub use version_plain::PlainVersionFile;

/// Replace the `version` value in a TOML string's `[package]` section while
/// preserving formatting.
///
/// Scans for the first `version = "..."` line under `[package]` and rewrites
/// just the value. Lines in other sections (e.g. `[dependencies]`) are left
/// untouched.
///
/// # Errors
///
/// Returns an error if no `version` field is found under `[package]`.
///
/// # Example
///
/// ```
/// let toml = r#"[package]
/// name = "my-crate"
/// version = "0.1.0"
///
/// [dependencies]
/// serde = { version = "1.0" }
/// "#;
///
/// let updated = standard_version::replace_version_in_toml(toml, "2.0.0").unwrap();
/// assert!(updated.contains(r#"version = "2.0.0""#));
/// // dependency version unchanged
/// assert!(updated.contains(r#"serde = { version = "1.0" }"#));
/// ```
pub fn replace_version_in_toml(
    content: &str,
    new_version: &str,
) -> Result<String, VersionFileError> {
    CargoVersionFile.write_version(content, new_version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_version_in_toml_basic() {
        let input = r#"[package]
name = "my-crate"
version = "0.1.0"
edition = "2021"
"#;
        let result = replace_version_in_toml(input, "1.0.0").unwrap();
        assert!(result.contains("version = \"1.0.0\""));
        assert!(result.contains("name = \"my-crate\""));
        assert!(result.contains("edition = \"2021\""));
    }

    #[test]
    fn replace_version_only_in_package_section() {
        let input = r#"[package]
name = "my-crate"
version = "0.1.0"

[dependencies]
foo = { version = "1.0" }
"#;
        let result = replace_version_in_toml(input, "2.0.0").unwrap();
        assert!(result.contains("[package]"));
        assert!(result.contains("version = \"2.0.0\""));
        // Dependency version should be unchanged.
        assert!(result.contains("foo = { version = \"1.0\" }"));
    }
}
