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

fn create_commit(dir: &std::path::Path, message: &str, content: &str) -> String {
    std::fs::write(dir.join("file.txt"), content).unwrap();
    git(dir, &["add", "file.txt"]);
    git(dir, &["commit", "-m", message]);
    git(dir, &["rev-parse", "HEAD"])
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

// ── check --range (#12) ────────────────────────────────────────

#[test]
fn range_all_valid_exits_0() {
    let dir = tempfile::tempdir().unwrap();
    make_test_repo(dir.path());

    let initial = create_commit(dir.path(), "feat: initial commit", "hello");
    create_commit(dir.path(), "fix: correct typo", "world");

    let range = format!("{}..HEAD", &initial[..7]);

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
    make_test_repo(dir.path());

    let initial = create_commit(dir.path(), "feat: initial", "hello");
    create_commit(dir.path(), "bad commit message", "world");

    let range = format!("{}..HEAD", &initial[..7]);

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
    make_test_repo(dir.path());

    let initial = create_commit(dir.path(), "feat: initial", "a");
    create_commit(dir.path(), "fix: valid one", "b");
    create_commit(dir.path(), "invalid message", "c");

    let range = format!("{}..HEAD", &initial[..7]);

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
