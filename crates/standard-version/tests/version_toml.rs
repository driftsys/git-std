//! Integration tests for TOML-format version file bumping:
//! pyproject.toml.

use std::fs;

use standard_version::update_version_files;

// =========================================================================
// pyproject.toml
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
