use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

/// Helper: run a git command and return stdout.
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

/// Helper: initialise a git repo with a Cargo.toml and one commit.
fn init_bump_repo(dir: &Path) {
    git(dir, &["init"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@test.com"]);

    // Write a minimal Cargo.toml.
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"test-pkg\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    git(dir, &["add", "Cargo.toml"]);
    git(dir, &["commit", "-m", "chore: init"]);
}

/// Helper: add a commit to a repo.
fn add_commit(dir: &Path, filename: &str, message: &str) {
    std::fs::write(dir.join(filename), message).unwrap();
    git(dir, &["add", filename]);
    git(dir, &["commit", "-m", message]);
}

/// Helper: create an annotated tag.
fn create_tag(dir: &Path, name: &str) {
    git(dir, &["tag", "-a", name, "-m", name]);
}

// --- Changelog --range integration tests ---

#[test]
fn changelog_range_valid_between_two_tags() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    add_commit(dir.path(), "a.txt", "feat: add feature A");
    add_commit(dir.path(), "b.txt", "fix: fix bug B");
    create_tag(dir.path(), "v1.1.0");

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["changelog", "--range", "v1.0.0..v1.1.0", "--stdout"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("add feature A"),
        "should contain the feat commit, got: {stdout}"
    );
    assert!(
        stdout.contains("fix bug B"),
        "should contain the fix commit, got: {stdout}"
    );
    assert!(
        stdout.contains("1.1.0"),
        "should use the 'to' tag as version label, got: {stdout}"
    );
}

#[test]
fn changelog_range_invalid_missing_dotdot() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["changelog", "--range", "v1.0.0", "--stdout"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains("range must contain '..'"));
}

#[test]
fn changelog_range_invalid_ref() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());

    Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "changelog",
            "--range",
            "nonexistent..also-nonexistent",
            "--stdout",
        ])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains("cannot resolve"));
}

#[test]
fn changelog_range_warns_on_reversed_range() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    add_commit(dir.path(), "a.txt", "feat: add feature A");
    create_tag(dir.path(), "v1.1.0");

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["changelog", "--range", "v1.1.0..v1.0.0", "--stdout"])
        .current_dir(dir.path())
        .assert()
        .code(1);

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("warning:") && stderr.contains("is empty"),
        "should print a warning about empty range, got stderr: {stderr}"
    );
    assert!(
        stderr.contains("hint:") && stderr.contains("did you mean 'v1.0.0..v1.1.0'"),
        "should print a hint with the corrected range, got stderr: {stderr}"
    );
}

#[test]
fn changelog_range_no_conventional_commits() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Add only non-conventional commits.
    add_commit(dir.path(), "a.txt", "random message with no type");
    create_tag(dir.path(), "v1.0.1");

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["changelog", "--range", "v1.0.0..v1.0.1", "--stdout"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("no conventional commits found"),
        "should report no conventional commits, got stderr: {stderr}"
    );
}

#[test]
fn changelog_range_no_warning_for_same_commit_tags() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Create a second tag on the same commit.
    git(dir.path(), &["tag", "-a", "v1.0.1", "-m", "v1.0.1"]);

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["changelog", "--range", "v1.0.0..v1.0.1", "--stdout"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        !stderr.contains("warning:"),
        "should not print a warning for same-commit tags, got stderr: {stderr}"
    );
    assert!(
        stderr.contains("no conventional commits found"),
        "should report no conventional commits, got stderr: {stderr}"
    );
}
