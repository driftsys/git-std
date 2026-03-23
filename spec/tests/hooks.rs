#[path = "../support/mod.rs"]
mod support;

use snapbox::cmd::Command;
use snapbox::file;
use support::TestRepo;

/// `hooks list` displays configured hooks with their mode and commands.
#[test]
fn hooks_list_shows_configured_hooks() {
    let repo = TestRepo::new().with_hooks_file(
        "pre-commit",
        "dprint check\ncargo clippy --workspace -- -D warnings *.rs\n",
    );

    Command::new(TestRepo::bin_path())
        .args(["hooks", "list"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/list_shows_configured_hooks.stderr.expected"
        ]);
}

/// `hooks list` shows fail-fast mode for pre-push hooks.
#[test]
fn hooks_list_fail_fast_mode() {
    let repo = TestRepo::new().with_hooks_file(
        "pre-push",
        "!cargo build --workspace\n!cargo test --workspace\n",
    );

    Command::new(TestRepo::bin_path())
        .args(["hooks", "list"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/list_fail_fast_mode.stderr.expected"
        ]);
}

/// `hooks list` with no hooks configured prints a message to stderr.
#[test]
fn hooks_list_no_hooks() {
    let repo = TestRepo::new();

    Command::new(TestRepo::bin_path())
        .args(["hooks", "list"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file!["../snapshots/hooks/list_no_hooks.stderr.expected"]);
}

/// `hooks install` creates shim scripts for each `.hooks` file.
#[test]
fn hooks_install_creates_shims() {
    let repo = TestRepo::new().with_hooks_file("pre-commit", "dprint check\ncargo test\n");

    Command::new(TestRepo::bin_path())
        .args(["hooks", "install"])
        .env("GIT_STD_HOOKS_ENABLE", "pre-commit")
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/install_creates_shims.stderr.expected"
        ]);

    // Verify shim exists.
    let shim_path = repo.path().join(".githooks/pre-commit");
    assert!(shim_path.exists(), "shim should exist");
}

/// `hooks run` shows pass, fail, and advisory results in collect mode.
#[test]
fn hooks_run_pass_fail_advisory() {
    let repo = TestRepo::new().with_hooks_file("pre-commit", "true\n?false\n!false\n");

    Command::new(TestRepo::bin_path())
        .args(["hooks", "run", "pre-commit"])
        .current_dir(repo.path())
        .assert()
        .code(1)
        .stderr_eq(file![
            "../snapshots/hooks/run_pass_fail_advisory.stderr.expected"
        ]);
}

/// `hooks run` skips execution when GIT_STD_SKIP_HOOKS=1 is set.
#[test]
fn hooks_run_skip_via_env_var() {
    let repo = TestRepo::new().with_hooks_file("pre-commit", "false\n");

    Command::new(TestRepo::bin_path())
        .args(["hooks", "run", "pre-commit"])
        .env("GIT_STD_SKIP_HOOKS", "1")
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/run_skip_via_env_var.stderr.expected"
        ]);
}

/// `hooks run` skips execution when GIT_STD_SKIP_HOOKS=true is set.
#[test]
fn hooks_run_skip_via_env_var_true() {
    let repo = TestRepo::new().with_hooks_file("pre-commit", "false\n");

    Command::new(TestRepo::bin_path())
        .args(["hooks", "run", "pre-commit"])
        .env("GIT_STD_SKIP_HOOKS", "true")
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/run_skip_via_env_var.stderr.expected"
        ]);
}

/// `hooks run` displays glob patterns and skips commands that don't match.
#[test]
fn hooks_run_glob_filtering() {
    let mut repo = TestRepo::new().with_hooks_file("pre-push", "true *.txt\ntrue *.py\n");
    // add_commit creates file-1.txt, so *.txt will match and *.py won't.
    repo.add_commit("chore: init");

    Command::new(TestRepo::bin_path())
        .args(["hooks", "run", "pre-push"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/run_glob_filtering.stderr.expected"
        ]);
}
