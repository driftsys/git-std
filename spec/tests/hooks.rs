#[path = "../support/mod.rs"]
mod support;

use snapbox::cmd::Command;
use snapbox::file;
use support::TestRepo;

/// `hook list` displays configured hooks with their mode and commands.
#[test]
fn hooks_list_shows_configured_hooks() {
    let repo = TestRepo::new().with_hooks_file(
        "pre-commit",
        "dprint check\ncargo clippy --workspace -- -D warnings *.rs\n",
    );

    Command::new(TestRepo::bin_path())
        .args(["hook", "list"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/list_shows_configured_hooks.stderr.expected"
        ]);
}

/// `hook list` shows fail-fast mode for pre-push hooks.
#[test]
fn hooks_list_fail_fast_mode() {
    let repo = TestRepo::new().with_hooks_file(
        "pre-push",
        "!cargo build --workspace\n!cargo test --workspace\n",
    );

    Command::new(TestRepo::bin_path())
        .args(["hook", "list"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/list_fail_fast_mode.stderr.expected"
        ]);
}

/// `hook list` with no hooks configured prints a message to stderr.
#[test]
fn hooks_list_no_hooks() {
    let repo = TestRepo::new();

    Command::new(TestRepo::bin_path())
        .args(["hook", "list"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file!["../snapshots/hooks/list_no_hooks.stderr.expected"]);
}

/// `git std init` creates shim scripts for each `.hooks` file.
#[test]
fn init_creates_shims() {
    let repo = TestRepo::new().with_hooks_file("pre-commit", "dprint check\ncargo test\n");

    Command::new(TestRepo::bin_path())
        .args(["init"])
        .env("GIT_STD_HOOKS_ENABLE", "pre-commit")
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/init_creates_shims.stderr.expected"
        ]);

    // Verify shim exists.
    let shim_path = repo.path().join(".githooks/pre-commit");
    assert!(shim_path.exists(), "shim should exist");
}

/// `hook run` shows pass, fail, and advisory results in collect mode.
#[test]
fn hooks_run_pass_fail_advisory() {
    let repo = TestRepo::new().with_hooks_file("pre-commit", "true\n?false\n!false\n");

    Command::new(TestRepo::bin_path())
        .args(["hook", "run", "pre-commit"])
        .current_dir(repo.path())
        .assert()
        .code(1)
        .stderr_eq(file![
            "../snapshots/hooks/run_pass_fail_advisory.stderr.expected"
        ]);
}

/// `hook run` skips execution when GIT_STD_SKIP_HOOKS=1 is set.
#[test]
fn hooks_run_skip_via_env_var() {
    let repo = TestRepo::new().with_hooks_file("pre-commit", "false\n");

    Command::new(TestRepo::bin_path())
        .args(["hook", "run", "pre-commit"])
        .env("GIT_STD_SKIP_HOOKS", "1")
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/run_skip_via_env_var.stderr.expected"
        ]);
}

/// `hook run` skips execution when GIT_STD_SKIP_HOOKS=true is set.
#[test]
fn hooks_run_skip_via_env_var_true() {
    let repo = TestRepo::new().with_hooks_file("pre-commit", "false\n");

    Command::new(TestRepo::bin_path())
        .args(["hook", "run", "pre-commit"])
        .env("GIT_STD_SKIP_HOOKS", "true")
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/run_skip_via_env_var.stderr.expected"
        ]);
}

/// `hook run` displays glob patterns and skips commands that don't match.
#[test]
fn hooks_run_glob_filtering() {
    let mut repo = TestRepo::new().with_hooks_file("pre-push", "true *.txt\ntrue *.py\n");
    // add_commit creates file-1.txt, so *.txt will match and *.py won't.
    repo.add_commit("chore: init");

    Command::new(TestRepo::bin_path())
        .args(["hook", "run", "pre-push"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/run_glob_filtering.stderr.expected"
        ]);
}

/// `hook run` correctly handles staged renames with fix mode (#387).
/// The stash dance corrupts renames by splitting them, but we repair
/// them by re-staging the old name as a deletion after formatting.
#[test]
fn hooks_run_fix_mode_handles_staged_renames() {
    let mut repo = TestRepo::new().with_hooks_file("pre-commit", "~ echo 'format check'\n");
    repo.add_commit("chore: init");

    // Create and commit a file, then rename it to stage the rename.
    let original_file = "original.txt";
    std::fs::write(repo.path().join(original_file), "content").expect("failed to write file");
    std::process::Command::new("git")
        .args(["add", original_file])
        .current_dir(repo.path())
        .status()
        .expect("failed to add file");
    std::process::Command::new("git")
        .args(["commit", "-m", "chore: add file to rename"])
        .current_dir(repo.path())
        .status()
        .expect("failed to commit file");

    // Now stage a rename
    let renamed_file = "renamed.txt";
    std::process::Command::new("git")
        .args(["mv", original_file, renamed_file])
        .current_dir(repo.path())
        .status()
        .expect("failed to rename file");

    // Run pre-commit hook with a fix command (~).
    // Should succeed and repair the rename corruption from stash apply.
    let output = std::process::Command::new(TestRepo::bin_path())
        .args(["hook", "run", "pre-commit"])
        .current_dir(repo.path())
        .output()
        .expect("failed to run hook run");

    assert!(
        output.status.success(),
        "hook run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    // Verify the fix command ran successfully
    assert!(
        stderr.contains("echo 'format check'"),
        "expected fix command to run, stderr: {stderr}"
    );
    // Verify no warning about the old name being formatted
    assert!(
        !stderr.contains(&format!(
            "{original_file}: unstaged changes were also formatted"
        )),
        "should not warn about old filename: {stderr}"
    );

    // Verify the rename is properly staged for commit
    let git_status = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo.path())
        .output()
        .expect("failed to get git status");
    let status = String::from_utf8_lossy(&git_status.stdout);
    // Should show the rename, not separate delete and add
    assert!(
        status.contains("R ") && status.contains(original_file) && status.contains(renamed_file),
        "should show rename in git status, got: {status}"
    );
}
