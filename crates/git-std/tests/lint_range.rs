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

// ── lint --range (#12) ─────────────────────────────────────────

#[test]
fn range_all_valid_exits_0() {
    let dir = tempfile::tempdir().unwrap();
    make_test_repo(dir.path());

    let initial = create_commit(dir.path(), "feat: initial commit", "hello");
    create_commit(dir.path(), "fix: correct typo", "world");

    let range = format!("{}..HEAD", &initial[..7]);

    git_std()
        .args(["lint", "--range", &range])
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
        .args(["lint", "--range", &range])
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
        .args(["lint", "--range", &range])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("\u{2713}"))
        .stderr(contains("\u{2717}"));
}

#[test]
fn range_invalid_range_exits_2() {
    git_std()
        .args(["lint", "--range", "nonexistent..also-nonexistent"])
        .assert()
        .code(2);
}

// ── empty range is a no-op (#545) ──────────────────────────────

#[test]
fn range_empty_exits_0() {
    let dir = tempfile::tempdir().unwrap();
    make_test_repo(dir.path());

    create_commit(dir.path(), "feat: initial commit", "hello");

    git_std()
        .args(["lint", "--range", "HEAD..HEAD"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout("")
        .stderr(contains("no commits in range 'HEAD..HEAD'"));
}

#[test]
fn range_empty_json_outputs_empty_array() {
    let dir = tempfile::tempdir().unwrap();
    make_test_repo(dir.path());

    create_commit(dir.path(), "feat: initial commit", "hello");

    git_std()
        .args(["lint", "--range", "HEAD..HEAD", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout("[]\n")
        // JSON mode is machine output: an empty range reports nothing as an error.
        .stderr("");
}

#[test]
fn range_reversed_json_still_emits_array() {
    let dir = tempfile::tempdir().unwrap();
    make_test_repo(dir.path());

    create_commit(dir.path(), "feat: initial commit", "hello");
    create_commit(dir.path(), "fix: second commit", "world");

    git_std()
        .args(["lint", "--range", "HEAD..HEAD~1", "--format", "json"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        // Machine output stays a valid array even when the range is rejected.
        .stdout("[]\n")
        .stderr(contains("warning: range 'HEAD..HEAD~1' is empty"))
        .stderr(contains("did you mean 'HEAD~1..HEAD'?"));
}

#[test]
fn range_reversed_text_writes_nothing_to_stdout() {
    let dir = tempfile::tempdir().unwrap();
    make_test_repo(dir.path());

    create_commit(dir.path(), "feat: initial commit", "hello");
    create_commit(dir.path(), "fix: second commit", "world");

    git_std()
        .args(["lint", "--range", "HEAD..HEAD~1"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stdout("")
        .stderr(contains("did you mean 'HEAD~1..HEAD'?"));
}

#[test]
fn range_without_separator_lints_all_reachable_commits() {
    let dir = tempfile::tempdir().unwrap();
    make_test_repo(dir.path());

    create_commit(dir.path(), "feat: initial commit", "hello");
    create_commit(dir.path(), "fix: second commit", "world");

    // Unlike `changelog --range`, a range need not contain '..'.
    git_std()
        .args(["lint", "--range", "HEAD"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("2/2 valid"));
}

#[test]
fn range_with_omitted_endpoint_lints_commits() {
    let dir = tempfile::tempdir().unwrap();
    make_test_repo(dir.path());

    create_commit(dir.path(), "feat: initial commit", "hello");
    create_commit(dir.path(), "fix: second commit", "world");

    // git reads an omitted endpoint as HEAD, so `<ref>..` is a real range.
    git_std()
        .args(["lint", "--range", "HEAD~1.."])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("1/1 valid"));
}
