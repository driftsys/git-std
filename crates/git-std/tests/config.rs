use assert_cmd::Command;
use predicates::str::contains;

fn git_std() -> Command {
    Command::cargo_bin("git-std").unwrap()
}

// ── config list (defaults) ────────────────────────────────────────

#[test]
fn config_list_defaults_exits_0() {
    let dir = tempfile::tempdir().unwrap();
    git_std()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn config_list_shows_scheme() {
    let dir = tempfile::tempdir().unwrap();
    git_std()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("scheme"));
}

#[test]
fn config_list_shows_versioning_section() {
    let dir = tempfile::tempdir().unwrap();
    git_std()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("[versioning]"))
        .stderr(contains("tag_prefix"));
}

#[test]
fn config_list_shows_changelog_section() {
    let dir = tempfile::tempdir().unwrap();
    git_std()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("[changelog]"))
        .stderr(contains("hidden"));
}

#[test]
fn config_list_source_is_default_when_no_file() {
    let dir = tempfile::tempdir().unwrap();
    git_std()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("(default)"));
}

#[test]
fn config_list_json_outputs_to_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let output = git_std()
        .args(["config", "list", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .get_output()
        .clone();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("stdout should be JSON");
    assert!(parsed.get("scheme").is_some());
    assert!(parsed.get("versioning").is_some());
    assert!(parsed.get("changelog").is_some());
}

// ── config list with .git-std.toml ───────────────────────────────

#[test]
fn config_list_shows_file_source_for_set_keys() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".git-std.toml"), "strict = true\n").unwrap();

    git_std()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains(".git-std.toml"));
}

#[test]
fn config_list_reflects_toml_values() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".git-std.toml"), "scheme = \"calver\"\n").unwrap();

    git_std()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("calver"));
}

// ── config get ───────────────────────────────────────────────────

#[test]
fn config_get_scheme_returns_semver_by_default() {
    let dir = tempfile::tempdir().unwrap();
    git_std()
        .args(["config", "get", "scheme"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("semver"));
}

#[test]
fn config_get_strict_returns_false_by_default() {
    let dir = tempfile::tempdir().unwrap();
    git_std()
        .args(["config", "get", "strict"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("false"));
}

#[test]
fn config_get_versioning_tag_prefix() {
    let dir = tempfile::tempdir().unwrap();
    git_std()
        .args(["config", "get", "versioning.tag_prefix"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("v"));
}

#[test]
fn config_get_changelog_title() {
    let dir = tempfile::tempdir().unwrap();
    git_std()
        .args(["config", "get", "changelog.title"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("Changelog"));
}

#[test]
fn config_get_types_lists_defaults() {
    let dir = tempfile::tempdir().unwrap();
    git_std()
        .args(["config", "get", "types"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("feat"))
        .stderr(contains("fix"));
}

#[test]
fn config_get_unknown_key_exits_1() {
    let dir = tempfile::tempdir().unwrap();
    git_std()
        .args(["config", "get", "nonexistent.key"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("unknown config key"));
}

#[test]
fn config_get_json_format_outputs_json_string() {
    let dir = tempfile::tempdir().unwrap();
    let output = git_std()
        .args(["config", "get", "scheme", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .get_output()
        .clone();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("output should be JSON");
    assert_eq!(parsed, serde_json::Value::String("semver".to_string()));
}

#[test]
fn config_get_json_types_outputs_array() {
    let dir = tempfile::tempdir().unwrap();
    let output = git_std()
        .args(["config", "get", "types", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .get_output()
        .clone();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("output should be JSON array");
    assert!(parsed.is_array());
    let arr = parsed.as_array().unwrap();
    assert!(arr.contains(&serde_json::Value::String("feat".to_string())));
}

#[test]
fn config_get_reflects_toml_value() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".git-std.toml"), "strict = true\n").unwrap();

    git_std()
        .args(["config", "get", "strict"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("true"));
}

// ── config subcommand requires subcommand ────────────────────────

#[test]
fn config_without_subcommand_exits_2() {
    git_std().arg("config").assert().code(2);
}
