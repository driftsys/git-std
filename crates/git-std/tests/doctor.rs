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

/// Fully-configured repo: all doctor health checks satisfied.
/// Doctor exits 0 (no hints) from this baseline.
fn init_full_repo(dir: &std::path::Path) {
    init_repo(dir);
    std::fs::create_dir_all(dir.join(".githooks")).unwrap();
    git(dir, &["config", "core.hooksPath", ".githooks"]);
    std::fs::write(dir.join(".git-std.toml"), "").unwrap();
    std::fs::write(dir.join(".git-blame-ignore-revs"), "").unwrap();
    git(
        dir,
        &["config", "blame.ignoreRevsFile", ".git-blame-ignore-revs"],
    );
}

// ===========================================================================
// Basic smoke tests
// ===========================================================================

#[test]
fn doctor_appears_in_help() {
    git_std()
        .args(["--help"])
        .assert()
        .success()
        .stdout(contains("doctor"));
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

#[test]
fn doctor_exits_0_in_basic_git_repo() {
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());
    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .code(0);
}

// ===========================================================================
// Status section
// ===========================================================================

#[test]
fn doctor_status_section_shows_git_and_git_std() {
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());
    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("Status"))
        .stderr(contains("git "))
        .stderr(contains("git-std "));
}

#[test]
fn doctor_status_skips_lfs_without_filter_lfs() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    // .gitattributes with no filter=lfs
    std::fs::write(dir.path().join(".gitattributes"), "*.png binary\n").unwrap();
    let output = git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("git-lfs"),
        "git-lfs should not appear without filter=lfs in .gitattributes"
    );
}

#[test]
fn doctor_status_shows_lfs_when_gitattributes_has_filter() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::write(dir.path().join(".gitattributes"), "*.bin filter=lfs\n").unwrap();
    let output = git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("git-lfs"),
        "git-lfs should appear when .gitattributes has filter=lfs"
    );
}

// ===========================================================================
// Hooks section
// ===========================================================================

#[test]
fn doctor_hooks_section_hidden_when_no_hooks() {
    // No .githooks/*.hooks files → Hooks section should not appear
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let output = git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("Hooks\n"),
        "Hooks section should not appear when no .hooks files configured"
    );
}

#[test]
fn doctor_hooks_section_shows_configured_hooks() {
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());
    let hooks_dir = dir.path().join(".githooks");
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "! cargo fmt --check\n").unwrap();

    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("Hooks"))
        .stderr(contains("pre-commit"))
        .stderr(contains("cargo fmt --check"));
}

#[test]
fn doctor_hooks_shows_disabled_label() {
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());
    let hooks_dir = dir.path().join(".githooks");
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "! cargo fmt\n").unwrap();
    // No shim file → hook is disabled

    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("(disabled)"));
}

#[test]
fn doctor_hooks_shows_fail_fast_sigil() {
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());
    let hooks_dir = dir.path().join(".githooks");
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "! cargo clippy\n").unwrap();

    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("!  cargo clippy"));
}

#[test]
fn doctor_hooks_shows_advisory_sigil() {
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());
    let hooks_dir = dir.path().join(".githooks");
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "? git lfs install\n").unwrap();

    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("?  git lfs install"));
}

// ===========================================================================
// Configuration section
// ===========================================================================

#[test]
fn doctor_config_section_always_shown() {
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());
    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("Configuration"))
        .stderr(contains("scheme"));
}

#[test]
fn doctor_config_shows_default_values() {
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());
    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("semver"));
}

#[test]
fn doctor_config_shows_file_values() {
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());
    std::fs::write(dir.path().join(".git-std.toml"), "scheme = \"calver\"\n").unwrap();
    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(contains("calver"));
}

#[test]
fn doctor_config_hint_for_invalid_toml() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::write(dir.path().join(".git-std.toml"), "[[invalid toml = bad\n").unwrap();
    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("hint:"))
        .stderr(contains(".git-std.toml invalid"));
}

// ===========================================================================
// Hints section
// ===========================================================================

#[test]
fn doctor_no_hints_when_all_ok() {
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());
    let output = git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("hint:"),
        "No hints when everything is fine"
    );
}

// ===========================================================================
// Hooks hints (ACs 1-3)
// ===========================================================================

#[test]
fn doctor_hint_when_githooks_dir_missing() {
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());
    std::fs::remove_dir(dir.path().join(".githooks")).unwrap();
    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains(".githooks/ not found"));
}

#[test]
fn doctor_hint_when_hooks_path_misconfigured() {
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());
    git(dir.path(), &["config", "core.hooksPath", ".git/hooks"]);
    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("core.hooksPath is '.git/hooks'"));
}

#[test]
fn doctor_hint_when_shim_not_executable() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());
    let shim = dir.path().join(".githooks/pre-commit");
    std::fs::write(&shim, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o644)).unwrap();
    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("pre-commit shim is not executable"));
}

// ===========================================================================
// Config hints (ACs 5-7)
// ===========================================================================

#[test]
fn doctor_hint_when_git_std_toml_absent() {
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());
    std::fs::remove_file(dir.path().join(".git-std.toml")).unwrap();
    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains(".git-std.toml not found"));
}

#[test]
fn doctor_hint_when_blame_ignore_revs_absent() {
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());
    std::fs::remove_file(dir.path().join(".git-blame-ignore-revs")).unwrap();
    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains(".git-blame-ignore-revs not found"));
}

#[test]
fn doctor_hint_when_blame_ignore_revs_not_configured() {
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());
    git(dir.path(), &["config", "--unset", "blame.ignoreRevsFile"]);
    git_std()
        .args(["doctor"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(contains("blame.ignoreRevsFile not set"));
}

// ===========================================================================
// --format json
// ===========================================================================

#[test]
fn doctor_json_outputs_to_stdout() {
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());

    let output = git_std()
        .args(["doctor", "--format", "json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert!(parsed.get("status").is_some(), "should have status");
    assert!(parsed.get("sections").is_some(), "should have sections");
    let sections = parsed["sections"].as_object().unwrap();
    assert!(sections.contains_key("status"), "sections.status");
    assert!(sections.contains_key("hooks"), "sections.hooks");
    assert!(
        sections.contains_key("configuration"),
        "sections.configuration"
    );
    assert!(
        output.stderr.is_empty(),
        "stderr should be empty in JSON mode"
    );
}

#[test]
fn doctor_json_has_pass_status_when_no_problems() {
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());

    let output = git_std()
        .args(["doctor", "--format", "json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "pass");
    // hints must be an empty array (not absent, not null) when clean
    let hints = parsed["hints"].as_array().expect("hints must be an array");
    assert!(hints.is_empty(), "hints must be empty when no problems");
    assert!(
        output.stderr.is_empty(),
        "stderr should be empty in JSON mode"
    );
}

#[test]
fn doctor_json_status_tools_have_version_string() {
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());

    let output = git_std()
        .args(["doctor", "--format", "json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let tools = parsed["sections"]["status"].as_array().unwrap();
    for tool in tools {
        assert!(
            tool["name"].is_string(),
            "every status tool must have a name string"
        );
        // version is present and is a string for tools that are found
        assert!(
            tool["version"].is_string(),
            "tool '{}' must have a version string",
            tool["name"]
        );
    }
}

#[test]
fn doctor_json_has_fail_status_when_invalid_toml() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::write(dir.path().join(".git-std.toml"), "[[invalid\n").unwrap();

    let output = git_std()
        .args(["doctor", "--format", "json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "fail");
    let hints = parsed["hints"].as_array().unwrap();
    assert!(!hints.is_empty(), "should have hints for invalid TOML");
    assert!(
        output.stderr.is_empty(),
        "stderr should be empty in JSON mode"
    );
}

#[test]
fn doctor_json_status_section_contains_git_std() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let output = git_std()
        .args(["doctor", "--format", "json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let status_tools = parsed["sections"]["status"].as_array().unwrap();
    let git_std_entry = status_tools
        .iter()
        .find(|t| t["name"] == "git-std")
        .expect("git-std should be in status");
    assert!(
        git_std_entry["version"].is_string(),
        "git-std should have version"
    );
}

#[test]
fn doctor_json_configuration_section_has_scheme() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let output = git_std()
        .args(["doctor", "--format", "json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let config_rows = parsed["sections"]["configuration"].as_array().unwrap();
    let scheme_row = config_rows
        .iter()
        .find(|r| r["key"] == "scheme")
        .expect("scheme should be in configuration");
    assert_eq!(scheme_row["value"], "semver");
    assert_eq!(scheme_row["source"], "default");
}

#[test]
fn doctor_json_configuration_source_file_when_explicit() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::write(dir.path().join(".git-std.toml"), "scheme = \"calver\"\n").unwrap();

    let output = git_std()
        .args(["doctor", "--format", "json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let config_rows = parsed["sections"]["configuration"].as_array().unwrap();
    let scheme_row = config_rows
        .iter()
        .find(|r| r["key"] == "scheme")
        .expect("scheme should be in configuration");
    assert_eq!(scheme_row["value"], "calver");
    assert_eq!(scheme_row["source"], "file");
}

// ===========================================================================
// Worktree and subdirectory
// ===========================================================================

#[test]
fn doctor_from_subdirectory() {
    let dir = tempfile::tempdir().unwrap();
    init_full_repo(dir.path());
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
        .stderr(contains("Status"))
        .stderr(contains("Configuration"));
}

#[test]
fn doctor_from_git_worktree() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    // Set up a valid repo with hooks, config, and commit them.
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    std::fs::write(dir.path().join(".githooks/.gitkeep"), "").unwrap();
    std::fs::write(
        dir.path().join(".git-std.toml"),
        "[versioning]\ntag_prefix = \"v\"\n",
    )
    .unwrap();
    std::fs::write(dir.path().join(".git-blame-ignore-revs"), "").unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-m", "initial commit"]);

    git(dir.path(), &["config", "core.hooksPath", ".githooks"]);
    git(
        dir.path(),
        &["config", "blame.ignoreRevsFile", ".git-blame-ignore-revs"],
    );

    // Create a real git worktree.
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

    // Run doctor from the worktree — should find repo-level config.
    git_std()
        .args(["doctor"])
        .current_dir(&wt_path)
        .assert()
        .success()
        .stderr(contains("Status"))
        .stderr(contains("Configuration"));
}
