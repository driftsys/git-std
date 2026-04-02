//! Integration tests for `git std --context`.
//!
//! Covers all five status states, workspace grouping, JSON output, and worktree.

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
    assert!(status.success(), "git {} failed", args.join(" "));
}

fn init_repo(dir: &std::path::Path) {
    git(dir, &["init"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@test.com"]);
}

fn commit_file(dir: &std::path::Path, name: &str, msg: &str) {
    std::fs::write(dir.join(name), "content").unwrap();
    git(dir, &["add", name]);
    git(dir, &["commit", "-m", msg]);
}

// ===========================================================================
// Basic smoke tests
// ===========================================================================

#[test]
fn context_flag_shows_in_help() {
    git_std()
        .args(["--help"])
        .assert()
        .success()
        .stdout(contains("--context"));
}

#[test]
fn context_exits_2_outside_git_repo() {
    let dir = tempfile::tempdir().unwrap();
    git_std()
        .args(["--context"])
        .current_dir(dir.path())
        .assert()
        .code(2);
}

#[test]
fn context_prints_to_stdout() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    commit_file(dir.path(), "a.txt", "chore: init");

    let output = git_std()
        .args(["--context"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "should exit 0");
    assert!(
        !output.stdout.is_empty(),
        "context output must be on stdout"
    );
    assert!(
        output.stderr.is_empty(),
        "no stderr on success: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ===========================================================================
// Project section
// ===========================================================================

#[test]
fn context_project_section_shows_scheme() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    commit_file(dir.path(), "a.txt", "chore: init");

    git_std()
        .args(["--context"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(contains("## Project"))
        .stdout(contains("Scheme: semver"));
}

#[test]
fn context_project_section_shows_tag_prefix() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::write(
        dir.path().join(".git-std.toml"),
        "[versioning]\ntag_prefix = \"v\"\n",
    )
    .unwrap();
    commit_file(dir.path(), "a.txt", "chore: init");

    git_std()
        .args(["--context"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(contains("Tag prefix: v"));
}

#[test]
fn context_project_shows_stable_when_on_main_no_prerelease() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    commit_file(dir.path(), "a.txt", "chore: init");
    // Tag with a stable semver — no prerelease
    git(dir.path(), &["tag", "v1.0.0"]);

    let stdout = String::from_utf8_lossy(
        &git_std()
            .args(["--context"])
            .current_dir(dir.path())
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();

    assert!(
        stdout.contains("Stable: true"),
        "should show Stable: true on main with stable tag, got:\n{stdout}"
    );
}

#[test]
fn context_project_shows_not_stable_for_prerelease_tag() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    commit_file(dir.path(), "a.txt", "chore: init");
    git(dir.path(), &["tag", "v1.0.0-rc.1"]);

    let stdout = String::from_utf8_lossy(
        &git_std()
            .args(["--context"])
            .current_dir(dir.path())
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();

    assert!(
        stdout.contains("Stable: false"),
        "should show Stable: false for prerelease tag, got:\n{stdout}"
    );
}

// ===========================================================================
// Workspace section
// ===========================================================================

#[test]
fn context_workspace_section_omitted_for_single_package() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    commit_file(dir.path(), "a.txt", "chore: init");

    let stdout = String::from_utf8_lossy(
        &git_std()
            .args(["--context"])
            .current_dir(dir.path())
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();

    assert!(
        !stdout.contains("## Workspace"),
        "Workspace section should be omitted for single-package repos, got:\n{stdout}"
    );
}

#[test]
fn context_workspace_section_shown_for_monorepo() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    // Simulate a cargo workspace
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/alpha\", \"crates/beta\"]\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("crates/alpha")).unwrap();
    std::fs::write(
        dir.path().join("crates/alpha/Cargo.toml"),
        "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("crates/beta")).unwrap();
    std::fs::write(
        dir.path().join("crates/beta/Cargo.toml"),
        "[package]\nname = \"beta\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(dir.path().join(".git-std.toml"), "monorepo = true\n").unwrap();
    commit_file(dir.path(), "a.txt", "chore: init");

    git_std()
        .args(["--context"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(contains("## Workspace"))
        .stdout(contains("Crates:"))
        .stdout(contains("alpha"))
        .stdout(contains("beta"));
}

// ===========================================================================
// Commit config section
// ===========================================================================

#[test]
fn context_commit_config_section_shows_types() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    commit_file(dir.path(), "a.txt", "chore: init");

    git_std()
        .args(["--context"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(contains("## Commit config"))
        .stdout(contains("Types:"));
}

#[test]
fn context_commit_config_scopes_omitted_when_none() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    // No scopes configured
    commit_file(dir.path(), "a.txt", "chore: init");

    let stdout = String::from_utf8_lossy(
        &git_std()
            .args(["--context"])
            .current_dir(dir.path())
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();

    assert!(
        !stdout.contains("Scopes:"),
        "Scopes line should be omitted when no scopes configured, got:\n{stdout}"
    );
}

#[test]
fn context_commit_config_scopes_explicit_list() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::write(
        dir.path().join(".git-std.toml"),
        "scopes = [\"api\", \"cli\"]\nstrict = true\n",
    )
    .unwrap();
    commit_file(dir.path(), "a.txt", "chore: init");

    git_std()
        .args(["--context"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(contains("Scopes: api, cli (required, strict)"));
}

#[test]
fn context_commit_config_scopes_from_workspace() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::write(
        dir.path().join(".git-std.toml"),
        "scopes = \"auto\"\nstrict = true\n",
    )
    .unwrap();
    commit_file(dir.path(), "a.txt", "chore: init");

    git_std()
        .args(["--context"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(contains("Scopes: from workspace (required, strict)"));
}

// ===========================================================================
// Status states
// ===========================================================================

// State: Not bootstrapped (⚠ warning)
#[test]
fn context_status_not_bootstrapped_when_hooks_not_configured() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    // .githooks/ exists but core.hooksPath is NOT set
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    commit_file(dir.path(), "a.txt", "chore: init");

    git_std()
        .args(["--context"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stdout(contains("Not bootstrapped"));
}

// State: Clean + bootstrapped → "Nothing to commit"
#[test]
fn context_status_clean_when_nothing_to_commit() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    commit_file(dir.path(), "a.txt", "chore: init");
    // No .githooks/ → considered bootstrapped (nothing to configure)

    git_std()
        .args(["--context"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(contains("Nothing to commit"));
}

// State: Nothing staged (unstaged changes only)
#[test]
fn context_status_nothing_staged_with_unstaged_files() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    commit_file(dir.path(), "a.txt", "chore: init");
    // Modify without staging
    std::fs::write(dir.path().join("a.txt"), "changed").unwrap();

    git_std()
        .args(["--context"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(contains("Nothing staged"));
}

// State: Staged only
#[test]
fn context_staged_diff_section_shown_when_staged() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    commit_file(dir.path(), "a.txt", "chore: init");

    // Stage a change
    std::fs::write(dir.path().join("new.txt"), "hello").unwrap();
    git(dir.path(), &["add", "new.txt"]);

    git_std()
        .args(["--context"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(contains("## Staged diff"));
}

// State: Staged + unstaged
#[test]
fn context_both_staged_and_unstaged_shown() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    commit_file(dir.path(), "a.txt", "chore: init");

    // Stage a new file
    std::fs::write(dir.path().join("new.txt"), "hello").unwrap();
    git(dir.path(), &["add", "new.txt"]);
    // Also leave an unstaged change
    std::fs::write(dir.path().join("a.txt"), "changed").unwrap();

    git_std()
        .args(["--context"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(contains("## Staged diff"))
        .stdout(contains("## Unstaged files"));
}

#[test]
fn context_unstaged_files_capped_at_five_with_suffix() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    commit_file(dir.path(), "a.txt", "chore: init");

    // Stage one file, leave 7 unstaged
    std::fs::write(dir.path().join("staged.txt"), "staged").unwrap();
    git(dir.path(), &["add", "staged.txt"]);
    for i in 0..7 {
        std::fs::write(dir.path().join(format!("unstaged{i}.txt")), "x").unwrap();
    }

    let stdout = String::from_utf8_lossy(
        &git_std()
            .args(["--context"])
            .current_dir(dir.path())
            .output()
            .unwrap()
            .stdout,
    )
    .to_string();

    assert!(
        stdout.contains("## Unstaged files"),
        "Unstaged section missing:\n{stdout}"
    );
    assert!(
        stdout.contains("... and"),
        "Should have overflow suffix for >5 unstaged files:\n{stdout}"
    );
}

// ===========================================================================
// --format json
// ===========================================================================

#[test]
fn context_json_outputs_to_stdout() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    commit_file(dir.path(), "a.txt", "chore: init");

    let output = git_std()
        .args(["--context", "--format", "json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert!(parsed.get("project").is_some(), "should have project key");
    assert!(
        parsed.get("commit_config").is_some(),
        "should have commit_config key"
    );
    assert!(
        parsed["status"].is_string(),
        "status field must always be a non-null string"
    );
    assert!(
        output.stderr.is_empty(),
        "stderr should be empty in JSON mode"
    );
}

#[test]
fn context_json_project_has_scheme_and_stable() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    commit_file(dir.path(), "a.txt", "chore: init");

    let output = git_std()
        .args(["--context", "--format", "json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let project = &parsed["project"];
    assert!(
        project["scheme"].is_string(),
        "project.scheme must be a string"
    );
    assert!(
        project["stable"].is_boolean(),
        "project.stable must be a boolean"
    );
}

#[test]
fn context_json_commit_config_has_types() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    commit_file(dir.path(), "a.txt", "chore: init");

    let output = git_std()
        .args(["--context", "--format", "json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let types = &parsed["commit_config"]["types"];
    assert!(types.is_array(), "commit_config.types must be an array");
    assert!(
        !types.as_array().unwrap().is_empty(),
        "commit_config.types must not be empty"
    );
}

// ===========================================================================
// Worktree and subdirectory
// ===========================================================================

#[test]
fn context_from_subdirectory() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    commit_file(dir.path(), "a.txt", "chore: init");

    let subdir = dir.path().join("src");
    std::fs::create_dir_all(&subdir).unwrap();

    git_std()
        .args(["--context"])
        .current_dir(&subdir)
        .assert()
        .success()
        .stdout(contains("## Project"));
}

#[test]
fn context_from_git_worktree() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    commit_file(dir.path(), "a.txt", "chore: init");

    // Create a real git worktree
    let wt_parent = tempfile::tempdir().unwrap();
    let wt_path = wt_parent.path().join("worktree-ctx");
    git(
        dir.path(),
        &[
            "worktree",
            "add",
            wt_path.to_str().unwrap(),
            "-b",
            "wt-branch",
        ],
    );

    git_std()
        .args(["--context"])
        .current_dir(&wt_path)
        .assert()
        .success()
        .stdout(contains("## Project"));
}
