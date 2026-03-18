//! Integration tests for JSON-format version file bumping:
//! package.json, deno.json, deno.jsonc.

use std::fs;

use standard_version::update_version_files;

// =========================================================================
// package.json
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
// deno.json
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
// deno.jsonc preserves comments
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
