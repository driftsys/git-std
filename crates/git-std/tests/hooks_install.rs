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

/// Helper: initialise a git repo for hooks tests.
fn init_hooks_repo(dir: &Path) {
    git(dir, &["init"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@test.com"]);
}

// --- Hooks install integration tests ---

#[test]
fn hooks_install_creates_shims() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["init"])
        .env("GIT_STD_HOOKS_ENABLE", "pre-commit")
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("git hooks configured"));

    // Active shim should exist.
    let shim_path = hooks_dir.join("pre-commit");
    assert!(shim_path.exists(), "active shim should exist");

    // Shim should contain exec line and managed comment.
    let content = std::fs::read_to_string(&shim_path).unwrap();
    assert!(content.contains("exec git std hook run pre-commit"));
    assert!(content.contains("Managed by git-std"));

    // Other hooks should be .off.
    assert!(hooks_dir.join("commit-msg.off").exists());

    // Verify executable permissions on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::metadata(&shim_path).unwrap().permissions();
        assert!(perms.mode() & 0o111 != 0, "shim should be executable");
    }
}

#[test]
fn hooks_install_multiple_hooks() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["init"])
        .env("GIT_STD_HOOKS_ENABLE", "pre-commit,pre-push,commit-msg")
        .current_dir(dir.path())
        .assert()
        .success();

    // Selected shims should be active.
    assert!(hooks_dir.join("pre-commit").exists());
    assert!(hooks_dir.join("pre-push").exists());
    assert!(hooks_dir.join("commit-msg").exists());
    // Unselected should be .off.
    assert!(hooks_dir.join("post-commit.off").exists());
}

#[test]
fn hooks_install_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();

    // Run install twice.
    for _ in 0..2 {
        Command::cargo_bin("git-std")
            .unwrap()
            .args(["init"])
            .env("GIT_STD_HOOKS_ENABLE", "pre-commit")
            .current_dir(dir.path())
            .assert()
            .success();
    }

    // Shim should exist and contain exec line.
    let content = std::fs::read_to_string(hooks_dir.join("pre-commit")).unwrap();
    assert!(content.contains("exec git std hook run pre-commit"));
}

#[test]
fn hooks_install_preserves_non_hooks_files() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(hooks_dir.join("custom-script.sh"), "#!/bin/bash\necho hi\n").unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["init"])
        .env("GIT_STD_HOOKS_ENABLE", "pre-commit")
        .current_dir(dir.path())
        .assert()
        .success();

    // custom-script.sh should be untouched.
    let custom = std::fs::read_to_string(hooks_dir.join("custom-script.sh")).unwrap();
    assert_eq!(custom, "#!/bin/bash\necho hi\n");
}

// --- Hooks list integration tests ---

#[test]
fn hooks_list_shows_configured_hooks() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(
        hooks_dir.join("pre-commit.hooks"),
        "dprint check\ncargo clippy --workspace -- -D warnings *.rs\n? detekt --input modules/ *.kt\n",
    )
    .unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["hook", "list"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("pre-commit (collect mode)"),
        "should show hook name and mode, got: {stderr}"
    );
    assert!(stderr.contains("dprint check"), "should list commands");
    assert!(stderr.contains("*.rs"), "should show glob pattern");
    assert!(stderr.contains("?"), "should show advisory prefix");
}

#[test]
fn hooks_list_fail_fast_mode() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(
        hooks_dir.join("pre-push.hooks"),
        "!cargo build --workspace\n!cargo test --workspace\n",
    )
    .unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["hook", "list"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("pre-push (fail-fast mode)"),
        "should show fail-fast mode"
    );
    assert!(
        stderr.contains("! cargo build --workspace"),
        "should show fail-fast prefix"
    );
}

#[test]
fn hooks_list_commit_msg() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(
        hooks_dir.join("commit-msg.hooks"),
        "! git std lint --file {msg}\n",
    )
    .unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["hook", "list"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("commit-msg (fail-fast mode)"),
        "should show commit-msg with fail-fast mode"
    );
    assert!(
        stderr.contains("git std lint --file {msg}"),
        "should show command with {{msg}} token"
    );
}

#[test]
fn hooks_list_no_hooks() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    // No .githooks/ directory at all.
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["hook", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("no hooks installed"));
}

#[test]
fn hooks_list_multiple_hooks() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "dprint check\n").unwrap();
    std::fs::write(hooks_dir.join("pre-push.hooks"), "!cargo test\n").unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["hook", "list"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("pre-commit") && stderr.contains("pre-push"),
        "should list all hooks"
    );
}

// --- Additional acceptance tests for hooks install (#195) ---

#[test]
fn hooks_install_sets_core_hooks_path() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["init"])
        .env("GIT_STD_HOOKS_ENABLE", "none")
        .current_dir(dir.path())
        .assert()
        .success();

    let hooks_path = git(dir.path(), &["config", "core.hooksPath"]);
    assert_eq!(hooks_path, ".githooks");
}

#[test]
fn hooks_install_creates_githooks_dir() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    // Do NOT pre-create .githooks/ — the install command should create it.
    assert!(!dir.path().join(".githooks").exists());

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["init"])
        .env("GIT_STD_HOOKS_ENABLE", "none")
        .current_dir(dir.path())
        .assert()
        .success();

    assert!(
        dir.path().join(".githooks").exists(),
        ".githooks/ should be created by install"
    );
}

#[test]
fn hooks_install_enable_all() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["init"])
        .env("GIT_STD_HOOKS_ENABLE", "all")
        .current_dir(dir.path())
        .assert()
        .success();

    let hooks_dir = dir.path().join(".githooks");
    // All known hooks should be active (no .off).
    assert!(hooks_dir.join("pre-commit").exists());
    assert!(hooks_dir.join("commit-msg").exists());
    assert!(hooks_dir.join("pre-push").exists());
    assert!(!hooks_dir.join("pre-commit.off").exists());
}

// ── non-TTY guard (#316) ─────────────────────────────────────────

#[test]
fn hooks_install_non_tty_without_env_fails() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["init"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "interactive prompt requires a TTY",
        ))
        .stderr(predicate::str::contains("GIT_STD_HOOKS_ENABLE"));
}

#[test]
fn hooks_install_non_tty_with_env_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["init"])
        .env("GIT_STD_HOOKS_ENABLE", "all")
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn hooks_install_enable_none() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["init"])
        .env("GIT_STD_HOOKS_ENABLE", "none")
        .current_dir(dir.path())
        .assert()
        .success();

    let hooks_dir = dir.path().join(".githooks");
    // All hooks should be .off.
    assert!(!hooks_dir.join("pre-commit").exists());
    assert!(hooks_dir.join("pre-commit.off").exists());
}

// ── repo-root resolution (#318) ─────────────────────────────────

#[test]
fn hooks_list_from_subdirectory() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());
    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "dprint check\n").unwrap();
    let subdir = dir.path().join("src").join("nested");
    std::fs::create_dir_all(&subdir).unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["hook", "list"])
        .current_dir(&subdir)
        .assert()
        .success()
        .stderr(predicate::str::contains("pre-commit"));
}

#[test]
fn hooks_install_from_subdirectory() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());
    let subdir = dir.path().join("src");
    std::fs::create_dir_all(&subdir).unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["init"])
        .env("GIT_STD_HOOKS_ENABLE", "pre-commit")
        .current_dir(&subdir)
        .assert()
        .success();

    assert!(dir.path().join(".githooks").exists());
    assert!(!subdir.join(".githooks").exists());
}
