//! Integration tests for YAML-format version file bumping:
//! pubspec.yaml and gradle.properties.

use std::fs;

use standard_version::update_version_files;

// =========================================================================
// pubspec.yaml with build number
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
// pubspec.yaml without build number
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
// gradle.properties with VERSION_CODE
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
