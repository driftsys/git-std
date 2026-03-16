use assert_cmd::Command;
use predicates::str::contains;

fn git_std() -> Command {
    Command::cargo_bin("git-std").unwrap()
}

// ── Valid messages exit 0 ────────────────────────────────────────

#[test]
fn valid_simple_message() {
    git_std()
        .args(["check", "feat: add login"])
        .assert()
        .success();
}

#[test]
fn valid_scoped_message() {
    git_std()
        .args(["check", "feat(auth): add PKCE"])
        .assert()
        .success();
}

#[test]
fn valid_breaking_bang() {
    git_std()
        .args(["check", "feat!: remove legacy API"])
        .assert()
        .success();
}

#[test]
fn valid_with_body() {
    git_std()
        .args([
            "check",
            "fix(core): handle nil pointer\n\nAdded nil check before dereferencing the config pointer.",
        ])
        .assert()
        .success();
}

#[test]
fn valid_with_breaking_change_footer() {
    git_std()
        .args([
            "check",
            "feat: change token format\n\nBREAKING CHANGE: tokens are now opaque strings",
        ])
        .assert()
        .success();
}

// ── Invalid messages exit 1 with diagnostic ─────────────────────

#[test]
fn invalid_missing_type() {
    git_std()
        .args(["check", "bad message"])
        .assert()
        .code(1)
        .stderr(contains("invalid"));
}

#[test]
fn invalid_missing_description() {
    git_std()
        .args(["check", "feat: "])
        .assert()
        .code(1)
        .stderr(contains("invalid"));
}

#[test]
fn invalid_no_colon() {
    git_std()
        .args(["check", "feat add login"])
        .assert()
        .code(1)
        .stderr(contains("invalid"));
}

#[test]
fn invalid_uppercase_type() {
    git_std()
        .args(["check", "FEAT: add login"])
        .assert()
        .code(1)
        .stderr(contains("invalid"));
}

#[test]
fn diagnostic_shows_expected_format() {
    git_std()
        .args(["check", "not a valid commit"])
        .assert()
        .code(1)
        .stderr(contains("Expected: <type>(<scope>): <description>"));
}

#[test]
fn diagnostic_shows_got_line() {
    git_std()
        .args(["check", "not a valid commit"])
        .assert()
        .code(1)
        .stderr(contains("Got:"));
}

// ── check --file (#11) ─────────────────────────────────────────

#[test]
fn file_valid_message() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("COMMIT_EDITMSG");
    std::fs::write(&path, "feat: add login\n").unwrap();

    git_std()
        .args(["check", "--file", path.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn file_strips_comment_lines() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("COMMIT_EDITMSG");
    std::fs::write(
        &path,
        "feat: add login\n# Please enter the commit message\n# Lines starting with '#' will be ignored\n",
    )
    .unwrap();

    git_std()
        .args(["check", "--file", path.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn file_invalid_message() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("COMMIT_EDITMSG");
    std::fs::write(&path, "bad message\n").unwrap();

    git_std()
        .args(["check", "--file", path.to_str().unwrap()])
        .assert()
        .code(1)
        .stderr(contains("invalid"));
}

#[test]
fn file_not_found_exits_2() {
    git_std()
        .args(["check", "--file", "/nonexistent/path"])
        .assert()
        .code(2)
        .stderr(contains("cannot read"));
}

#[test]
fn file_with_body_and_comments() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("COMMIT_EDITMSG");
    std::fs::write(
        &path,
        "feat: add OAuth2 PKCE flow\n\nImplements the full PKCE authorization code flow.\n# On branch main\n# Changes to be committed:\n",
    )
    .unwrap();

    git_std()
        .args(["check", "--file", path.to_str().unwrap()])
        .assert()
        .success();
}

// ── check --range (#12) ────────────────────────────────────────

fn make_test_repo(dir: &std::path::Path) -> git2::Repository {
    let repo = git2::Repository::init(dir).unwrap();
    let mut config = repo.config().unwrap();
    config.set_str("user.name", "Test").unwrap();
    config.set_str("user.email", "test@test.com").unwrap();
    repo
}

fn create_commit(
    repo: &git2::Repository,
    dir: &std::path::Path,
    message: &str,
    content: &str,
) -> git2::Oid {
    let sig = repo.signature().unwrap();
    let path = dir.join("file.txt");
    std::fs::write(&path, content).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("file.txt")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();

    let parents: Vec<git2::Commit> = if let Ok(head) = repo.head() {
        vec![head.peel_to_commit().unwrap()]
    } else {
        vec![]
    };
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
        .unwrap()
}

#[test]
fn range_all_valid_exits_0() {
    let dir = tempfile::tempdir().unwrap();
    let repo = make_test_repo(dir.path());

    let initial = create_commit(&repo, dir.path(), "feat: initial commit", "hello");
    create_commit(&repo, dir.path(), "fix: correct typo", "world");

    let range = format!("{}..HEAD", &initial.to_string()[..7]);

    git_std()
        .args(["check", "--range", &range])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("\u{2713}"));
}

#[test]
fn range_invalid_commit_exits_1() {
    let dir = tempfile::tempdir().unwrap();
    let repo = make_test_repo(dir.path());

    let initial = create_commit(&repo, dir.path(), "feat: initial", "hello");
    create_commit(&repo, dir.path(), "bad commit message", "world");

    let range = format!("{}..HEAD", &initial.to_string()[..7]);

    git_std()
        .args(["check", "--range", &range])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("\u{2717}"));
}

#[test]
fn range_mixed_reports_both() {
    let dir = tempfile::tempdir().unwrap();
    let repo = make_test_repo(dir.path());

    let initial = create_commit(&repo, dir.path(), "feat: initial", "a");
    create_commit(&repo, dir.path(), "fix: valid one", "b");
    create_commit(&repo, dir.path(), "invalid message", "c");

    let range = format!("{}..HEAD", &initial.to_string()[..7]);

    git_std()
        .args(["check", "--range", &range])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("\u{2713}"))
        .stderr(contains("\u{2717}"));
}

#[test]
fn range_invalid_range_exits_2() {
    git_std()
        .args(["check", "--range", "nonexistent..also-nonexistent"])
        .assert()
        .code(2);
}

// ── .git-std.toml types (#13) + --strict (#14) ──────────────────

#[test]
fn strict_accepts_default_types_without_config() {
    let dir = tempfile::tempdir().unwrap();
    // No .git-std.toml — should use default types
    git_std()
        .args(["check", "--strict", "feat: add login"])
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn strict_rejects_unknown_type_without_config() {
    let dir = tempfile::tempdir().unwrap();
    git_std()
        .args(["check", "--strict", "yolo: do things"])
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
        .args(["check", "--strict", "custom: do something"])
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
        .args(["check", "--strict", "docs: update readme"])
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
        .args(["check", "--strict", "feat: add login"])
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
        .args(["check", "--strict", "feat(unknown): add login"])
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
        .args(["check", "--strict", "feat(auth): add login"])
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
        .args(["check", "yolo: do things"])
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
        .args(["check", "--strict", "--file", msg_path.to_str().unwrap()])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("not in the allowed list"));
}

// ── --format json (#20) ─────────────────────────────────────────

#[test]
fn json_valid_simple() {
    let output = git_std()
        .args(["check", "--format", "json", "feat: add login"])
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
            "check",
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
        .args(["check", "--format", "json", "bad message"])
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
        .args(["check", "--strict", "--format", "json", "yolo: do things"])
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
        .args(["--color", "never", "check", "feat: add login"])
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
        .args(["--color", "never", "check", "bad message"])
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
    let _repo = make_test_repo(dir.path());
    std::fs::create_dir_all(dir.path().join("crates/auth")).unwrap();
    std::fs::write(dir.path().join(".git-std.toml"), "scopes = \"auto\"\n").unwrap();

    git_std()
        .args(["check", "--strict", "feat(auth): add login"])
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn strict_auto_scopes_rejects_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let _repo = make_test_repo(dir.path());
    std::fs::create_dir_all(dir.path().join("crates/auth")).unwrap();
    std::fs::write(dir.path().join(".git-std.toml"), "scopes = \"auto\"\n").unwrap();

    git_std()
        .args(["check", "--strict", "feat(unknown): something"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("not in the allowed list"));
}

#[test]
fn strict_auto_scopes_requires_scope() {
    let dir = tempfile::tempdir().unwrap();
    let _repo = make_test_repo(dir.path());
    std::fs::create_dir_all(dir.path().join("crates/auth")).unwrap();
    std::fs::write(dir.path().join(".git-std.toml"), "scopes = \"auto\"\n").unwrap();

    git_std()
        .args(["check", "--strict", "feat: no scope"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("scope is required"));
}

#[test]
fn strict_auto_scopes_empty_dirs_no_requirement() {
    let dir = tempfile::tempdir().unwrap();
    let _repo = make_test_repo(dir.path());
    // No crates/packages/modules directories
    std::fs::write(dir.path().join(".git-std.toml"), "scopes = \"auto\"\n").unwrap();

    git_std()
        .args(["check", "--strict", "feat: anything"])
        .current_dir(dir.path())
        .assert()
        .success();
}
