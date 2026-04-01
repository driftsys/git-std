//! Integration tests for `git std init` (#400).

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
}

fn run_init(dir: &Path, extra_args: &[&str]) -> assert_cmd::assert::Assert {
    let mut args = vec!["--color", "never", "init"];
    args.extend_from_slice(extra_args);
    Command::cargo_bin("git-std")
        .unwrap()
        .args(&args)
        .env("GIT_STD_HOOKS_ENABLE", "none")
        .current_dir(dir)
        .assert()
}

fn stderr_text(assert: &assert_cmd::assert::Assert) -> String {
    String::from_utf8_lossy(&assert.get_output().stderr).to_string()
}

// ===========================================================================
// init from scratch
// ===========================================================================

#[test]
fn init_from_scratch_creates_all_files() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    run_init(dir.path(), &[]).success();

    // core.hooksPath is set
    let hooks_path = git(dir.path(), &["config", "core.hooksPath"]);
    assert_eq!(hooks_path, ".githooks");

    // .githooks/ directory exists
    assert!(
        dir.path().join(".githooks").is_dir(),
        ".githooks/ should exist"
    );

    // .hooks templates written for known hooks
    for hook in &["pre-commit", "commit-msg", "pre-push"] {
        let tpl = dir.path().join(format!(".githooks/{hook}.hooks"));
        assert!(tpl.exists(), "{hook}.hooks template should exist");
    }

    // All shims are .off (since GIT_STD_HOOKS_ENABLE=none)
    assert!(dir.path().join(".githooks/pre-commit.off").exists());
    assert!(dir.path().join(".githooks/commit-msg.off").exists());

    // ./bootstrap script created
    let bootstrap = dir.path().join("bootstrap");
    assert!(bootstrap.exists(), "bootstrap script should exist");

    // .githooks/bootstrap.hooks created
    assert!(
        dir.path().join(".githooks/bootstrap.hooks").exists(),
        "bootstrap.hooks should exist"
    );
}

#[test]
fn init_sets_core_hooks_path() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    run_init(dir.path(), &[]).success();

    let val = git(dir.path(), &["config", "core.hooksPath"]);
    assert_eq!(val, ".githooks");
}

#[test]
fn init_creates_githooks_directory() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    assert!(!dir.path().join(".githooks").exists());

    run_init(dir.path(), &[]).success();

    assert!(
        dir.path().join(".githooks").is_dir(),
        ".githooks/ should be created"
    );
}

#[test]
fn init_creates_bootstrap_script_executable() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    run_init(dir.path(), &[]).success();

    let bootstrap = dir.path().join("bootstrap");
    assert!(bootstrap.exists(), "bootstrap should exist");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&bootstrap).unwrap().permissions().mode();
        assert!(mode & 0o111 != 0, "bootstrap should be executable");
    }

    // bootstrap script content should reference current version
    let content = std::fs::read_to_string(&bootstrap).unwrap();
    let version = env!("CARGO_PKG_VERSION");
    assert!(
        content.contains(&format!("MIN_VERSION=\"{version}\"")),
        "MIN_VERSION should match crate version"
    );
}

#[test]
fn init_stages_created_files() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    run_init(dir.path(), &[]).success();

    let staged = git(dir.path(), &["diff", "--cached", "--name-only"]);
    assert!(
        staged.contains("bootstrap"),
        "bootstrap should be staged, got: {staged}"
    );
    assert!(
        staged.contains(".githooks"),
        ".githooks/ contents should be staged, got: {staged}"
    );
}

#[test]
fn init_appends_bootstrap_marker_to_readme() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::write(dir.path().join("README.md"), "# My Project\n").unwrap();

    run_init(dir.path(), &[]).success();

    let content = std::fs::read_to_string(dir.path().join("README.md")).unwrap();
    assert!(
        content.contains("<!-- git-std:bootstrap -->"),
        "README.md should have bootstrap marker"
    );
    assert!(
        content.contains("./bootstrap"),
        "README.md should mention ./bootstrap"
    );
}

#[test]
fn init_appends_marker_to_agents_md() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::write(dir.path().join("AGENTS.md"), "# Agents\n").unwrap();

    run_init(dir.path(), &[]).success();

    let content = std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
    assert!(
        content.contains("<!-- git-std:bootstrap -->"),
        "AGENTS.md should have bootstrap marker"
    );
}

#[test]
fn init_enables_selected_hooks() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "init"])
        .env("GIT_STD_HOOKS_ENABLE", "pre-commit,commit-msg")
        .current_dir(dir.path())
        .assert()
        .success();

    let hooks_dir = dir.path().join(".githooks");
    assert!(
        hooks_dir.join("pre-commit").exists(),
        "pre-commit shim active"
    );
    assert!(
        hooks_dir.join("commit-msg").exists(),
        "commit-msg shim active"
    );
    assert!(!hooks_dir.join("pre-commit.off").exists());
    assert!(hooks_dir.join("pre-push.off").exists(), "pre-push disabled");
}

#[test]
fn init_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    // Run twice — both should succeed
    run_init(dir.path(), &[]).success();
    run_init(dir.path(), &[]).success();

    let val = git(dir.path(), &["config", "core.hooksPath"]);
    assert_eq!(val, ".githooks");
}

#[test]
fn init_non_tty_without_env_fails() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "init"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "interactive prompt requires a TTY",
        ))
        .stderr(predicate::str::contains("GIT_STD_HOOKS_ENABLE"));
}

#[test]
fn init_outputs_hooks_configured() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let a = run_init(dir.path(), &[]).success();
    let err = stderr_text(&a);
    assert!(
        err.contains("git hooks configured"),
        "should confirm hooks configured, got: {err}"
    );
}

#[test]
fn init_outputs_bootstrap_created() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let a = run_init(dir.path(), &[]).success();
    let err = stderr_text(&a);
    assert!(
        err.contains("bootstrap created"),
        "should confirm bootstrap created, got: {err}"
    );
}

// ===========================================================================
// init --force
// ===========================================================================

#[test]
fn init_skips_existing_files_without_force() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    // Pre-create files
    std::fs::write(dir.path().join("bootstrap"), "existing content\n").unwrap();
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    std::fs::write(
        dir.path().join(".githooks/bootstrap.hooks"),
        "existing hooks\n",
    )
    .unwrap();

    let a = run_init(dir.path(), &[]).success();
    let err = stderr_text(&a);
    assert!(
        err.contains("already exists"),
        "should warn about existing files, got: {err}"
    );

    // Content should be unchanged
    let content = std::fs::read_to_string(dir.path().join("bootstrap")).unwrap();
    assert_eq!(content, "existing content\n");
}

#[test]
fn init_force_overwrites_existing_files() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    // Pre-create files with old content
    std::fs::write(dir.path().join("bootstrap"), "old content\n").unwrap();
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    std::fs::write(dir.path().join(".githooks/bootstrap.hooks"), "old hooks\n").unwrap();

    run_init(dir.path(), &["--force"]).success();

    // bootstrap should have new content
    let content = std::fs::read_to_string(dir.path().join("bootstrap")).unwrap();
    assert!(
        content.contains("MIN_VERSION"),
        "bootstrap should have new content after --force, got: {content}"
    );

    // bootstrap.hooks should have new content
    let hooks_content =
        std::fs::read_to_string(dir.path().join(".githooks/bootstrap.hooks")).unwrap();
    assert!(
        hooks_content.contains("git std bootstrap"),
        "bootstrap.hooks should have template content"
    );
}

#[test]
fn init_force_does_not_double_append_marker() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::write(dir.path().join("README.md"), "# Project\n").unwrap();

    // Run twice with --force
    run_init(dir.path(), &["--force"]).success();
    run_init(dir.path(), &["--force"]).success();

    let content = std::fs::read_to_string(dir.path().join("README.md")).unwrap();
    let count = content.matches("<!-- git-std:bootstrap -->").count();
    assert_eq!(count, 1, "marker should appear exactly once, found {count}");
}

#[test]
fn init_from_subdirectory() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let subdir = dir.path().join("src");
    std::fs::create_dir_all(&subdir).unwrap();

    run_init(&subdir, &[]).success();

    // Files should be at repo root, not subdirectory
    assert!(
        dir.path().join("bootstrap").exists(),
        "bootstrap should be at repo root"
    );
    assert!(
        dir.path().join(".githooks").is_dir(),
        ".githooks should be at repo root"
    );
    assert!(
        !subdir.join("bootstrap").exists(),
        "bootstrap should not be in subdir"
    );
    assert!(
        !subdir.join(".githooks").exists(),
        ".githooks should not be in subdir"
    );
}

#[test]
fn init_not_in_git_repo_fails() {
    let dir = tempfile::tempdir().unwrap();
    // No git init

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "init"])
        .env("GIT_STD_HOOKS_ENABLE", "none")
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not inside a git repository"));
}
