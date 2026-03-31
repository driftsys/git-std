//! Version file discovery and updating.
//!
//! Scans a repository root for known version file formats and applies
//! version updates. See [`update_version_files`] and [`detect_version_files`].

use std::fs;
use std::path::Path;

use crate::cargo::CargoVersionFile;
use crate::gradle::GradleVersionFile;
use crate::json::{DenoVersionFile, JsonVersionFile};
use crate::project::{ProjectJsonVersionFile, ProjectTomlVersionFile, ProjectYamlVersionFile};
use crate::pubspec::PubspecVersionFile;
use crate::pyproject::PyprojectVersionFile;
use crate::regex_engine::RegexVersionFile;
use crate::version_file::{
    CustomVersionFile, DetectedFile, UpdateResult, VersionFile, VersionFileError,
};
use crate::version_plain::PlainVersionFile;

/// Build the list of built-in version file engines.
fn builtin_engines() -> Vec<Box<dyn VersionFile>> {
    vec![
        Box::new(CargoVersionFile),
        Box::new(PyprojectVersionFile),
        Box::new(JsonVersionFile),
        Box::new(DenoVersionFile),
        Box::new(PubspecVersionFile),
        Box::new(GradleVersionFile),
        Box::new(ProjectTomlVersionFile),
        Box::new(ProjectJsonVersionFile),
        Box::new(ProjectYamlVersionFile),
        Box::new(PlainVersionFile),
    ]
}

/// Discover and update version files at `root`.
///
/// Iterates all built-in version file engines ([`CargoVersionFile`],
/// [`PyprojectVersionFile`], [`JsonVersionFile`], [`DenoVersionFile`],
/// [`PubspecVersionFile`], [`GradleVersionFile`], [`PlainVersionFile`])
/// and, for each file that is detected, replaces the version string with
/// `new_version`. Then processes any user-defined `custom_files` using the
/// [`RegexVersionFile`] engine.
///
/// Updated content is written back to disk.
///
/// # Errors
///
/// Returns a [`VersionFileError`] if a detected file cannot be read or
/// written, or if a custom file has an invalid regex pattern.
pub fn update_version_files(
    root: &Path,
    new_version: &str,
    custom_files: &[CustomVersionFile],
) -> Result<Vec<UpdateResult>, VersionFileError> {
    // Validate all custom regexes upfront before any file writes.
    let custom_engines: Vec<RegexVersionFile> = custom_files
        .iter()
        .map(RegexVersionFile::new)
        .collect::<Result<Vec<_>, _>>()?;

    let engines = builtin_engines();
    let mut results = Vec::new();

    for engine in &engines {
        for filename in engine.filenames() {
            let path = root.join(filename);
            if !path.exists() {
                continue;
            }

            let content = fs::read_to_string(&path).map_err(VersionFileError::ReadFailed)?;

            if !engine.detect(&content) {
                continue;
            }

            let old_version = match engine.read_version(&content) {
                Some(v) => v,
                None => continue,
            };

            let updated = engine.write_version(&content, new_version)?;
            let extra = engine.extra_info(&content, &updated);
            // Read the actual version from updated content (may differ for
            // pubspec build numbers or other engines with side-effects).
            let actual_new_version = engine
                .read_version(&updated)
                .unwrap_or_else(|| new_version.to_string());
            fs::write(&path, &updated).map_err(VersionFileError::WriteFailed)?;

            results.push(UpdateResult {
                path,
                name: engine.name().to_string(),
                old_version,
                new_version: actual_new_version,
                extra,
            });
        }
    }

    // Process custom version files (already validated).
    for engine in &custom_engines {
        let path = root.join(engine.path());
        if !path.exists() {
            continue;
        }
        let content = fs::read_to_string(&path).map_err(VersionFileError::ReadFailed)?;
        if !engine.detect(&content) {
            continue;
        }
        let old_version = match engine.read_version(&content) {
            Some(v) => v,
            None => continue,
        };
        let updated = engine.write_version(&content, new_version)?;
        let actual_new_version = engine
            .read_version(&updated)
            .unwrap_or_else(|| new_version.to_string());
        fs::write(&path, &updated).map_err(VersionFileError::WriteFailed)?;
        results.push(UpdateResult {
            path,
            name: engine.name(),
            old_version,
            new_version: actual_new_version,
            extra: None,
        });
    }

    Ok(results)
}

/// Detect version files at `root` without modifying them.
///
/// Returns a list of [`DetectedFile`] entries for each file that is found,
/// detected, and has a readable version string. No files are written.
///
/// # Errors
///
/// Returns a [`VersionFileError`] if a file cannot be read or if a custom
/// regex pattern is invalid.
pub fn detect_version_files(
    root: &Path,
    custom_files: &[CustomVersionFile],
) -> Result<Vec<DetectedFile>, VersionFileError> {
    // Validate all custom regexes upfront.
    let custom_engines: Vec<RegexVersionFile> = custom_files
        .iter()
        .map(RegexVersionFile::new)
        .collect::<Result<Vec<_>, _>>()?;

    let engines = builtin_engines();
    let mut results = Vec::new();

    for engine in &engines {
        for filename in engine.filenames() {
            let path = root.join(filename);
            if !path.exists() {
                continue;
            }
            let content = fs::read_to_string(&path).map_err(VersionFileError::ReadFailed)?;
            if !engine.detect(&content) {
                continue;
            }
            let old_version = match engine.read_version(&content) {
                Some(v) => v,
                None => continue,
            };
            results.push(DetectedFile {
                path,
                name: engine.name().to_string(),
                old_version,
            });
        }
    }

    for engine in &custom_engines {
        let path = root.join(engine.path());
        if !path.exists() {
            continue;
        }
        let content = fs::read_to_string(&path).map_err(VersionFileError::ReadFailed)?;
        if !engine.detect(&content) {
            continue;
        }
        let old_version = match engine.read_version(&content) {
            Some(v) => v,
            None => continue,
        };
        results.push(DetectedFile {
            path,
            name: engine.name(),
            old_version,
        });
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn update_version_files_updates_cargo_toml() {
        let dir = tempfile::tempdir().unwrap();
        let cargo_toml = dir.path().join("Cargo.toml");
        fs::write(
            &cargo_toml,
            r#"[package]
name = "example"
version = "0.1.0"
edition = "2024"
"#,
        )
        .unwrap();

        let results = update_version_files(dir.path(), "2.0.0", &[]).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].old_version, "0.1.0");
        assert_eq!(results[0].new_version, "2.0.0");
        assert_eq!(results[0].name, "Cargo.toml");
        assert_eq!(results[0].path, cargo_toml);

        let on_disk = fs::read_to_string(&cargo_toml).unwrap();
        assert!(on_disk.contains("version = \"2.0.0\""));
    }

    #[test]
    fn update_version_files_skips_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        // No Cargo.toml present.
        let results = update_version_files(dir.path(), "1.0.0", &[]).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn update_version_files_skips_undetected() {
        let dir = tempfile::tempdir().unwrap();
        let cargo_toml = dir.path().join("Cargo.toml");
        // File exists but has no [package] section.
        fs::write(&cargo_toml, "[dependencies]\nfoo = \"1\"\n").unwrap();

        let results = update_version_files(dir.path(), "1.0.0", &[]).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn update_version_files_updates_pyproject_toml() {
        let dir = tempfile::tempdir().unwrap();
        let pyproject = dir.path().join("pyproject.toml");
        fs::write(
            &pyproject,
            r#"[project]
name = "example"
version = "0.1.0"
requires-python = ">=3.8"
"#,
        )
        .unwrap();

        let results = update_version_files(dir.path(), "2.0.0", &[]).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].old_version, "0.1.0");
        assert_eq!(results[0].new_version, "2.0.0");
        assert_eq!(results[0].name, "pyproject.toml");
        assert_eq!(results[0].path, pyproject);

        let on_disk = fs::read_to_string(&pyproject).unwrap();
        assert!(on_disk.contains("version = \"2.0.0\""));
    }

    #[test]
    fn update_version_files_updates_pubspec_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let pubspec = dir.path().join("pubspec.yaml");
        fs::write(
            &pubspec,
            "name: my_app\nversion: 1.0.0\ndescription: test\n",
        )
        .unwrap();

        let results = update_version_files(dir.path(), "2.0.0", &[]).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].old_version, "1.0.0");
        assert_eq!(results[0].new_version, "2.0.0");
        assert_eq!(results[0].name, "pubspec.yaml");

        let on_disk = fs::read_to_string(&pubspec).unwrap();
        assert!(on_disk.contains("version: 2.0.0"));
    }

    #[test]
    fn update_version_files_updates_gradle_properties() {
        let dir = tempfile::tempdir().unwrap();
        let gradle = dir.path().join("gradle.properties");
        fs::write(&gradle, "VERSION_NAME=1.0.0\nVERSION_CODE=10\n").unwrap();

        let results = update_version_files(dir.path(), "2.0.0", &[]).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].old_version, "1.0.0");
        assert_eq!(results[0].name, "gradle.properties");
        assert_eq!(
            results[0].extra,
            Some("VERSION_CODE: 10 \u{2192} 11".to_string()),
        );

        let on_disk = fs::read_to_string(&gradle).unwrap();
        assert!(on_disk.contains("VERSION_NAME=2.0.0"));
        assert!(on_disk.contains("VERSION_CODE=11"));
    }

    #[test]
    fn update_version_files_updates_version_file() {
        let dir = tempfile::tempdir().unwrap();
        let version = dir.path().join("VERSION");
        fs::write(&version, "1.0.0\n").unwrap();

        let results = update_version_files(dir.path(), "2.0.0", &[]).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].old_version, "1.0.0");
        assert_eq!(results[0].name, "VERSION");

        let on_disk = fs::read_to_string(&version).unwrap();
        assert_eq!(on_disk, "2.0.0\n");
    }

    #[test]
    fn update_version_files_updates_multiple_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"x\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        fs::write(dir.path().join("pubspec.yaml"), "name: x\nversion: 1.0.0\n").unwrap();
        fs::write(dir.path().join("VERSION"), "1.0.0\n").unwrap();

        let results = update_version_files(dir.path(), "2.0.0", &[]).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn error_display() {
        let err = VersionFileError::NoVersionField;
        assert_eq!(err.to_string(), "no version field found");

        let err = VersionFileError::FileNotFound(std::path::PathBuf::from("/tmp/gone"));
        assert!(err.to_string().contains("/tmp/gone"));
    }
}
