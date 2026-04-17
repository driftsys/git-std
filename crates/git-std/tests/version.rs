use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn git(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn init_repo(dir: &Path) {
    git(dir, &["init"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@test.com"]);
    // Write a minimal Cargo.toml so the repo looks like a Rust project.
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"test-pkg\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    git(dir, &["add", "Cargo.toml"]);
    git(dir, &["commit", "-m", "chore: init"]);
}

fn add_commit(dir: &Path, filename: &str, message: &str) {
    std::fs::write(dir.join(filename), message).unwrap();
    git(dir, &["add", filename]);
    git(dir, &["commit", "-m", message]);
}

fn create_tag(dir: &Path, name: &str) {
    git(dir, &["tag", "-a", name, "-m", name]);
}

fn git_std(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("git-std").unwrap();
    cmd.current_dir(dir);
    cmd
}

fn write_calver_config(dir: &Path) {
    std::fs::write(dir.join(".git-std.toml"), "scheme = \"calver\"\n").unwrap();
    git(dir, &["add", ".git-std.toml"]);
    git(dir, &["commit", "-m", "chore: add calver config"]);
}

// ---------------------------------------------------------------------------
// Help / usage
// ---------------------------------------------------------------------------

#[test]
fn version_help_shows_flags() {
    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["version", "--help"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    for flag in ["--describe", "--next", "--label", "--code", "--format"] {
        assert!(
            stdout.contains(flag),
            "version help should list '{flag}' flag"
        );
    }
}

// ---------------------------------------------------------------------------
// Bare version — semver
// ---------------------------------------------------------------------------

#[test]
fn version_bare_prints_current_semver() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    create_tag(dir.path(), "v1.2.3");

    git_std(dir.path())
        .arg("version")
        .assert()
        .success()
        .stdout("1.2.3\n");
}

#[test]
fn version_bare_no_v_prefix() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    create_tag(dir.path(), "v0.10.2");

    let output = git_std(dir.path())
        .arg("version")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&output);
    assert!(
        !text.trim().starts_with('v'),
        "output must not have v prefix"
    );
    assert_eq!(text.trim(), "0.10.2");
}

#[test]
fn version_no_tag_exits_with_error() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    git_std(dir.path())
        .arg("version")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no version tag found"));
}

// ---------------------------------------------------------------------------
// --describe
// ---------------------------------------------------------------------------

#[test]
fn version_describe_at_tag_is_clean() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    git_std(dir.path())
        .args(["version", "--describe"])
        .assert()
        .success()
        .stdout("1.0.0\n");
}

#[test]
fn version_describe_ahead_includes_distance_and_hash() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "feat: something");
    add_commit(dir.path(), "b.txt", "fix: another");

    let output = git_std(dir.path())
        .args(["version", "--describe"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&output).trim().to_string();

    assert!(
        text.starts_with("1.0.0-dev.2+g"),
        "describe should have -dev.2+g prefix, got: {text}"
    );
}

#[test]
fn version_describe_dirty_tree() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Create an unstaged file to make the tree dirty
    std::fs::write(dir.path().join("dirty.txt"), "uncommitted").unwrap();

    let output = git_std(dir.path())
        .args(["version", "--describe"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&output).trim().to_string();

    assert!(
        text.ends_with(".dirty"),
        "describe should end with .dirty for dirty tree, got: {text}"
    );
}

// ---------------------------------------------------------------------------
// --next
// ---------------------------------------------------------------------------

#[test]
fn version_next_feat_gives_minor_bump() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "feat: add something");

    git_std(dir.path())
        .args(["version", "--next"])
        .assert()
        .success()
        .stdout("1.1.0\n");
}

#[test]
fn version_next_fix_gives_patch_bump() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "fix: something");

    git_std(dir.path())
        .args(["version", "--next"])
        .assert()
        .success()
        .stdout("1.0.1\n");
}

#[test]
fn version_next_no_bump_worthy_commits_prints_current() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "chore: cleanup");

    git_std(dir.path())
        .args(["version", "--next"])
        .assert()
        .success()
        .stdout("1.0.0\n");
}

#[test]
fn version_next_pre1_breaking_gives_minor() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    create_tag(dir.path(), "v0.10.2");
    add_commit(dir.path(), "a.txt", "feat!: breaking change");

    git_std(dir.path())
        .args(["version", "--next"])
        .assert()
        .success()
        .stdout("0.11.0\n");
}

// ---------------------------------------------------------------------------
// --label
// ---------------------------------------------------------------------------

#[test]
fn version_label_feat_gives_minor() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "feat: add feature");

    git_std(dir.path())
        .args(["version", "--label"])
        .assert()
        .success()
        .stdout("minor\n");
}

#[test]
fn version_label_breaking_pre1_gives_minor() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    create_tag(dir.path(), "v0.5.0");
    add_commit(dir.path(), "a.txt", "feat!: breaking");

    git_std(dir.path())
        .args(["version", "--label"])
        .assert()
        .success()
        .stdout("minor\n");
}

#[test]
fn version_label_no_commits_gives_none() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "chore: nothing");

    git_std(dir.path())
        .args(["version", "--label"])
        .assert()
        .success()
        .stdout("none\n");
}

// ---------------------------------------------------------------------------
// --code
// ---------------------------------------------------------------------------

#[test]
fn version_code_stable_semver() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    create_tag(dir.path(), "v0.10.2");

    // 0.10.2 stable: ((0*1000+10)*100+2)*100+99 = 1002*100+99 = 100299
    git_std(dir.path())
        .args(["version", "--code"])
        .assert()
        .success()
        .stdout("100299\n");
}

#[test]
fn version_code_outputs_integer() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    let output = git_std(dir.path())
        .args(["version", "--code"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&output).trim().to_string();
    assert!(
        text.parse::<u64>().is_ok(),
        "--code output should be an integer, got: {text}"
    );
}

// ---------------------------------------------------------------------------
// --format json
// ---------------------------------------------------------------------------

#[test]
fn version_format_json_has_all_fields() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "feat: add feature");

    let output = git_std(dir.path())
        .args(["version", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8_lossy(&output);
    let val: serde_json::Value = serde_json::from_str(text.trim()).expect("valid JSON");

    assert_eq!(val["version"], "1.0.0");
    assert!(val["next"].is_string(), "next should be a string");
    assert!(val["label"].is_string(), "label should be a string");
    assert!(val["code"].is_number(), "code should be a number");
}

#[test]
fn version_format_json_version_no_v_prefix() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    create_tag(dir.path(), "v2.0.0");

    let output = git_std(dir.path())
        .args(["version", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let val: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&output).trim()).unwrap();
    assert_eq!(val["version"], "2.0.0");
}

// ---------------------------------------------------------------------------
// Multiple flags
// ---------------------------------------------------------------------------

#[test]
fn version_multiple_flags_each_printed() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "feat: feature");

    // Both --next and --label should produce two output lines.
    let output = git_std(dir.path())
        .args(["version", "--next", "--label"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&output);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "expected two output lines, got: {text}");
    assert_eq!(lines[0], "1.1.0");
    assert_eq!(lines[1], "minor");
}

// ---------------------------------------------------------------------------
// Calver — bare version
// ---------------------------------------------------------------------------

#[test]
fn calver_bare_version() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    write_calver_config(dir.path());
    create_tag(dir.path(), "v2026.3.0");

    git_std(dir.path())
        .arg("version")
        .assert()
        .success()
        .stdout("2026.3.0\n");
}

#[test]
fn calver_no_tag_fails() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    write_calver_config(dir.path());

    git_std(dir.path())
        .arg("version")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no version tag found"));
}

// ---------------------------------------------------------------------------
// Calver — --describe
// ---------------------------------------------------------------------------

#[test]
fn calver_describe_at_tag() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    write_calver_config(dir.path());
    create_tag(dir.path(), "v2026.3.0");

    git_std(dir.path())
        .args(["version", "--describe"])
        .assert()
        .success()
        .stdout("2026.3.0\n");
}

#[test]
fn calver_describe_ahead() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    write_calver_config(dir.path());
    create_tag(dir.path(), "v2026.3.0");
    add_commit(dir.path(), "a.txt", "feat: something");

    let output = git_std(dir.path())
        .args(["version", "--describe"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&output).trim().to_string();
    assert!(
        text.starts_with("2026.3.0-dev.1+g"),
        "calver describe should have -dev.1+g prefix, got: {text}"
    );
}

// ---------------------------------------------------------------------------
// Calver — --next
// ---------------------------------------------------------------------------

#[test]
fn calver_next_computes_new_version() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    write_calver_config(dir.path());
    create_tag(dir.path(), "v2026.3.0");
    add_commit(dir.path(), "a.txt", "feat: feature");

    let output = git_std(dir.path())
        .args(["version", "--next"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&output).trim().to_string();
    // Next calver version should start with the current year/month.
    assert!(
        text.chars().next().unwrap().is_ascii_digit(),
        "calver next should be a date-based version, got: {text}"
    );
}

// ---------------------------------------------------------------------------
// Calver — --code
// ---------------------------------------------------------------------------

#[test]
fn calver_code_returns_numeric() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    write_calver_config(dir.path());
    create_tag(dir.path(), "v2026.3.0");

    let output = git_std(dir.path())
        .args(["version", "--code"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&output).trim().to_string();
    let code: u64 = text
        .parse()
        .unwrap_or_else(|_| panic!("calver code should be numeric, got: {text}"));
    assert!(code > 0, "calver code should be positive");
}

// ---------------------------------------------------------------------------
// Calver — --format json
// ---------------------------------------------------------------------------

#[test]
fn calver_format_json() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    write_calver_config(dir.path());
    create_tag(dir.path(), "v2026.3.0");

    let output = git_std(dir.path())
        .args(["version", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&output);
    let json: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("calver json should be valid JSON: {e}\n{text}"));
    assert_eq!(json["version"], "2026.3.0");
    assert_eq!(json["label"], "calver");
}

// ---------------------------------------------------------------------------
// Calver — --label
// ---------------------------------------------------------------------------

#[test]
fn calver_label_is_calver() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    write_calver_config(dir.path());
    create_tag(dir.path(), "v2026.3.0");
    add_commit(dir.path(), "a.txt", "feat: feature");

    git_std(dir.path())
        .args(["version", "--label"])
        .assert()
        .success()
        .stdout("calver\n");
}
