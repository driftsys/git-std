use assert_cmd::Command;
use predicates::str::contains;

fn git_std() -> Command {
    Command::cargo_bin("git-std").unwrap()
}

fn make_test_repo(dir: &std::path::Path) {
    git(dir, &["init"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@test.com"]);
}

fn git(dir: &std::path::Path, args: &[&str]) -> String {
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

// ── .git-std.toml types (#13) + --strict (#14) ──────────────────

#[test]
fn strict_accepts_default_types_without_config() {
    let dir = tempfile::tempdir().unwrap();
    // No .git-std.toml — should use default types
    git_std()
        .args(["lint", "--strict", "feat: add login"])
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn strict_rejects_unknown_type_without_config() {
    let dir = tempfile::tempdir().unwrap();
    git_std()
        .args(["lint", "--strict", "yolo: do things"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("not in the allowed list"));
}

#[test]
fn strict_with_custom_types_accepts_custom() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".git-std.toml"),
        "types = [\"feat\", \"fix\", \"custom\"]\n",
    )
    .unwrap();

    git_std()
        .args(["lint", "--strict", "custom: do something"])
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn strict_with_custom_types_rejects_unlisted() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".git-std.toml"),
        "types = [\"feat\", \"fix\"]\n",
    )
    .unwrap();

    git_std()
        .args(["lint", "--strict", "docs: update readme"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("not in the allowed list"));
}

#[test]
fn strict_with_scopes_requires_scope() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".git-std.toml"),
        "scopes = [\"auth\", \"api\"]\n",
    )
    .unwrap();

    // No scope provided — should fail
    git_std()
        .args(["lint", "--strict", "feat: add login"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("scope is required"));
}

#[test]
fn strict_with_scopes_rejects_unknown_scope() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".git-std.toml"),
        "scopes = [\"auth\", \"api\"]\n",
    )
    .unwrap();

    git_std()
        .args(["lint", "--strict", "feat(unknown): add login"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("not in the allowed list"));
}

#[test]
fn strict_with_scopes_accepts_valid_scope() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".git-std.toml"),
        "scopes = [\"auth\", \"api\"]\n",
    )
    .unwrap();

    git_std()
        .args(["lint", "--strict", "feat(auth): add login"])
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn without_strict_any_type_accepted() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".git-std.toml"), "types = [\"feat\"]\n").unwrap();

    // Without --strict, custom types pass (only parse validation)
    git_std()
        .args(["lint", "yolo: do things"])
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn strict_file_validates_against_config() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".git-std.toml"),
        "types = [\"feat\", \"fix\"]\n",
    )
    .unwrap();
    let msg_path = dir.path().join("COMMIT_EDITMSG");
    std::fs::write(&msg_path, "docs: update readme\n").unwrap();

    git_std()
        .args(["lint", "--strict", "--file", msg_path.to_str().unwrap()])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("not in the allowed list"));
}

// ── --format json (#20) ─────────────────────────────────────────

#[test]
fn json_valid_simple() {
    let output = git_std()
        .args(["lint", "--format", "json", "feat: add login"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["valid"], true);
    assert_eq!(v["type"], "feat");
    assert_eq!(v["scope"], serde_json::Value::Null);
    assert_eq!(v["description"], "add login");
    assert_eq!(v["breaking"], false);
    assert!(v["errors"].as_array().unwrap().is_empty());
}

#[test]
fn json_valid_scoped_breaking() {
    let output = git_std()
        .args([
            "lint",
            "--format",
            "json",
            "refactor(runtime)!: drop Python 2 support",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["valid"], true);
    assert_eq!(v["type"], "refactor");
    assert_eq!(v["scope"], "runtime");
    assert_eq!(v["breaking"], true);
}

#[test]
fn json_invalid_message() {
    let output = git_std()
        .args(["lint", "--format", "json", "bad message"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["valid"], false);
    assert!(!v["errors"].as_array().unwrap().is_empty());
}

#[test]
fn json_invalid_strict() {
    let dir = tempfile::tempdir().unwrap();
    let output = git_std()
        .args(["lint", "--strict", "--format", "json", "yolo: do things"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["valid"], false);
    assert!(
        v["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e.as_str().unwrap().contains("not in the allowed list"))
    );
}

// ── --color never (#21) ─────────────────────────────────────────

#[test]
fn color_never_no_ansi_codes_valid() {
    let output = git_std()
        .args(["--color", "never", "lint", "feat: add login"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // ANSI escape sequences start with ESC (0x1B)
    assert!(
        !stderr.contains('\x1b'),
        "stderr should not contain ANSI escape codes with --color never"
    );
    assert!(stderr.contains("\u{2713}"));
}

#[test]
fn color_never_no_ansi_codes_invalid() {
    let output = git_std()
        .args(["--color", "never", "lint", "bad message"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains('\x1b'),
        "stderr should not contain ANSI escape codes with --color never"
    );
    assert!(stderr.contains("\u{2717}"));
}

// ── auto-discover scopes (#72) ──────────────────────────────────

#[test]
fn strict_auto_scopes_accepts_discovered() {
    let dir = tempfile::tempdir().unwrap();
    make_test_repo(dir.path());
    std::fs::create_dir_all(dir.path().join("crates/auth")).unwrap();
    std::fs::write(dir.path().join(".git-std.toml"), "scopes = \"auto\"\n").unwrap();

    git_std()
        .args(["lint", "--strict", "feat(auth): add login"])
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn strict_auto_scopes_rejects_unknown() {
    let dir = tempfile::tempdir().unwrap();
    make_test_repo(dir.path());
    std::fs::create_dir_all(dir.path().join("crates/auth")).unwrap();
    std::fs::write(dir.path().join(".git-std.toml"), "scopes = \"auto\"\n").unwrap();

    git_std()
        .args(["lint", "--strict", "feat(unknown): something"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("not in the allowed list"));
}

#[test]
fn strict_auto_scopes_requires_scope() {
    let dir = tempfile::tempdir().unwrap();
    make_test_repo(dir.path());
    std::fs::create_dir_all(dir.path().join("crates/auth")).unwrap();
    std::fs::write(dir.path().join(".git-std.toml"), "scopes = \"auto\"\n").unwrap();

    git_std()
        .args(["lint", "--strict", "feat: no scope"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("scope is required"));
}

#[test]
fn strict_auto_scopes_empty_dirs_no_requirement() {
    let dir = tempfile::tempdir().unwrap();
    make_test_repo(dir.path());
    // No crates/packages/modules directories
    std::fs::write(dir.path().join(".git-std.toml"), "scopes = \"auto\"\n").unwrap();

    git_std()
        .args(["lint", "--strict", "feat: anything"])
        .current_dir(dir.path())
        .assert()
        .success();
}
