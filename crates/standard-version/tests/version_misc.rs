//! Integration tests for miscellaneous version file bumping:
//! VERSION file, custom regex files, error cases, and all-ecosystems test.

use std::fs;
use std::path::PathBuf;

use standard_version::{CustomVersionFile, update_version_files};

// =========================================================================
// Helpers
// =========================================================================

/// Shorthand for creating a [`CustomVersionFile`].
fn custom(path: &str, pattern: &str) -> CustomVersionFile {
    CustomVersionFile {
        path: PathBuf::from(path),
        pattern: pattern.to_string(),
    }
}

// =========================================================================
// VERSION file
// =========================================================================

#[test]
fn bump_version_file() {
    let dir = tempfile::tempdir().unwrap();
    let version = dir.path().join("VERSION");
    fs::write(&version, "1.0.0\n").unwrap();

    let results = update_version_files(dir.path(), "2.0.0", &[]).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "VERSION");
    assert_eq!(results[0].old_version, "1.0.0");
    assert_eq!(results[0].new_version, "2.0.0");

    let on_disk = fs::read_to_string(&version).unwrap();
    assert_eq!(on_disk, "2.0.0\n");
}

// =========================================================================
// Custom regex file (pom.xml)
// =========================================================================

#[test]
fn bump_custom_regex_file() {
    let dir = tempfile::tempdir().unwrap();
    let pom = dir.path().join("pom.xml");
    fs::write(
        &pom,
        r#"<?xml version="1.0"?>
<project>
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.example</groupId>
  <artifactId>my-app</artifactId>
  <version>1.0.0</version>
</project>
"#,
    )
    .unwrap();

    let custom_files = vec![custom(
        "pom.xml",
        r"<artifactId>my-app</artifactId>\s*<version>([^<]+)</version>",
    )];
    let results = update_version_files(dir.path(), "2.0.0", &custom_files).unwrap();

    let pom_result = results.iter().find(|r| r.name == "pom.xml").unwrap();
    assert_eq!(pom_result.old_version, "1.0.0");
    assert_eq!(pom_result.new_version, "2.0.0");

    let on_disk = fs::read_to_string(&pom).unwrap();
    assert!(on_disk.contains("<version>2.0.0</version>"));
    // modelVersion should be untouched.
    assert!(on_disk.contains("<modelVersion>4.0.0</modelVersion>"));
}

// =========================================================================
// Invalid regex (no capture group)
// =========================================================================

#[test]
fn bump_invalid_regex_no_capture_group() {
    let dir = tempfile::tempdir().unwrap();
    let txt = dir.path().join("version.txt");
    fs::write(&txt, "version = 1.0.0\n").unwrap();

    let custom_files = vec![custom("version.txt", r"version = \d+\.\d+\.\d+")];
    let result = update_version_files(dir.path(), "2.0.0", &custom_files);

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("capture group"),
        "expected capture group error, got: {err}"
    );
}

// =========================================================================
// Missing files are skipped
// =========================================================================

#[test]
fn bump_missing_files_skipped() {
    let dir = tempfile::tempdir().unwrap();
    // No built-in files exist, and the custom file points to a nonexistent path.
    let custom_files = vec![custom("nonexistent.xml", r"<version>([^<]+)</version>")];
    let results = update_version_files(dir.path(), "2.0.0", &custom_files).unwrap();
    assert!(
        results.is_empty(),
        "missing files should be skipped, got {} results",
        results.len()
    );
}

// =========================================================================
// Files without a version field are skipped
// =========================================================================

#[test]
fn bump_files_without_version_skipped() {
    let dir = tempfile::tempdir().unwrap();

    // package.json without "version" field.
    fs::write(
        dir.path().join("package.json"),
        r#"{
  "name": "my-app",
  "main": "index.js"
}"#,
    )
    .unwrap();

    // Cargo.toml without [package] section.
    fs::write(
        dir.path().join("Cargo.toml"),
        "[dependencies]\nfoo = \"1\"\n",
    )
    .unwrap();

    let results = update_version_files(dir.path(), "2.0.0", &[]).unwrap();
    assert!(
        results.is_empty(),
        "files without version fields should be skipped, got {} results",
        results.len()
    );
}

// =========================================================================
// All ecosystems at once
// =========================================================================

#[test]
fn bump_all_ecosystems() {
    let dir = tempfile::tempdir().unwrap();

    // Cargo.toml
    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[package]
name = "my-app"
version = "1.0.0"
edition = "2021"
"#,
    )
    .unwrap();

    // package.json
    fs::write(
        dir.path().join("package.json"),
        r#"{
  "name": "my-app",
  "version": "1.0.0",
  "main": "index.js"
}"#,
    )
    .unwrap();

    // deno.json
    fs::write(
        dir.path().join("deno.json"),
        r#"{
  "version": "1.0.0",
  "tasks": { "dev": "deno run main.ts" }
}"#,
    )
    .unwrap();

    // pyproject.toml
    fs::write(
        dir.path().join("pyproject.toml"),
        r#"[project]
name = "my-app"
version = "1.0.0"
requires-python = ">=3.8"
"#,
    )
    .unwrap();

    // pubspec.yaml
    fs::write(
        dir.path().join("pubspec.yaml"),
        "name: my_app\nversion: 1.0.0\ndescription: A Flutter app\n",
    )
    .unwrap();

    // gradle.properties
    fs::write(
        dir.path().join("gradle.properties"),
        "VERSION_NAME=1.0.0\nVERSION_CODE=10\n",
    )
    .unwrap();

    // VERSION
    fs::write(dir.path().join("VERSION"), "1.0.0\n").unwrap();

    // Custom regex file: pom.xml
    fs::write(
        dir.path().join("pom.xml"),
        "<project>\n  <version>1.0.0</version>\n</project>\n",
    )
    .unwrap();

    let custom_files = vec![custom("pom.xml", r"<version>([^<]+)</version>")];
    let results = update_version_files(dir.path(), "2.0.0", &custom_files).unwrap();

    assert_eq!(
        results.len(),
        8,
        "expected 8 updated files, got {}: {:?}",
        results.len(),
        results.iter().map(|r| &r.name).collect::<Vec<_>>()
    );

    // Verify each engine is represented.
    let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"Cargo.toml"));
    assert!(names.contains(&"pyproject.toml"));
    assert!(names.contains(&"package.json"));
    assert!(names.contains(&"deno.json"));
    assert!(names.contains(&"pubspec.yaml"));
    assert!(names.contains(&"gradle.properties"));
    assert!(names.contains(&"VERSION"));
    assert!(names.contains(&"pom.xml"));

    // Spot-check files on disk.
    let cargo_disk = fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo_disk.contains(r#"version = "2.0.0""#));

    let pkg_disk = fs::read_to_string(dir.path().join("package.json")).unwrap();
    assert!(pkg_disk.contains(r#""version": "2.0.0""#));

    let pyproject_disk = fs::read_to_string(dir.path().join("pyproject.toml")).unwrap();
    assert!(pyproject_disk.contains(r#"version = "2.0.0""#));

    let deno_disk = fs::read_to_string(dir.path().join("deno.json")).unwrap();
    assert!(deno_disk.contains(r#""version": "2.0.0""#));

    let pubspec_disk = fs::read_to_string(dir.path().join("pubspec.yaml")).unwrap();
    assert!(pubspec_disk.contains("version: 2.0.0"));

    let gradle_disk = fs::read_to_string(dir.path().join("gradle.properties")).unwrap();
    assert!(gradle_disk.contains("VERSION_NAME=2.0.0"));
    assert!(gradle_disk.contains("VERSION_CODE=11"));

    let version_disk = fs::read_to_string(dir.path().join("VERSION")).unwrap();
    assert_eq!(version_disk, "2.0.0\n");

    let pom_disk = fs::read_to_string(dir.path().join("pom.xml")).unwrap();
    assert!(pom_disk.contains("<version>2.0.0</version>"));
}
