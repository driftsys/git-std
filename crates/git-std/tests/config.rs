// The `config` subcommand was removed in #403 (absorbed into `doctor`).
// These tests verify that `config` no longer exists and that its
// functionality is available through `git std doctor`.

use assert_cmd::Command;
use predicates::str::contains;

fn git_std() -> Command {
    Command::cargo_bin("git-std").unwrap()
}

// ── config subcommand is gone ────────────────────────────────────────

#[test]
fn config_subcommand_no_longer_exists() {
    // `git std config list` should fail with an unknown subcommand error.
    git_std().args(["config", "list"]).assert().code(2);
}

#[test]
fn config_not_listed_in_help() {
    let output = git_std()
        .args(["--help"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // `config` should no longer appear as a subcommand in the help text.
    // NOTE: The word "config" may still appear in descriptions, so we
    // check that it does not appear as a standalone subcommand entry.
    assert!(
        !stdout.contains("  config     "),
        "config should not appear as a subcommand in help"
    );
}

// ── doctor replaces config ───────────────────────────────────────────

#[test]
fn doctor_shows_configuration_section() {
    let dir = tempfile::tempdir().unwrap();
    let status = std::process::Command::new("git")
        .current_dir(dir.path())
        .args(["init"])
        .status()
        .unwrap();
    assert!(status.success());

    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("Configuration"))
        .stderr(contains("scheme"))
        .stderr(contains("semver"));
}

#[test]
fn doctor_shows_explicit_config_values() {
    let dir = tempfile::tempdir().unwrap();
    let status = std::process::Command::new("git")
        .current_dir(dir.path())
        .args(["init"])
        .status()
        .unwrap();
    assert!(status.success());
    std::fs::write(dir.path().join(".git-std.toml"), "scheme = \"calver\"\n").unwrap();

    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("calver"));
}

#[test]
fn doctor_json_has_configuration_section() {
    let dir = tempfile::tempdir().unwrap();
    let status = std::process::Command::new("git")
        .current_dir(dir.path())
        .args(["init"])
        .status()
        .unwrap();
    assert!(status.success());

    let output = git_std()
        .args(["doctor", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .get_output()
        .clone();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("should be JSON");
    let configuration = parsed["sections"]["configuration"]
        .as_array()
        .expect("sections.configuration should be an array");
    let scheme = configuration
        .iter()
        .find(|r| r["key"] == "scheme")
        .expect("scheme should be present");
    assert_eq!(scheme["value"], "semver");
    assert_eq!(scheme["source"], "default");
}
