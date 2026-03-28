use assert_cmd::Command;
use predicates::str::contains;

fn git_std() -> Command {
    Command::cargo_bin("git-std").unwrap()
}

fn git(dir: &std::path::Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success());
}

fn init_repo(dir: &std::path::Path) {
    git(dir, &["init"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@test.com"]);
}

#[test]
fn doctor_appears_in_help() {
    git_std()
        .args(["--help"])
        .assert()
        .success()
        .stdout(contains("doctor"));
}

#[test]
fn doctor_exits_1_in_git_repo_without_hooks() {
    // A bare git repo has no .githooks/ and no core.hooksPath — expect fail.
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .code(1);
}

// ===========================================================================
// #323 — hooks health checks
// ===========================================================================

#[test]
fn doctor_hooks_pass_when_fully_configured() {
    // Repo with .githooks/, core.hooksPath set, shim present
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    git(dir.path(), &["config", "core.hooksPath", ".githooks"]);
    std::fs::write(dir.path().join(".githooks/bootstrap.hooks"), "").unwrap();

    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("hooks"));
}

#[test]
fn doctor_hooks_fail_when_githooks_dir_missing() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    // No .githooks/ directory

    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("hooks"));
}

#[test]
fn doctor_hooks_fail_when_hooks_path_not_configured() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    // .githooks/ exists but core.hooksPath not set

    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("core.hooksPath"));
}

#[test]
fn doctor_exits_2_outside_git_repo() {
    let dir = tempfile::tempdir().unwrap();
    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .code(2);
}

// ===========================================================================
// #324 — bootstrap health checks
// ===========================================================================

#[test]
fn doctor_bootstrap_warns_when_no_convention_files() {
    // Fresh repo with no convention files — Warn but not Fail
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    // Set up hooks so hooks section passes
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    git(dir.path(), &["config", "core.hooksPath", ".githooks"]);

    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .success(); // Warn does not cause failure
}

#[test]
fn doctor_bootstrap_pass_when_blame_ignore_revs_configured() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    git(dir.path(), &["config", "core.hooksPath", ".githooks"]);
    std::fs::write(dir.path().join(".git-blame-ignore-revs"), "").unwrap();
    git(
        dir.path(),
        &["config", "blame.ignoreRevsFile", ".git-blame-ignore-revs"],
    );

    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("bootstrap"));
}

#[test]
fn doctor_bootstrap_fail_when_blame_ignore_revs_not_configured() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    git(dir.path(), &["config", "core.hooksPath", ".githooks"]);
    // .git-blame-ignore-revs exists but blame.ignoreRevsFile not configured
    std::fs::write(dir.path().join(".git-blame-ignore-revs"), "").unwrap();

    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("blame.ignoreRevsFile"));
}

// ===========================================================================
// #325 — config health checks
// ===========================================================================

#[test]
fn doctor_config_warn_when_no_config_file() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    git(dir.path(), &["config", "core.hooksPath", ".githooks"]);
    // No .git-std.toml — should be Warn, not Fail

    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .success(); // Warn does not fail
}

#[test]
fn doctor_config_pass_when_valid_config() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    git(dir.path(), &["config", "core.hooksPath", ".githooks"]);
    std::fs::write(
        dir.path().join(".git-std.toml"),
        "[versioning]\ntag_prefix = \"v\"\n",
    )
    .unwrap();

    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("config"));
}

#[test]
fn doctor_config_fail_when_invalid_toml() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    git(dir.path(), &["config", "core.hooksPath", ".githooks"]);
    std::fs::write(dir.path().join(".git-std.toml"), "[[invalid toml = bad\n").unwrap();

    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("config"));
}
