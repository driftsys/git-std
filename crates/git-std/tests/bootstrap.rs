use std::path::Path;

use assert_cmd::Command;

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

fn run_bootstrap(dir: &Path, extra_args: &[&str]) -> assert_cmd::assert::Assert {
    let mut args = vec!["--color", "never", "bootstrap"];
    args.extend_from_slice(extra_args);
    Command::cargo_bin("git-std")
        .unwrap()
        .args(&args)
        .current_dir(dir)
        .assert()
}

fn run_bootstrap_install(dir: &Path, extra_args: &[&str]) -> assert_cmd::assert::Assert {
    let mut args = vec!["--color", "never", "bootstrap", "install"];
    args.extend_from_slice(extra_args);
    Command::cargo_bin("git-std")
        .unwrap()
        .args(&args)
        .current_dir(dir)
        .assert()
}

fn stderr_text(assert: &assert_cmd::assert::Assert) -> String {
    String::from_utf8_lossy(&assert.get_output().stderr).to_string()
}

// ===========================================================================
// #294 — git std bootstrap (run)
// ===========================================================================

#[test]
fn bootstrap_sets_hooks_path_when_githooks_exists() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();

    let a = run_bootstrap(dir.path(), &[]).success();
    let err = stderr_text(&a);
    assert!(
        err.contains("git hooks configured"),
        "should confirm hooks configured, got: {err}"
    );

    let val = git(dir.path(), &["config", "core.hooksPath"]);
    assert_eq!(val, ".githooks");
}

#[test]
fn bootstrap_skips_hooks_path_when_no_githooks() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let a = run_bootstrap(dir.path(), &[]).success();
    let err = stderr_text(&a);
    // No output when nothing to do
    assert!(
        !err.contains("hooks"),
        "should be silent on skip, got: {err}"
    );
}

#[test]
fn bootstrap_skips_lfs_when_no_gitattributes() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let a = run_bootstrap(dir.path(), &[]).success();
    let err = stderr_text(&a);
    // No output when nothing to do
    assert!(!err.contains("LFS"), "should be silent on skip, got: {err}");
}

#[test]
fn bootstrap_skips_lfs_when_no_filter_lfs() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::write(dir.path().join(".gitattributes"), "*.bin binary\n").unwrap();

    let a = run_bootstrap(dir.path(), &[]).success();
    let err = stderr_text(&a);
    assert!(!err.contains("LFS"), "should be silent on skip, got: {err}");
}

#[test]
fn bootstrap_sets_blame_ignore_revs() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::write(dir.path().join(".git-blame-ignore-revs"), "# revs\n").unwrap();

    let a = run_bootstrap(dir.path(), &[]).success();
    let err = stderr_text(&a);
    assert!(
        err.contains("blame ignore revs configured"),
        "should confirm blame config, got: {err}"
    );

    let val = git(dir.path(), &["config", "blame.ignoreRevsFile"]);
    assert_eq!(val, ".git-blame-ignore-revs");
}

#[test]
fn bootstrap_skips_blame_when_no_file() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let a = run_bootstrap(dir.path(), &[]).success();
    let err = stderr_text(&a);
    assert!(
        !err.contains("blame"),
        "should be silent on skip, got: {err}"
    );
}

#[test]
fn bootstrap_runs_custom_hooks() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(hooks_dir.join("bootstrap.hooks"), "! echo bootstrap-ran\n").unwrap();

    let a = run_bootstrap(dir.path(), &[]).success();

    let stdout = String::from_utf8_lossy(&a.get_output().stdout);
    let stderr = stderr_text(&a);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("bootstrap-ran"),
        "custom hook should execute, got: {combined}"
    );
}

#[test]
fn bootstrap_dry_run_no_side_effects() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(dir.path().join(".git-blame-ignore-revs"), "# revs\n").unwrap();

    let a = run_bootstrap(dir.path(), &["--dry-run"]).success();
    let err = stderr_text(&a);
    assert!(
        err.contains("configured"),
        "should show what would be done, got: {err}"
    );

    // Verify no config was actually set
    let output = std::process::Command::new("git")
        .current_dir(dir.path())
        .args(["config", "core.hooksPath"])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "hooksPath should not be set in dry-run"
    );
}

#[test]
fn bootstrap_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(dir.path().join(".git-blame-ignore-revs"), "# revs\n").unwrap();

    // Run twice
    run_bootstrap(dir.path(), &[]).success();
    run_bootstrap(dir.path(), &[]).success();

    // Both should succeed and leave valid config
    let hooks_path = git(dir.path(), &["config", "core.hooksPath"]);
    assert_eq!(hooks_path, ".githooks");
    let blame = git(dir.path(), &["config", "blame.ignoreRevsFile"]);
    assert_eq!(blame, ".git-blame-ignore-revs");
}

#[test]
fn bootstrap_exits_zero_with_nothing_to_do() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    run_bootstrap(dir.path(), &[]).success();
}

// ===========================================================================
// #295 — git std bootstrap install
// ===========================================================================

#[test]
fn bootstrap_install_creates_files() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    run_bootstrap_install(dir.path(), &[]).success();

    // ./bootstrap exists and is executable
    let script = dir.path().join("bootstrap");
    assert!(script.exists(), "bootstrap script should exist");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&script).unwrap().permissions().mode();
        assert!(mode & 0o111 != 0, "bootstrap should be executable");
    }

    // .githooks/bootstrap.hooks exists
    let hooks = dir.path().join(".githooks/bootstrap.hooks");
    assert!(hooks.exists(), "bootstrap.hooks should exist");

    // Created files are staged in the git index
    let staged = git(dir.path(), &["diff", "--cached", "--name-only"]);
    assert!(
        staged.contains("bootstrap"),
        "bootstrap should be staged, got: {staged}"
    );
    assert!(
        staged.contains("bootstrap.hooks"),
        "bootstrap.hooks should be staged, got: {staged}"
    );

    // Executable bit is tracked in the index
    let ls = git(dir.path(), &["ls-files", "-s", "bootstrap"]);
    assert!(
        ls.starts_with("100755"),
        "bootstrap should be 100755 in index, got: {ls}"
    );
}

#[test]
fn bootstrap_install_min_version_matches_crate() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    run_bootstrap_install(dir.path(), &[]).success();

    let script = std::fs::read_to_string(dir.path().join("bootstrap")).unwrap();
    let version = env!("CARGO_PKG_VERSION");
    assert!(
        script.contains(&format!("MIN_VERSION=\"{version}\"")),
        "MIN_VERSION should match crate version"
    );
}

#[test]
fn bootstrap_install_skips_existing_without_force() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    // Create existing files
    std::fs::write(dir.path().join("bootstrap"), "existing\n").unwrap();
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    std::fs::write(dir.path().join(".githooks/bootstrap.hooks"), "existing\n").unwrap();

    let a = run_bootstrap_install(dir.path(), &[]).success();
    let err = stderr_text(&a);
    assert!(
        err.contains("already exists"),
        "should warn about existing files, got: {err}"
    );

    // Verify content unchanged
    let content = std::fs::read_to_string(dir.path().join("bootstrap")).unwrap();
    assert_eq!(content, "existing\n");
}

#[test]
fn bootstrap_install_overwrites_with_force() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    // Create existing files
    std::fs::write(dir.path().join("bootstrap"), "old\n").unwrap();
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    std::fs::write(dir.path().join(".githooks/bootstrap.hooks"), "old\n").unwrap();

    run_bootstrap_install(dir.path(), &["--force"]).success();

    // Verify content was replaced
    let content = std::fs::read_to_string(dir.path().join("bootstrap")).unwrap();
    assert!(content.contains("MIN_VERSION"), "should have new content");
}

#[test]
fn bootstrap_install_appends_marker_to_docs() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    std::fs::write(dir.path().join("README.md"), "# My Project\n").unwrap();
    std::fs::write(dir.path().join("AGENTS.md"), "# Agents\n").unwrap();

    run_bootstrap_install(dir.path(), &[]).success();

    let readme = std::fs::read_to_string(dir.path().join("README.md")).unwrap();
    assert!(
        readme.contains("<!-- git-std:bootstrap -->"),
        "README should have marker"
    );
    assert!(
        readme.contains("./bootstrap"),
        "README should mention ./bootstrap"
    );

    let agents = std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
    assert!(
        agents.contains("<!-- git-std:bootstrap -->"),
        "AGENTS should have marker"
    );
}

#[test]
fn bootstrap_install_does_not_double_append() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    std::fs::write(dir.path().join("README.md"), "# My Project\n").unwrap();

    // Run twice
    run_bootstrap_install(dir.path(), &["--force"]).success();
    run_bootstrap_install(dir.path(), &["--force"]).success();

    let readme = std::fs::read_to_string(dir.path().join("README.md")).unwrap();
    let count = readme.matches("<!-- git-std:bootstrap -->").count();
    assert_eq!(count, 1, "marker should appear exactly once, found {count}");
}

// ===========================================================================
// commit-msg default template (#294 AC)
// ===========================================================================

#[test]
fn hooks_install_generates_commit_msg_with_check_command() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    // Run hooks install with env var to avoid interactive prompt
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hooks", "install"])
        .env("GIT_STD_HOOKS_ENABLE", "commit-msg")
        .current_dir(dir.path())
        .assert()
        .success();

    let hooks_file = dir.path().join(".githooks/commit-msg.hooks");
    assert!(hooks_file.exists(), "commit-msg.hooks should exist");

    let content = std::fs::read_to_string(&hooks_file).unwrap();
    assert!(
        content.contains("! git std check --file {msg}"),
        "commit-msg.hooks should have default check command, got:\n{content}"
    );
}

#[test]
fn hooks_install_commit_msg_shim_is_active() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hooks", "install"])
        .env("GIT_STD_HOOKS_ENABLE", "commit-msg")
        .current_dir(dir.path())
        .assert()
        .success();

    // The active shim (no .off extension) should exist
    let shim = dir.path().join(".githooks/commit-msg");
    assert!(shim.exists(), "commit-msg shim should be active (no .off)");

    // The .off file should NOT exist
    let off = dir.path().join(".githooks/commit-msg.off");
    assert!(
        !off.exists(),
        "commit-msg.off should not exist when enabled"
    );
}

// ── repo-root resolution (#317) ─────────────────────────────────

#[test]
fn bootstrap_run_from_subdirectory() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    std::fs::create_dir_all(dir.path().join(".githooks")).unwrap();
    std::fs::write(dir.path().join(".git-blame-ignore-revs"), "# revs\n").unwrap();

    let subdir = dir.path().join("src").join("nested");
    std::fs::create_dir_all(&subdir).unwrap();

    let a = run_bootstrap(&subdir, &[]).success();
    let err = stderr_text(&a);
    assert!(
        err.contains("git hooks configured"),
        "should configure hooks from subdir, got: {err}"
    );
    assert!(
        err.contains("blame ignore revs configured"),
        "should configure blame from subdir, got: {err}"
    );
}

#[test]
fn bootstrap_install_from_subdirectory() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let subdir = dir.path().join("src");
    std::fs::create_dir_all(&subdir).unwrap();

    run_bootstrap_install(&subdir, &[]).success();

    // Files should be at repo root, not in subdirectory
    assert!(
        dir.path().join("bootstrap").exists(),
        "bootstrap script should be at repo root"
    );
    assert!(
        dir.path().join(".githooks/bootstrap.hooks").exists(),
        "bootstrap.hooks should be at repo root"
    );
    assert!(
        !subdir.join("bootstrap").exists(),
        "bootstrap should not be in subdirectory"
    );
    assert!(
        !subdir.join(".githooks").exists(),
        ".githooks should not be in subdirectory"
    );
}
