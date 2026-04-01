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

/// Helper: get HEAD commit message.
fn head_message(dir: &Path) -> String {
    git(dir, &["log", "-1", "--format=%B"]).trim().to_string()
}

/// Helper: count commits in repo.
fn commit_count(dir: &Path) -> usize {
    let output = git(dir, &["rev-list", "--count", "HEAD"]);
    output.parse().unwrap()
}

/// Helper: initialise a git repo with one committed file (for commit tests).
fn init_commit_repo(dir: &Path) {
    git(dir, &["init"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@test.com"]);

    std::fs::write(dir.join("hello.txt"), "hello").unwrap();
    git(dir, &["add", "hello.txt"]);
    git(dir, &["commit", "-m", "chore: init"]);
}

#[test]
fn commit_dry_run_prints_message() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["commit", "--type", "feat", "-m", "add login", "--dry-run"])
        .assert()
        .success()
        .stderr(predicate::str::contains("feat: add login"));
}

#[test]
fn commit_dry_run_with_scope() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "commit",
            "--type",
            "fix",
            "--scope",
            "auth",
            "-m",
            "handle tokens",
            "--dry-run",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("fix(auth): handle tokens"));
}

#[test]
fn commit_dry_run_with_breaking() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "commit",
            "--type",
            "feat",
            "-m",
            "remove legacy API",
            "--breaking",
            "removed v1 endpoints",
            "--dry-run",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("feat!: remove legacy API"))
        .stderr(predicate::str::contains(
            "BREAKING CHANGE: removed v1 endpoints",
        ));
}

#[test]
fn commit_help_shows_flags() {
    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["commit", "--help"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    for flag in [
        "--type",
        "--scope",
        "--message",
        "--breaking",
        "--dry-run",
        "--amend",
        "--sign",
        "--all",
    ] {
        assert!(
            stdout.contains(flag),
            "commit help should list '{flag}' flag"
        );
    }
}

#[test]
fn commit_short_flags() {
    // -m alias for --message
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["commit", "--type", "feat", "-m", "short flag", "--dry-run"])
        .assert()
        .success()
        .stderr(predicate::str::contains("feat: short flag"));
}

#[test]
fn commit_fails_fast_when_stdin_is_not_a_tty() {
    // Piping stdin makes it non-TTY; without --type and --message the command
    // should fail immediately with a clear error rather than hanging.
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["commit"])
        .write_stdin("")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "interactive prompts require a TTY \u{2014} use --message to provide a commit message non-interactively",
        ));
}

#[test]
fn commit_non_interactive_with_type_and_message_works_in_piped_context() {
    // When --type and --message are both provided, no prompt is needed; the
    // command must succeed even with a piped (non-TTY) stdin.
    Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "commit",
            "--type",
            "feat",
            "-m",
            "no tty needed",
            "--dry-run",
        ])
        .write_stdin("")
        .assert()
        .success()
        .stderr(predicate::str::contains("feat: no tty needed"));
}

// --- Commit integration tests (actual repo) ---

#[test]
fn commit_actual_execution() {
    let dir = tempfile::tempdir().unwrap();
    init_commit_repo(dir.path());

    // Stage a new file so the commit has content.
    std::fs::write(dir.path().join("feature.txt"), "feature").unwrap();
    git(dir.path(), &["add", "feature.txt"]);

    Command::cargo_bin("git-std")
        .unwrap()
        .current_dir(dir.path())
        .args(["commit", "--type", "feat", "-m", "add feature"])
        .assert()
        .success();

    assert_eq!(head_message(dir.path()), "feat: add feature");
}

#[test]
fn commit_with_scope() {
    let dir = tempfile::tempdir().unwrap();
    init_commit_repo(dir.path());

    std::fs::write(dir.path().join("login.txt"), "login").unwrap();
    git(dir.path(), &["add", "login.txt"]);

    Command::cargo_bin("git-std")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "commit",
            "--type",
            "feat",
            "--scope",
            "auth",
            "-m",
            "add login",
        ])
        .assert()
        .success();

    assert_eq!(head_message(dir.path()), "feat(auth): add login");
}

#[test]
fn commit_with_breaking() {
    let dir = tempfile::tempdir().unwrap();
    init_commit_repo(dir.path());

    std::fs::write(dir.path().join("api.txt"), "new api").unwrap();
    git(dir.path(), &["add", "api.txt"]);

    Command::cargo_bin("git-std")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "commit",
            "--type",
            "feat",
            "--breaking",
            "remove old API",
            "-m",
            "new auth",
        ])
        .assert()
        .success();

    let msg = git(dir.path(), &["log", "-1", "--format=%B"]);
    assert!(msg.starts_with("feat!: new auth"), "got: {msg}");
    assert!(
        msg.contains("BREAKING CHANGE: remove old API"),
        "got: {msg}"
    );
}

#[test]
fn commit_amend() {
    let dir = tempfile::tempdir().unwrap();
    init_commit_repo(dir.path());

    // Create a commit to amend.
    std::fs::write(dir.path().join("bug.txt"), "bug").unwrap();
    git(dir.path(), &["add", "bug.txt"]);

    Command::cargo_bin("git-std")
        .unwrap()
        .current_dir(dir.path())
        .args(["commit", "--type", "fix", "-m", "original message"])
        .assert()
        .success();

    // Now amend it.
    Command::cargo_bin("git-std")
        .unwrap()
        .current_dir(dir.path())
        .args(["commit", "--amend", "--type", "fix", "-m", "corrected"])
        .assert()
        .success();

    assert_eq!(head_message(dir.path()), "fix: corrected");

    // Verify amend didn't create an extra commit — should be 2 total (init + amended).
    assert_eq!(
        commit_count(dir.path()),
        2,
        "amend should not create a new commit"
    );
}

#[test]
fn commit_all_flag() {
    let dir = tempfile::tempdir().unwrap();
    init_commit_repo(dir.path());

    // Modify the tracked file without staging.
    std::fs::write(dir.path().join("hello.txt"), "modified").unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .current_dir(dir.path())
        .args(["commit", "--all", "--type", "fix", "-m", "fix"])
        .assert()
        .success();

    assert_eq!(head_message(dir.path()), "fix: fix");

    // Verify the modified content was committed.
    let content = git(dir.path(), &["show", "HEAD:hello.txt"]);
    assert_eq!(content, "modified");
}

#[test]
fn commit_combined_flags() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "commit",
            "--type",
            "feat",
            "--scope",
            "auth",
            "-m",
            "add login",
            "--dry-run",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("feat(auth): add login"));
}

// ── commit --dry-run with auto-discover scopes (#72) ────────────

#[test]
fn commit_dry_run_auto_scopes() {
    let dir = tempfile::tempdir().unwrap();
    init_commit_repo(dir.path());
    std::fs::create_dir_all(dir.path().join("crates/web")).unwrap();
    std::fs::create_dir_all(dir.path().join("crates/api")).unwrap();
    std::fs::write(dir.path().join(".git-std.toml"), "scopes = \"auto\"\n").unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "commit",
            "--type",
            "feat",
            "--scope",
            "web",
            "--message",
            "add page",
            "--dry-run",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("feat(web): add page"));
}

// ── post-commit confirmation output (#220) ──────────────────────

#[test]
fn commit_prints_committed_confirmation() {
    let dir = tempfile::tempdir().unwrap();
    init_commit_repo(dir.path());

    std::fs::write(dir.path().join("new.txt"), "new").unwrap();
    git(dir.path(), &["add", "new.txt"]);

    let branch = git(dir.path(), &["rev-parse", "--abbrev-ref", "HEAD"]);

    Command::cargo_bin("git-std")
        .unwrap()
        .current_dir(dir.path())
        .args(["commit", "--type", "feat", "-m", "add new"])
        .assert()
        .success()
        .stderr(
            predicate::str::is_match(format!(r"committed \[{branch} [0-9a-f]{{7}}\]")).unwrap(),
        );
}

#[test]
fn commit_amend_prints_amended_confirmation() {
    let dir = tempfile::tempdir().unwrap();
    init_commit_repo(dir.path());

    std::fs::write(dir.path().join("fix.txt"), "fix").unwrap();
    git(dir.path(), &["add", "fix.txt"]);

    Command::cargo_bin("git-std")
        .unwrap()
        .current_dir(dir.path())
        .args(["commit", "--type", "fix", "-m", "original"])
        .assert()
        .success();

    let branch = git(dir.path(), &["rev-parse", "--abbrev-ref", "HEAD"]);

    Command::cargo_bin("git-std")
        .unwrap()
        .current_dir(dir.path())
        .args(["commit", "--amend", "--type", "fix", "-m", "corrected"])
        .assert()
        .success()
        .stderr(predicate::str::is_match(format!(r"amended \[{branch} [0-9a-f]{{7}}\]")).unwrap());
}
