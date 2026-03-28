use assert_cmd::Command;
use predicates::prelude::*;

fn git_std() -> Command {
    Command::cargo_bin("git-std").unwrap()
}

// ── Malformed TOML ───────────────────────────────────────────────

#[test]
fn config_list_malformed_toml_falls_back_to_defaults() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".git-std.toml"), "[[broken toml = {\n").unwrap();

    let output = git_std()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .get_output()
        .clone();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("warning"),
        "should warn about invalid TOML, got: {stderr}"
    );
    assert!(
        stderr.contains("scheme = semver"),
        "should fall back to defaults, got: {stderr}"
    );
}

#[test]
fn config_list_json_malformed_toml_still_outputs_valid_json() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".git-std.toml"), "broken [\n").unwrap();

    let output = git_std()
        .args(["config", "list", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .get_output()
        .clone();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("should still output valid JSON with defaults");

    // Should contain default values.
    assert_eq!(parsed["scheme"], "semver");
    assert_eq!(parsed["strict"], false);
}

// ── Invalid value types (silently fall back to defaults) ─────────
//
// `build_config()` uses `.and_then(|v| v.as_bool())`, `.as_str()`,
// `.as_array()` — wrong types silently return `None`, falling back
// to defaults with NO warning. These tests assert that behavior.

#[test]
fn config_list_scheme_wrong_type_falls_back_to_default() {
    let dir = tempfile::tempdir().unwrap();
    // scheme expects a string; provide an integer.
    std::fs::write(dir.path().join(".git-std.toml"), "scheme = 123\n").unwrap();

    git_std()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("scheme = semver"));
}

#[test]
fn config_list_strict_wrong_type_falls_back_to_default() {
    let dir = tempfile::tempdir().unwrap();
    // strict expects a boolean; provide a string.
    std::fs::write(dir.path().join(".git-std.toml"), "strict = \"yes\"\n").unwrap();

    git_std()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("strict = false"));
}

#[test]
fn config_list_types_wrong_type_falls_back_to_default() {
    let dir = tempfile::tempdir().unwrap();
    // types expects an array; provide a string.
    std::fs::write(dir.path().join(".git-std.toml"), "types = \"feat\"\n").unwrap();

    git_std()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("types = [\"feat\""));
}

// ── Invalid calver_format ────────────────────────────────────────

#[test]
fn config_list_invalid_calver_format_warns() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".git-std.toml"),
        "scheme = \"calver\"\n\n[versioning]\ncalver_format = \"INVALID\"\n",
    )
    .unwrap();

    let output = git_std()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .get_output()
        .clone();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("warning"),
        "should warn about invalid calver_format, got: {stderr}"
    );
}

// ── config get — unknown key ─────────────────────────────────────

#[test]
fn config_get_unknown_key_json_exits_1() {
    let dir = tempfile::tempdir().unwrap();
    git_std()
        .args(["config", "get", "nonexistent", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown config key"));
}

// ── config get — JSON stream behavior ────────────────────────────

#[test]
fn config_get_json_scheme_goes_to_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let output = git_std()
        .args(["config", "get", "scheme", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .get_output()
        .clone();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "JSON should be on stdout");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.is_empty(), "stderr should be empty in JSON mode");
}

#[test]
fn config_get_json_types_goes_to_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let output = git_std()
        .args(["config", "get", "types", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .get_output()
        .clone();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "JSON types should be on stdout");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.is_empty(),
        "stderr should be empty in JSON mode for types"
    );
}

#[test]
fn config_get_json_strict_goes_to_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let output = git_std()
        .args(["config", "get", "strict", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .get_output()
        .clone();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "JSON strict should be on stdout");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.is_empty(),
        "stderr should be empty in JSON mode for strict"
    );
}
