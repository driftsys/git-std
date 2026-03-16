//! Integration tests for multi-ecosystem version file bumping.
//!
//! Each test creates a temporary directory with realistic file contents,
//! calls [`update_version_files`], and verifies both the return value and
//! the file on disk.

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
// 1. Cargo.toml only
// =========================================================================

#[test]
fn bump_cargo_toml_only() {
    let dir = tempfile::tempdir().unwrap();
    let cargo_toml = dir.path().join("Cargo.toml");
    fs::write(
        &cargo_toml,
        r#"[package]
name = "my-app"
version = "1.0.0"
edition = "2021"
"#,
    )
    .unwrap();

    let results = update_version_files(dir.path(), "1.1.0", &[]).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Cargo.toml");
    assert_eq!(results[0].old_version, "1.0.0");
    assert_eq!(results[0].new_version, "1.1.0");

    let on_disk = fs::read_to_string(&cargo_toml).unwrap();
    assert!(on_disk.contains(r#"version = "1.1.0""#));
    assert!(on_disk.contains(r#"name = "my-app""#));
    assert!(on_disk.contains(r#"edition = "2021""#));
}

// =========================================================================
// 2. Cargo.toml + package.json
// =========================================================================

#[test]
fn bump_cargo_and_package_json() {
    let dir = tempfile::tempdir().unwrap();

    fs::write(
        dir.path().join("Cargo.toml"),
        r#"[package]
name = "my-app"
version = "1.0.0"
edition = "2021"
"#,
    )
    .unwrap();

    fs::write(
        dir.path().join("package.json"),
        r#"{
  "name": "my-app",
  "version": "1.0.0",
  "main": "index.js"
}"#,
    )
    .unwrap();

    let results = update_version_files(dir.path(), "2.0.0", &[]).unwrap();

    assert_eq!(results.len(), 2);

    let cargo = results.iter().find(|r| r.name == "Cargo.toml").unwrap();
    assert_eq!(cargo.old_version, "1.0.0");
    assert_eq!(cargo.new_version, "2.0.0");

    let pkg = results.iter().find(|r| r.name == "package.json").unwrap();
    assert_eq!(pkg.old_version, "1.0.0");
    assert_eq!(pkg.new_version, "2.0.0");

    let cargo_disk = fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo_disk.contains(r#"version = "2.0.0""#));

    let pkg_disk = fs::read_to_string(dir.path().join("package.json")).unwrap();
    assert!(pkg_disk.contains(r#""version": "2.0.0""#));
}

// =========================================================================
// 3. pyproject.toml
// =========================================================================

#[test]
fn bump_pyproject_toml() {
    let dir = tempfile::tempdir().unwrap();
    let pyproject = dir.path().join("pyproject.toml");
    fs::write(
        &pyproject,
        r#"[project]
name = "my-app"
version = "1.0.0"
requires-python = ">=3.8"
"#,
    )
    .unwrap();

    let results = update_version_files(dir.path(), "1.2.0", &[]).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "pyproject.toml");
    assert_eq!(results[0].old_version, "1.0.0");
    assert_eq!(results[0].new_version, "1.2.0");

    let on_disk = fs::read_to_string(&pyproject).unwrap();
    assert!(on_disk.contains(r#"version = "1.2.0""#));
    assert!(on_disk.contains(r#"requires-python = ">=3.8""#));
}

// =========================================================================
// 4. deno.json
// =========================================================================

#[test]
fn bump_deno_json() {
    let dir = tempfile::tempdir().unwrap();
    let deno = dir.path().join("deno.json");
    fs::write(
        &deno,
        r#"{
  "version": "1.0.0",
  "tasks": {
    "dev": "deno run --watch main.ts"
  }
}"#,
    )
    .unwrap();

    let results = update_version_files(dir.path(), "1.1.0", &[]).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "deno.json");
    assert_eq!(results[0].old_version, "1.0.0");
    assert_eq!(results[0].new_version, "1.1.0");

    let on_disk = fs::read_to_string(&deno).unwrap();
    assert!(on_disk.contains(r#""version": "1.1.0""#));
}

// =========================================================================
// 5. deno.jsonc preserves comments
// =========================================================================

#[test]
fn bump_deno_jsonc_preserves_comments() {
    let dir = tempfile::tempdir().unwrap();
    let deno = dir.path().join("deno.jsonc");
    fs::write(
        &deno,
        r#"{
  // The current release version.
  "version": "1.0.0",
  "tasks": {
    "dev": "deno run --watch main.ts"
  }
}"#,
    )
    .unwrap();

    let results = update_version_files(dir.path(), "2.0.0", &[]).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "deno.json");
    assert_eq!(results[0].old_version, "1.0.0");
    assert_eq!(results[0].new_version, "2.0.0");

    let on_disk = fs::read_to_string(&deno).unwrap();
    assert!(on_disk.contains(r#""version": "2.0.0""#));
    assert!(
        on_disk.contains("// The current release version."),
        "JSONC comments should be preserved"
    );
}

// =========================================================================
// 6. pubspec.yaml with build number
// =========================================================================

#[test]
fn bump_pubspec_yaml_with_build_number() {
    let dir = tempfile::tempdir().unwrap();
    let pubspec = dir.path().join("pubspec.yaml");
    fs::write(
        &pubspec,
        "name: my_app\nversion: 1.2.3+42\ndescription: A Flutter app\n",
    )
    .unwrap();

    let results = update_version_files(dir.path(), "2.0.0", &[]).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "pubspec.yaml");
    assert_eq!(results[0].old_version, "1.2.3+42");
    assert_eq!(
        results[0].new_version, "2.0.0+43",
        "new_version should reflect the actual written value including build number"
    );

    let on_disk = fs::read_to_string(&pubspec).unwrap();
    assert!(
        on_disk.contains("version: 2.0.0+43"),
        "build number should be incremented from 42 to 43"
    );
}

// =========================================================================
// 7. pubspec.yaml without build number
// =========================================================================

#[test]
fn bump_pubspec_yaml_without_build_number() {
    let dir = tempfile::tempdir().unwrap();
    let pubspec = dir.path().join("pubspec.yaml");
    fs::write(
        &pubspec,
        "name: my_app\nversion: 1.2.3\ndescription: A Flutter app\n",
    )
    .unwrap();

    let results = update_version_files(dir.path(), "2.0.0", &[]).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "pubspec.yaml");

    let on_disk = fs::read_to_string(&pubspec).unwrap();
    assert!(on_disk.contains("version: 2.0.0"));
    assert!(
        !on_disk.contains('+'),
        "should not add a build number where there was none"
    );
}

// =========================================================================
// 8. gradle.properties with VERSION_CODE
// =========================================================================

#[test]
fn bump_gradle_properties_with_version_code() {
    let dir = tempfile::tempdir().unwrap();
    let gradle = dir.path().join("gradle.properties");
    fs::write(
        &gradle,
        "VERSION_NAME=1.0.0\nVERSION_CODE=42\norg.gradle.jvmargs=-Xmx2048m\n",
    )
    .unwrap();

    let results = update_version_files(dir.path(), "2.0.0", &[]).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "gradle.properties");
    assert_eq!(results[0].old_version, "1.0.0");
    assert_eq!(results[0].new_version, "2.0.0");
    assert_eq!(
        results[0].extra,
        Some("VERSION_CODE: 42 \u{2192} 43".to_string()),
    );

    let on_disk = fs::read_to_string(&gradle).unwrap();
    assert!(on_disk.contains("VERSION_NAME=2.0.0"));
    assert!(on_disk.contains("VERSION_CODE=43"));
}

// =========================================================================
// 9. VERSION file
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
// 10. Custom regex file (pom.xml)
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
// 11. Invalid regex (no capture group)
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
// 12. Missing files are skipped
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
// 13. Files without a version field are skipped
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
// 14. Dry-run scenario (verifies files are updated on disk)
// =========================================================================

#[test]
fn bump_dry_run_scenario() {
    let dir = tempfile::tempdir().unwrap();
    let cargo_toml = dir.path().join("Cargo.toml");
    fs::write(
        &cargo_toml,
        r#"[package]
name = "my-app"
version = "1.0.0"
edition = "2021"
"#,
    )
    .unwrap();

    let results = update_version_files(dir.path(), "3.0.0", &[]).unwrap();
    assert_eq!(results.len(), 1);

    // Verify the file was actually written to disk (not just returned).
    let on_disk = fs::read_to_string(&cargo_toml).unwrap();
    assert!(
        on_disk.contains(r#"version = "3.0.0""#),
        "update_version_files must write changes to disk"
    );
    assert!(
        !on_disk.contains(r#"version = "1.0.0""#),
        "old version should no longer be present"
    );
}

// =========================================================================
// 15. All ecosystems at once
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
