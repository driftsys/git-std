//! Integration tests for Cargo.toml version file bumping.

use std::fs;

use standard_version::update_version_files;

// =========================================================================
// Cargo.toml only
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
// Dry-run scenario (verifies files are updated on disk)
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
