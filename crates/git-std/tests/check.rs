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
