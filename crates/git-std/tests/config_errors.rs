// Tests for configuration error handling — now surfaced via `git std doctor`
// since the `config` subcommand was removed in #403.

use assert_cmd::Command;
use predicates::prelude::*;

fn git_std() -> Command {
    Command::cargo_bin("git-std").unwrap()
}

fn init_git_repo(dir: &std::path::Path) {
    std::process::Command::new("git")
        .current_dir(dir)
        .args(["init"])
        .status()
        .unwrap();
}

// ── Malformed TOML ───────────────────────────────────────────────────────────

#[test]
fn doctor_malformed_toml_shows_hint() {
    let dir = tempfile::tempdir().unwrap();
    init_git_repo(dir.path());
    std::fs::write(dir.path().join(".git-std.toml"), "[[broken toml = {\n").unwrap();

    let output = git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .get_output()
        .clone();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("hint:"),
        "should show a hint for invalid TOML, got: {stderr}"
    );
    assert!(
        stderr.contains(".git-std.toml invalid"),
        "hint should mention .git-std.toml, got: {stderr}"
    );
}

#[test]
fn doctor_malformed_toml_json_still_outputs_valid_json() {
    let dir = tempfile::tempdir().unwrap();
    init_git_repo(dir.path());
    std::fs::write(dir.path().join(".git-std.toml"), "broken [\n").unwrap();

    let output = git_std()
        .args(["doctor", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .get_output()
        .clone();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("should still output valid JSON");

    // Hints should list the error.
    let hints = parsed["hints"].as_array().expect("should have hints array");
    assert!(
        !hints.is_empty(),
        "should have at least one hint for malformed TOML"
    );

    // Configuration section should still be present with defaults.
    let config = parsed["sections"]["configuration"]
        .as_array()
        .expect("should have configuration section");
    let scheme = config
        .iter()
        .find(|r| r["key"] == "scheme")
        .expect("scheme should be present");
    assert_eq!(scheme["value"], "semver", "should fall back to default");
}

// ── Invalid value types (silently fall back to defaults) ─────────────────────

#[test]
fn doctor_scheme_wrong_type_falls_back_to_default() {
    let dir = tempfile::tempdir().unwrap();
    init_git_repo(dir.path());
    // scheme expects a string; provide an integer.
    std::fs::write(dir.path().join(".git-std.toml"), "scheme = 123\n").unwrap();

    // No .githooks/ → exit 1, but config section still rendered.
    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains("semver"));
}

#[test]
fn doctor_strict_wrong_type_falls_back_to_default() {
    let dir = tempfile::tempdir().unwrap();
    init_git_repo(dir.path());
    // strict expects a boolean; provide a string.
    std::fs::write(dir.path().join(".git-std.toml"), "strict = \"yes\"\n").unwrap();

    // No .githooks/ → exit 1, but config section still rendered.
    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains("false"));
}

// ── Invalid calver_format ────────────────────────────────────────────────────

#[test]
fn doctor_invalid_calver_format_still_shows_defaults() {
    let dir = tempfile::tempdir().unwrap();
    init_git_repo(dir.path());
    std::fs::write(
        dir.path().join(".git-std.toml"),
        "scheme = \"calver\"\n\n[versioning]\ncalver_format = \"INVALID\"\n",
    )
    .unwrap();

    // No .githooks/ → exit 1; calver_format warning is a stderr warning, not a hint.
    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .code(1);
}
