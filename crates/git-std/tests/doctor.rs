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
        .success() // Warn does not fail
        .stderr(contains("config"))
        .stderr(contains("⚠"));
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
        .stderr(contains("config"))
        .stderr(contains("hint:"));
}

// ===========================================================================
// #326 — --format json
// ===========================================================================

#[test]
fn doctor_json_outputs_to_stdout() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    git(dir.path(), &["config", "core.hooksPath", ".githooks"]);

    let output = git_std()
        .args(["doctor", "--format", "json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert!(parsed.get("status").is_some());
    assert!(parsed.get("sections").is_some());
    assert!(
        output.stderr.is_empty(),
        "stderr should be empty in JSON mode"
    );
}

#[test]
fn doctor_json_fail_status_when_checks_fail() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    // No .githooks/ — hooks checks will fail

    let output = git_std()
        .args(["doctor", "--format", "json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "fail");
    assert!(
        output.stderr.is_empty(),
        "stderr should be empty in JSON mode"
    );
}

// ===========================================================================
// #320 — subdirectory and worktree invocation
// ===========================================================================

#[test]
fn doctor_from_subdirectory() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    git(dir.path(), &["config", "core.hooksPath", ".githooks"]);
    std::fs::write(
        dir.path().join(".git-std.toml"),
        "[versioning]\ntag_prefix = \"v\"\n",
    )
    .unwrap();

    let subdir = dir.path().join("src");
    std::fs::create_dir_all(&subdir).unwrap();

    git_std()
        .args(["doctor"])
        .current_dir(&subdir)
        .assert()
        .success()
        .stderr(contains("hooks"))
        .stderr(contains("config"));
}

#[test]
fn doctor_from_git_worktree() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    // Set up a valid repo with hooks, config, and commit them so the
    // worktree's working tree will contain these tracked files.
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    std::fs::write(dir.path().join(".githooks/.gitkeep"), "").unwrap();
    std::fs::write(
        dir.path().join(".git-std.toml"),
        "[versioning]\ntag_prefix = \"v\"\n",
    )
    .unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-m", "initial commit"]);

    // Set core.hooksPath (this is a local git config, shared across worktrees).
    git(dir.path(), &["config", "core.hooksPath", ".githooks"]);

    // Create a real git worktree inside a separate tempdir so paths never collide.
    let wt_parent = tempfile::tempdir().unwrap();
    let wt_path = wt_parent.path().join("worktree-test");
    git(
        dir.path(),
        &[
            "worktree",
            "add",
            wt_path.to_str().unwrap(),
            "-b",
            "test-branch",
        ],
    );

    // The worktree should have a .git file (not a directory).
    let git_entry = wt_path.join(".git");
    assert!(git_entry.exists(), ".git should exist in worktree");
    assert!(
        git_entry.is_file(),
        ".git in worktree should be a file, not a directory"
    );

    // Tracked files should be visible in the worktree.
    assert!(
        wt_path.join(".githooks").exists(),
        ".githooks/ should be present in worktree via tracked files"
    );

    // Run doctor from the worktree — should find repo-level config.
    git_std()
        .args(["doctor"])
        .current_dir(&wt_path)
        .assert()
        .success()
        .stderr(contains("hooks"))
        .stderr(contains("config"));
}

// ===========================================================================
// #341 — LFS-absent skip path (bootstrap section)
// ===========================================================================

#[test]
fn doctor_bootstrap_skips_lfs_when_gitattributes_has_no_lfs() {
    // .gitattributes exists but contains no filter=lfs token.
    // The LFS check should be skipped entirely — doctor must exit 0.
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    git(dir.path(), &["config", "core.hooksPath", ".githooks"]);
    std::fs::write(dir.path().join(".gitattributes"), "*.png binary\n").unwrap();

    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("bootstrap"));
}

// ===========================================================================
// #347 — JSON structure: per-check status and hint omission
// ===========================================================================

#[test]
fn doctor_json_check_status_and_hint_omitted_when_unset() {
    // Repo with hooks fully configured so at least one check passes without a hint.
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    git(dir.path(), &["config", "core.hooksPath", ".githooks"]);

    let output = git_std()
        .args(["doctor", "--format", "json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");

    // Every check in every section must have a "status" field.
    let sections = parsed["sections"]
        .as_array()
        .expect("sections must be array");
    for section in sections {
        let checks = section["checks"].as_array().expect("checks must be array");
        for check in checks {
            assert!(
                check.get("status").is_some(),
                "each check must have a status field: {check}"
            );
            // Checks without a hint must omit the key entirely (not serialize as null).
            if check.get("hint").is_some() {
                assert!(
                    check["hint"].is_string(),
                    "hint must be a string when present: {check}"
                );
            }
        }
    }

    // The .githooks/ directory check passes with no hint; verify at least one
    // "pass" status exists to confirm the shape contract.
    let has_pass = sections.iter().any(|s| {
        s["checks"]
            .as_array()
            .map(|cs| cs.iter().any(|c| c["status"] == "pass"))
            .unwrap_or(false)
    });
    assert!(has_pass, "expected at least one check with status=pass");
}
