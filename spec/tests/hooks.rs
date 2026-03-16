#[path = "../support/mod.rs"]
mod support;

use snapbox::cmd::Command;
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
        .stdout_eq("...\n[..] pre-commit (collect mode):[..]\n...");
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
        .stdout_eq("...\n[..] pre-push (fail-fast mode):[..]\n...");
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
        .stderr_eq("[..] no hooks configured\n");
}

/// `hooks install` creates shim scripts for each `.hooks` file.
#[test]
fn hooks_install_creates_shims() {
    let repo = TestRepo::new().with_hooks_file("pre-commit", "dprint check\ncargo test\n");

    Command::new(TestRepo::bin_path())
        .args(["hooks", "install"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq("...\n[..] core.hooksPath [..]\n...");

    // Verify shim exists.
    let shim_path = repo.path().join(".githooks/pre-commit");
    assert!(shim_path.exists(), "shim should exist");
}
