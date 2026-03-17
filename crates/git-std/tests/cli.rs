use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn version_flag_prints_version() {
    Command::cargo_bin("git-std")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_lists_subcommands() {
    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .arg("--help")
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    for sub in [
        "commit",
        "check",
        "bump",
        "changelog",
        "hooks",
        "completions",
    ] {
        assert!(
            stdout.contains(sub),
            "help output should list '{sub}' subcommand"
        );
    }
}

#[test]
fn unknown_subcommand_exits_2() {
    Command::cargo_bin("git-std")
        .unwrap()
        .arg("does-not-exist")
        .assert()
        .code(2);
}


#[test]
fn hooks_requires_subcommand() {
    Command::cargo_bin("git-std")
        .unwrap()
        .arg("hooks")
        .assert()
        .code(2);
}

#[test]
fn commit_dry_run_prints_message() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["commit", "--type", "feat", "-m", "add login", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("feat: add login"));
}

#[test]
fn commit_dry_run_with_scope() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "commit",
            "--type",
            "fix",
            "--scope",
            "auth",
            "-m",
            "handle tokens",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("fix(auth): handle tokens"));
}

#[test]
fn commit_dry_run_with_breaking() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "commit",
            "--type",
            "feat",
            "-m",
            "remove legacy API",
            "--breaking",
            "removed v1 endpoints",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("feat!: remove legacy API"))
        .stdout(predicate::str::contains(
            "BREAKING CHANGE: removed v1 endpoints",
        ));
}

#[test]
fn commit_help_shows_flags() {
    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["commit", "--help"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    for flag in [
        "--type",
        "--scope",
        "--message",
        "--breaking",
        "--dry-run",
        "--amend",
        "--sign",
        "--all",
    ] {
        assert!(
            stdout.contains(flag),
            "commit help should list '{flag}' flag"
        );
    }
}

#[test]
fn commit_short_flags() {
    // -m alias for --message
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["commit", "--type", "feat", "-m", "short flag", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("feat: short flag"));
}

// --- Commit integration tests (actual repo) ---

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

/// Helper: get HEAD commit message.
fn head_message(dir: &Path) -> String {
    git(dir, &["log", "-1", "--format=%B"]).trim().to_string()
}

/// Helper: check if a tag exists.
fn tag_exists(dir: &Path, tag: &str) -> bool {
    std::process::Command::new("git")
        .current_dir(dir)
        .args(["rev-parse", "--verify", &format!("refs/tags/{tag}")])
        .output()
        .unwrap()
        .status
        .success()
}

/// Helper: check if a branch exists.
fn branch_exists(dir: &Path, branch: &str) -> bool {
    std::process::Command::new("git")
        .current_dir(dir)
        .args(["rev-parse", "--verify", &format!("refs/heads/{branch}")])
        .output()
        .unwrap()
        .status
        .success()
}

/// Helper: count commits in repo.
fn commit_count(dir: &Path) -> usize {
    let output = git(dir, &["rev-list", "--count", "HEAD"]);
    output.parse().unwrap()
}

/// Helper: collect all tag names.
fn collect_tag_names(dir: &Path) -> Vec<String> {
    let output = git(dir, &["tag", "-l"]);
    if output.is_empty() {
        vec![]
    } else {
        output.lines().map(|s| s.to_string()).collect()
    }
}

/// Helper: initialise a git repo with one committed file (for commit tests).
fn init_commit_repo(dir: &Path) {
    git(dir, &["init"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@test.com"]);

    std::fs::write(dir.join("hello.txt"), "hello").unwrap();
    git(dir, &["add", "hello.txt"]);
    git(dir, &["commit", "-m", "chore: init"]);
}

#[test]
fn commit_actual_execution() {
    let dir = tempfile::tempdir().unwrap();
    init_commit_repo(dir.path());

    // Stage a new file so the commit has content.
    std::fs::write(dir.path().join("feature.txt"), "feature").unwrap();
    git(dir.path(), &["add", "feature.txt"]);

    Command::cargo_bin("git-std")
        .unwrap()
        .current_dir(dir.path())
        .args(["commit", "--type", "feat", "-m", "add feature"])
        .assert()
        .success();

    assert_eq!(head_message(dir.path()), "feat: add feature");
}

#[test]
fn commit_with_scope() {
    let dir = tempfile::tempdir().unwrap();
    init_commit_repo(dir.path());

    std::fs::write(dir.path().join("login.txt"), "login").unwrap();
    git(dir.path(), &["add", "login.txt"]);

    Command::cargo_bin("git-std")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "commit",
            "--type",
            "feat",
            "--scope",
            "auth",
            "-m",
            "add login",
        ])
        .assert()
        .success();

    assert_eq!(head_message(dir.path()), "feat(auth): add login");
}

#[test]
fn commit_with_breaking() {
    let dir = tempfile::tempdir().unwrap();
    init_commit_repo(dir.path());

    std::fs::write(dir.path().join("api.txt"), "new api").unwrap();
    git(dir.path(), &["add", "api.txt"]);

    Command::cargo_bin("git-std")
        .unwrap()
        .current_dir(dir.path())
        .args([
            "commit",
            "--type",
            "feat",
            "--breaking",
            "remove old API",
            "-m",
            "new auth",
        ])
        .assert()
        .success();

    let msg = git(dir.path(), &["log", "-1", "--format=%B"]);
    assert!(msg.starts_with("feat!: new auth"), "got: {msg}");
    assert!(
        msg.contains("BREAKING CHANGE: remove old API"),
        "got: {msg}"
    );
}

#[test]
fn commit_amend() {
    let dir = tempfile::tempdir().unwrap();
    init_commit_repo(dir.path());

    // Create a commit to amend.
    std::fs::write(dir.path().join("bug.txt"), "bug").unwrap();
    git(dir.path(), &["add", "bug.txt"]);

    Command::cargo_bin("git-std")
        .unwrap()
        .current_dir(dir.path())
        .args(["commit", "--type", "fix", "-m", "original message"])
        .assert()
        .success();

    // Now amend it.
    Command::cargo_bin("git-std")
        .unwrap()
        .current_dir(dir.path())
        .args(["commit", "--amend", "--type", "fix", "-m", "corrected"])
        .assert()
        .success();

    assert_eq!(head_message(dir.path()), "fix: corrected");

    // Verify amend didn't create an extra commit — should be 2 total (init + amended).
    assert_eq!(
        commit_count(dir.path()),
        2,
        "amend should not create a new commit"
    );
}

#[test]
fn commit_all_flag() {
    let dir = tempfile::tempdir().unwrap();
    init_commit_repo(dir.path());

    // Modify the tracked file without staging.
    std::fs::write(dir.path().join("hello.txt"), "modified").unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .current_dir(dir.path())
        .args(["commit", "--all", "--type", "fix", "-m", "fix"])
        .assert()
        .success();

    assert_eq!(head_message(dir.path()), "fix: fix");

    // Verify the modified content was committed.
    let content = git(dir.path(), &["show", "HEAD:hello.txt"]);
    assert_eq!(content, "modified");
}

#[test]
fn commit_combined_flags() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "commit",
            "--type",
            "feat",
            "--scope",
            "auth",
            "-m",
            "add login",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("feat(auth): add login"));
}

// --- Bump integration tests ---

/// Helper: initialise a git repo with a Cargo.toml and one commit.
fn init_bump_repo(dir: &Path) {
    git(dir, &["init"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@test.com"]);

    // Write a minimal Cargo.toml.
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"test-pkg\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    git(dir, &["add", "Cargo.toml"]);
    git(dir, &["commit", "-m", "chore: init"]);
}

/// Helper: add a commit to a repo.
fn add_commit(dir: &Path, filename: &str, message: &str) {
    std::fs::write(dir.join(filename), message).unwrap();
    git(dir, &["add", filename]);
    git(dir, &["commit", "-m", message]);
}

/// Helper: create an annotated tag.
fn create_tag(dir: &Path, name: &str) {
    git(dir, &["tag", "-a", name, "-m", name]);
}

#[test]
fn bump_help_shows_flags() {
    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--help"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    for flag in [
        "--dry-run",
        "--prerelease",
        "--release-as",
        "--first-release",
        "--no-tag",
        "--no-commit",
        "--skip-changelog",
        "--sign",
        "--force",
        "--stable",
        "--minor",
    ] {
        assert!(stdout.contains(flag), "bump help should list '{flag}' flag");
    }
}

#[test]
fn bump_dry_run_shows_plan() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());

    // Tag v0.1.0 on the init commit.
    create_tag(dir.path(), "v0.1.0");

    // Add a feat commit.
    add_commit(dir.path(), "a.txt", "feat: add feature A");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("0.1.0 → 0.2.0"))
        .stderr(predicate::str::contains("Would commit"))
        .stderr(predicate::str::contains("Would tag"));
}

#[test]
fn bump_dry_run_no_writes() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v0.1.0");
    add_commit(dir.path(), "a.txt", "feat: add feature A");

    // Read Cargo.toml before.
    let before = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Cargo.toml should be unchanged.
    let after = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert_eq!(before, after);

    // No CHANGELOG.md should be created.
    assert!(!dir.path().join("CHANGELOG.md").exists());
}

#[test]
fn bump_performs_full_workflow() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Add a fix commit.
    add_commit(dir.path(), "b.txt", "fix: handle edge case");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.0.0 → 1.0.1"))
        .stderr(predicate::str::contains("Committed"))
        .stderr(predicate::str::contains("Tagged"));

    // Verify Cargo.toml was updated.
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"1.0.1\""));

    // Verify CHANGELOG.md was created.
    assert!(dir.path().join("CHANGELOG.md").exists());

    // Verify the tag exists.
    assert!(tag_exists(dir.path(), "v1.0.1"), "tag v1.0.1 should exist");

    // Verify commit message.
    assert_eq!(head_message(dir.path()), "chore(release): 1.0.1");
}

#[test]
fn bump_no_bump_worthy_commits() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Add a non-bump commit.
    add_commit(dir.path(), "c.txt", "chore: update deps");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("no bump-worthy commits"));
}

#[test]
fn bump_major_on_breaking_change() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.2.3");

    add_commit(dir.path(), "d.txt", "feat!: remove old API");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.2.3 → 2.0.0"));
}

#[test]
fn bump_no_commit_flag() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "e.txt", "feat: new thing");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--no-commit"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Cargo.toml should be updated.
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"1.1.0\""));

    // No release commit should exist.
    assert_ne!(head_message(dir.path()), "chore(release): 1.1.0");

    // No tag should exist.
    assert!(!tag_exists(dir.path(), "v1.1.0"));
}

#[test]
fn bump_skip_changelog() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "f.txt", "fix: a fix");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .success();

    // No CHANGELOG.md should be created.
    assert!(!dir.path().join("CHANGELOG.md").exists());
}

#[test]
fn bump_release_as() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "g.txt", "fix: a fix");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--release-as", "5.0.0"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.0.0 → 5.0.0"));

    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"5.0.0\""));
}

#[test]
fn bump_first_release() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    // No tags — first release. Add a feat commit so the changelog has content.
    add_commit(dir.path(), "init.txt", "feat: initial feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--first-release"])
        .current_dir(dir.path())
        .assert()
        .success();

    // CHANGELOG.md should be created.
    assert!(dir.path().join("CHANGELOG.md").exists());

    // Version should stay at 0.0.0 (first-release doesn't bump).
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"0.0.0\""));
}

/// Assert that a version string matches calver format: YYYY.MM.PATCH
/// where YYYY is a 4-digit year, MM is a 1-2 digit month (1-12),
/// and PATCH is a non-negative integer.
fn assert_calver_format(ver: &str) {
    let parts: Vec<&str> = ver.split('.').collect();
    assert_eq!(
        parts.len(),
        3,
        "calver should have 3 dot-separated parts, got: {ver}"
    );

    // Year: exactly 4 digits, >= 2020
    let year: u32 = parts[0]
        .parse()
        .unwrap_or_else(|_| panic!("year should be numeric, got: {}", parts[0]));
    assert!(
        (2020..=2100).contains(&year),
        "year should be a plausible 4-digit year, got: {year}"
    );

    // Month: 1-2 digits, 1-12
    let month: u32 = parts[1]
        .parse()
        .unwrap_or_else(|_| panic!("month should be numeric, got: {}", parts[1]));
    assert!(
        (1..=12).contains(&month),
        "month should be 1-12, got: {month}"
    );

    // Patch: non-negative integer
    let _patch: u32 = parts[2]
        .parse()
        .unwrap_or_else(|_| panic!("patch should be numeric, got: {}", parts[2]));
}

/// Full calver release cycle: first release, then a second bump in the same month.
#[test]
fn bump_full_release_cycle() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());

    // Write a .git-std.toml with calver scheme.
    std::fs::write(dir.path().join(".git-std.toml"), "scheme = \"calver\"\n").unwrap();

    // Add a feat commit.
    add_commit(dir.path(), "a.txt", "feat: first feature");

    // First release.
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("calver"))
        .stderr(predicate::str::contains("Committed"))
        .stderr(predicate::str::contains("Tagged"));

    // Verify a tag was created.
    let tags = collect_tag_names(dir.path());
    assert!(!tags.is_empty(), "at least one tag should exist after bump");

    // Verify the first tag matches calver format: vYYYY.MM.PATCH
    let tag_name = &tags[0];
    assert!(
        tag_name.starts_with('v'),
        "tag should start with 'v', got: {tag_name}"
    );
    let ver = tag_name.strip_prefix('v').unwrap();
    assert_calver_format(ver);

    // The first release should have patch == 0.
    let parts: Vec<&str> = ver.split('.').collect();
    assert_eq!(
        parts[2], "0",
        "first calver release should have patch 0, got: {}",
        parts[2]
    );

    // Second bump: add another commit, should increment patch.
    add_commit(dir.path(), "b.txt", "fix: a bugfix");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("calver"));

    // Should now have two tags.
    let tags2 = collect_tag_names(dir.path());
    assert!(
        tags2.len() >= 2,
        "should have at least 2 tags after second bump, got: {}",
        tags2.len()
    );

    // Verify both tags match calver format.
    for t in &tags2 {
        let v = t.strip_prefix('v').unwrap_or(t);
        assert_calver_format(v);
    }

    // The second tag should share the same YYYY.MM prefix and have patch == 1.
    let first_ver = tags2[0].strip_prefix('v').unwrap();
    let second_ver = tags2[1].strip_prefix('v').unwrap();
    let first_parts: Vec<&str> = first_ver.split('.').collect();
    let second_parts: Vec<&str> = second_ver.split('.').collect();
    assert_eq!(
        first_parts[0], second_parts[0],
        "year should match between bumps: {} vs {}",
        first_parts[0], second_parts[0]
    );
    assert_eq!(
        first_parts[1], second_parts[1],
        "month should match between bumps: {} vs {}",
        first_parts[1], second_parts[1]
    );
    let patch: u32 = second_parts[2].parse().expect("patch should be numeric");
    assert_eq!(
        patch, 1,
        "second calver bump should have patch 1, got: {patch}"
    );
}

/// Pre-release cycle: first bump with --prerelease creates -rc.0 style tag,
/// second bump increments it.
#[test]
fn bump_prerelease_cycle() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Add a feat commit.
    add_commit(dir.path(), "a.txt", "feat: new feature");

    // First pre-release bump.
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--prerelease"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.0.0 → 1.1.0-rc.0"));

    // Verify tag.
    assert!(
        tag_exists(dir.path(), "v1.1.0-rc.0"),
        "tag v1.1.0-rc.0 should exist"
    );

    // Second pre-release bump.
    add_commit(dir.path(), "b.txt", "fix: a fix");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--prerelease"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.1.0-rc.0 → 1.1.0-rc.1"));

    // Verify second tag.
    assert!(
        tag_exists(dir.path(), "v1.1.0-rc.1"),
        "tag v1.1.0-rc.1 should exist"
    );
}

// --- Hooks install integration tests ---

/// Helper: initialise a git repo for hooks tests.
fn init_hooks_repo(dir: &Path) {
    git(dir, &["init"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@test.com"]);
}

#[test]
fn hooks_install_creates_shims() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    // Create .githooks/ with a hooks file.
    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(
        hooks_dir.join("pre-commit.hooks"),
        "dprint check\ncargo test\n",
    )
    .unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["hooks", "install"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("core.hooksPath"))
        .stderr(predicate::str::contains(".githooks/pre-commit"));

    // Verify shim exists.
    let shim_path = hooks_dir.join("pre-commit");
    assert!(shim_path.exists(), "shim should exist");

    // Verify shim content.
    let content = std::fs::read_to_string(&shim_path).unwrap();
    assert_eq!(
        content,
        "#!/bin/bash\nexec git std hooks run pre-commit -- \"$@\"\n"
    );

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
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "dprint check\n").unwrap();
    std::fs::write(hooks_dir.join("pre-push.hooks"), "!cargo test\n").unwrap();
    std::fs::write(
        hooks_dir.join("commit-msg.hooks"),
        "!git std check --file {msg}\n",
    )
    .unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["hooks", "install"])
        .current_dir(dir.path())
        .assert()
        .success();

    // All three shims should exist.
    assert!(hooks_dir.join("pre-commit").exists());
    assert!(hooks_dir.join("pre-push").exists());
    assert!(hooks_dir.join("commit-msg").exists());
}

#[test]
fn hooks_install_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "cargo test\n").unwrap();

    // Run install twice.
    for _ in 0..2 {
        Command::cargo_bin("git-std")
            .unwrap()
            .args(["hooks", "install"])
            .current_dir(dir.path())
            .assert()
            .success();
    }

    let content = std::fs::read_to_string(hooks_dir.join("pre-commit")).unwrap();
    assert_eq!(
        content,
        "#!/bin/bash\nexec git std hooks run pre-commit -- \"$@\"\n"
    );
}

#[test]
fn hooks_install_preserves_non_hooks_files() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "cargo test\n").unwrap();
    std::fs::write(hooks_dir.join("custom-script.sh"), "#!/bin/bash\necho hi\n").unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["hooks", "install"])
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

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(
        hooks_dir.join("pre-commit.hooks"),
        "dprint check\ncargo clippy --workspace -- -D warnings *.rs\n? detekt --input modules/ *.kt\n",
    )
    .unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["hooks", "list"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("pre-commit (collect mode):"),
        "should show hook name and mode, got: {stdout}"
    );
    assert!(stdout.contains("dprint check"), "should list commands");
    assert!(stdout.contains("*.rs"), "should show glob pattern");
    assert!(stdout.contains("?"), "should show advisory prefix");
}

#[test]
fn hooks_list_fail_fast_mode() {
    let dir = tempfile::tempdir().unwrap();

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(
        hooks_dir.join("pre-push.hooks"),
        "!cargo build --workspace\n!cargo test --workspace\n",
    )
    .unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["hooks", "list"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("pre-push (fail-fast mode):"),
        "should show fail-fast mode"
    );
    assert!(
        stdout.contains("! cargo build --workspace"),
        "should show fail-fast prefix"
    );
}

#[test]
fn hooks_list_commit_msg() {
    let dir = tempfile::tempdir().unwrap();

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(
        hooks_dir.join("commit-msg.hooks"),
        "! git std check --file {msg}\n",
    )
    .unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["hooks", "list"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("commit-msg (fail-fast mode):"),
        "should show commit-msg with fail-fast mode"
    );
    assert!(
        stdout.contains("git std check --file {msg}"),
        "should show command with {{msg}} token"
    );
}

#[test]
fn hooks_list_no_hooks() {
    let dir = tempfile::tempdir().unwrap();

    // No .githooks/ directory at all.
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["hooks", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("no hooks configured"));
}

#[test]
fn hooks_list_multiple_hooks() {
    let dir = tempfile::tempdir().unwrap();

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "dprint check\n").unwrap();
    std::fs::write(hooks_dir.join("pre-push.hooks"), "!cargo test\n").unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["hooks", "list"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("pre-commit") && stdout.contains("pre-push"),
        "should list all hooks"
    );
}

// --- Hooks run integration tests (#32–#35) ---

/// #32 — Argument passthrough: `{msg}` token gets substituted with the path
/// passed after `--`.
#[test]
fn hooks_run_arg_passthrough_substitutes_msg_token() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // Use `echo {msg}` so the substituted path appears in output.
    std::fs::write(hooks_dir.join("commit-msg.hooks"), "! echo {msg}\n").unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "--color",
            "never",
            "hooks",
            "run",
            "commit-msg",
            "--",
            "/tmp/test-msg",
        ])
        .current_dir(dir.path())
        .assert()
        .success();

    // The substituted path should appear in stdout (from echo) or stderr
    // (from the hook runner summary line).
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("/tmp/test-msg"),
        "output should contain the substituted path, got:\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// #33 — Pre-commit workflow: mix of passing, failing, and advisory commands.
#[test]
fn hooks_run_pre_commit_workflow() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // The advisory command (`? false`) fails but is advisory, so it gets ⚠.
    std::fs::write(
        hooks_dir.join("pre-commit.hooks"),
        "echo \"lint ok\"\nfalse\n? false\n",
    )
    .unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hooks", "run", "pre-commit"])
        .current_dir(dir.path())
        .assert()
        .code(1);

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    // Check mark for the passing `echo "lint ok"` command.
    assert!(
        stderr.contains('\u{2713}'),
        "should contain check mark for passing command, got: {stderr}"
    );
    // Cross mark for the failing `false` command.
    assert!(
        stderr.contains('\u{2717}'),
        "should contain cross mark for failing command, got: {stderr}"
    );
    // Warning mark for the advisory `echo "advisory warning"` command.
    assert!(
        stderr.contains('\u{26a0}'),
        "should contain warning mark for advisory command, got: {stderr}"
    );
}

/// #34 — Commit-msg workflow: bad message fails validation.
#[test]
fn hooks_run_commit_msg_bad_message_fails() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    // Use the cargo-built binary path so the hook command works in CI
    // where `git std` isn't on PATH.
    let bin = Command::cargo_bin("git-std")
        .unwrap()
        .get_program()
        .to_owned();
    let bin_str = bin.to_string_lossy();

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(
        hooks_dir.join("commit-msg.hooks"),
        format!("! {bin_str} check --file {{msg}}\n"),
    )
    .unwrap();

    // Write a bad commit message to a temp file.
    let msg_file = dir.path().join("COMMIT_MSG");
    std::fs::write(&msg_file, "bad message\n").unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "--color",
            "never",
            "hooks",
            "run",
            "commit-msg",
            "--",
            msg_file.to_str().unwrap(),
        ])
        .current_dir(dir.path())
        .assert()
        .code(1);
}

/// #34 — Commit-msg workflow: good message passes validation.
#[test]
fn hooks_run_commit_msg_good_message_passes() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    // Use the cargo-built binary path so the hook command works in CI.
    let bin = Command::cargo_bin("git-std")
        .unwrap()
        .get_program()
        .to_owned();
    let bin_str = bin.to_string_lossy();

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(
        hooks_dir.join("commit-msg.hooks"),
        format!("! {bin_str} check --file {{msg}}\n"),
    )
    .unwrap();

    // Write a valid conventional commit message to a temp file.
    let msg_file = dir.path().join("COMMIT_MSG");
    std::fs::write(&msg_file, "feat: valid commit\n").unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "--color",
            "never",
            "hooks",
            "run",
            "commit-msg",
            "--",
            msg_file.to_str().unwrap(),
        ])
        .current_dir(dir.path())
        .assert()
        .success();
}

/// #35 — Full install cycle: install hooks, then commit through git which
/// triggers the shims.
#[test]
fn hooks_full_install_cycle() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["config", "user.email", "test@test.com"]);

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(
        hooks_dir.join("pre-commit.hooks"),
        "echo \"pre-commit ok\"\n",
    )
    .unwrap();
    // Use the cargo-built binary path for the commit-msg hook so it works
    // in CI where `git std` isn't on PATH.
    let bin = Command::cargo_bin("git-std")
        .unwrap()
        .get_program()
        .to_owned();
    let bin_str = bin.to_string_lossy();
    std::fs::write(
        hooks_dir.join("commit-msg.hooks"),
        format!("! {bin_str} check --file {{msg}}\n"),
    )
    .unwrap();

    // Run `git std hooks install`.
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["hooks", "install"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Verify core.hooksPath is set.
    let hooks_path = git(dir.path(), &["config", "core.hooksPath"]);
    assert_eq!(hooks_path, ".githooks");

    // Verify shims exist and are executable.
    let pre_commit_shim = hooks_dir.join("pre-commit");
    let commit_msg_shim = hooks_dir.join("commit-msg");
    assert!(pre_commit_shim.exists(), "pre-commit shim should exist");
    assert!(commit_msg_shim.exists(), "commit-msg shim should exist");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::metadata(&pre_commit_shim).unwrap().permissions();
        assert!(
            perms.mode() & 0o111 != 0,
            "pre-commit shim should be executable"
        );
        let perms = std::fs::metadata(&commit_msg_shim).unwrap().permissions();
        assert!(
            perms.mode() & 0o111 != 0,
            "commit-msg shim should be executable"
        );
    }

    // The shims call `git std hooks run ...` which invokes `git-std` as a
    // git subcommand. For this to work, the `git-std` binary must be on
    // PATH. Locate the cargo-built binary and prepend its directory.
    let bin_path = Command::cargo_bin("git-std")
        .unwrap()
        .get_program()
        .to_owned();
    let bin_dir = Path::new(&bin_path).parent().unwrap();
    let path_env = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    // Stage a file and commit with a valid conventional message.
    // The hooks fire (pre-commit + commit-msg) via the installed shims.
    std::fs::write(dir.path().join("hello.txt"), "hello\n").unwrap();

    let status = std::process::Command::new("git")
        .args(["add", "hello.txt"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(status.success(), "git add should succeed");

    let output = std::process::Command::new("git")
        .args(["commit", "-m", "feat: add hello"])
        .current_dir(dir.path())
        .env("PATH", &path_env)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git commit with valid message should succeed when hooks are installed.\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    // Verify the commit was created.
    let msg = head_message(dir.path());
    assert!(
        msg.starts_with("feat: add hello"),
        "commit message should start with 'feat: add hello', got: {msg:?}",
    );
}

// --- Fail-fast mode integration test (#114) ---

/// #114 — Fail-fast mode stops on first failure and skips remaining commands.
///
/// Uses `pre-push` which defaults to fail-fast mode. The first command
/// succeeds, the second fails, and the third should be skipped.
#[test]
fn hooks_run_fail_fast_skips_remaining_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // pre-push defaults to fail-fast mode:
    //   1. `true`  — succeeds
    //   2. `false` — fails  (should trigger abort)
    //   3. `echo should-not-run` — should be skipped
    std::fs::write(
        hooks_dir.join("pre-push.hooks"),
        "true\nfalse\necho should-not-run\n",
    )
    .unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hooks", "run", "pre-push"])
        .current_dir(dir.path())
        .assert()
        .code(1);

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let combined = format!("{stdout}{stderr}");

    // The first command should pass.
    assert!(
        combined.contains('\u{2713}'),
        "should contain check mark for passing command, got: {combined}"
    );
    // The second command should fail.
    assert!(
        combined.contains('\u{2717}'),
        "should contain cross mark for failing command, got: {combined}"
    );
    // The runner should report that remaining commands were skipped.
    assert!(
        combined.contains("skipped (fail-fast)"),
        "should report skipped commands, got: {combined}"
    );
    // The skipped command should NOT have run.
    assert!(
        !combined.contains("should-not-run"),
        "skipped command output should not appear, got: {combined}"
    );
}

/// #114 — Fail-fast with explicit `!` prefix on a collect-mode hook.
///
/// Uses `pre-commit` (collect mode by default) but the failing command
/// has a `!` prefix, forcing fail-fast for that command.
#[test]
fn hooks_run_fail_fast_prefix_overrides_collect_mode() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // pre-commit defaults to collect mode, but `!false` forces fail-fast
    // for that specific command:
    //   1. `true`  — succeeds
    //   2. `!false` — fails with fail-fast prefix (should abort)
    //   3. `echo should-not-run` — should be skipped
    std::fs::write(
        hooks_dir.join("pre-commit.hooks"),
        "true\n!false\necho should-not-run\n",
    )
    .unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hooks", "run", "pre-commit"])
        .current_dir(dir.path())
        .assert()
        .code(1);

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let combined = format!("{stdout}{stderr}");

    // Should report skipped commands due to fail-fast.
    assert!(
        combined.contains("skipped (fail-fast)"),
        "should report skipped commands when ! prefix triggers fail-fast, got: {combined}"
    );
    // The skipped command should NOT have run.
    assert!(
        !combined.contains("should-not-run"),
        "skipped command output should not appear, got: {combined}"
    );
}

// --- Changelog --range integration tests ---

#[test]
fn changelog_range_valid_between_two_tags() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    add_commit(dir.path(), "a.txt", "feat: add feature A");
    add_commit(dir.path(), "b.txt", "fix: fix bug B");
    create_tag(dir.path(), "v1.1.0");

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["changelog", "--range", "v1.0.0..v1.1.0", "--stdout"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("add feature A"),
        "should contain the feat commit, got: {stdout}"
    );
    assert!(
        stdout.contains("fix bug B"),
        "should contain the fix commit, got: {stdout}"
    );
    assert!(
        stdout.contains("1.1.0"),
        "should use the 'to' tag as version label, got: {stdout}"
    );
}

#[test]
fn changelog_range_invalid_missing_dotdot() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["changelog", "--range", "v1.0.0", "--stdout"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains("range must contain '..'"));
}

#[test]
fn changelog_range_invalid_ref() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());

    Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "changelog",
            "--range",
            "nonexistent..also-nonexistent",
            "--stdout",
        ])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains("cannot resolve"));
}

#[test]
fn changelog_range_no_conventional_commits() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Add only non-conventional commits.
    add_commit(dir.path(), "a.txt", "random message with no type");
    create_tag(dir.path(), "v1.0.1");

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["changelog", "--range", "v1.0.0..v1.0.1", "--stdout"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("no conventional commits found"),
        "should report no conventional commits, got stderr: {stderr}"
    );
}

// --- Patch scheme integration tests (#138) ---

#[test]
fn bump_patch_scheme_produces_patch_from_feat() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Write a .git-std.toml with patch scheme.
    std::fs::write(dir.path().join(".git-std.toml"), "scheme = \"patch\"\n").unwrap();

    // Add a feat commit — should still produce a patch bump.
    add_commit(dir.path(), "a.txt", "feat: add feature A");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.0.0 \u{2192} 1.0.1"))
        .stderr(predicate::str::contains("patch"));

    // Verify tag was created.
    assert!(tag_exists(dir.path(), "v1.0.1"), "tag v1.0.1 should exist");
}

#[test]
fn bump_patch_scheme_rejects_breaking_without_force() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Write a .git-std.toml with patch scheme.
    std::fs::write(dir.path().join(".git-std.toml"), "scheme = \"patch\"\n").unwrap();

    // Add a breaking change commit.
    add_commit(dir.path(), "a.txt", "feat!: remove old API");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "breaking change not allowed on patch-only branch (use --force to override)",
        ));
}

#[test]
fn bump_patch_scheme_allows_breaking_with_force() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Write a .git-std.toml with patch scheme.
    std::fs::write(dir.path().join(".git-std.toml"), "scheme = \"patch\"\n").unwrap();

    // Add a breaking change commit.
    add_commit(dir.path(), "a.txt", "feat!: remove old API");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--force"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.0.0 \u{2192} 1.0.1"))
        .stderr(predicate::str::contains("patch"));

    // Verify tag was created.
    assert!(tag_exists(dir.path(), "v1.0.1"), "tag v1.0.1 should exist");
}

// --- Stable branch integration tests (#139) ---

#[test]
fn bump_stable_creates_branch_and_bumps_major() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "feat: new feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--stable", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Creating stable branch"))
        .stderr(predicate::str::contains("stable-v1.0"))
        .stderr(predicate::str::contains("patch (patch-only bumps)"))
        .stderr(predicate::str::contains("Committed"))
        .stderr(predicate::str::contains("1.0.0 \u{2192} 2.0.0"))
        .stderr(predicate::str::contains("major"))
        .stderr(predicate::str::contains("Tagged"));

    // Verify the stable branch was created.
    assert!(
        branch_exists(dir.path(), "stable-v1.0"),
        "stable-v1.0 branch should exist"
    );

    // Verify HEAD is back on the original branch with the new tag.
    assert!(tag_exists(dir.path(), "v2.0.0"), "tag v2.0.0 should exist");

    // Verify main was bumped in Cargo.toml.
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(
        cargo.contains("version = \"2.0.0\""),
        "expected version 2.0.0, got: {cargo}"
    );

    // Verify the stable branch has .git-std.toml with scheme = "patch".
    let config_content = git(dir.path(), &["show", "stable-v1.0:.git-std.toml"]);
    assert!(
        config_content.contains("scheme = \"patch\""),
        "stable branch config should have scheme = \"patch\", got: {config_content}"
    );
}

#[test]
fn bump_stable_with_minor_flag() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "feat: new feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--stable", "--minor", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.0.0 \u{2192} 1.1.0"))
        .stderr(predicate::str::contains("minor"));

    // Verify the tag.
    assert!(tag_exists(dir.path(), "v1.1.0"), "tag v1.1.0 should exist");

    // Verify Cargo.toml.
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(
        cargo.contains("version = \"1.1.0\""),
        "expected version 1.1.0, got: {cargo}"
    );
}

#[test]
fn bump_stable_custom_branch_name() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "feat: new feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--stable", "my-release-branch", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("my-release-branch"));

    // Verify the custom branch was created.
    assert!(
        branch_exists(dir.path(), "my-release-branch"),
        "my-release-branch should exist"
    );
}

#[test]
fn bump_stable_dry_run() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "feat: new feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--stable", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Creating stable branch"))
        .stderr(predicate::str::contains("stable-v1.0"))
        .stderr(predicate::str::contains("Would commit"))
        .stderr(predicate::str::contains("Would tag"))
        .stderr(predicate::str::contains("Advancing"))
        .stderr(predicate::str::contains("1.0.0 \u{2192} 2.0.0"));

    // No branch should be created.
    assert!(
        !branch_exists(dir.path(), "stable-v1.0"),
        "stable-v1.0 branch should NOT exist in dry-run"
    );

    // No tag should be created.
    assert!(
        !tag_exists(dir.path(), "v2.0.0"),
        "tag v2.0.0 should NOT exist in dry-run"
    );
}

#[test]
fn bump_stable_rejects_existing_branch() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "feat: new feature");

    // Pre-create the branch that --stable would try to create.
    git(dir.path(), &["branch", "stable-v1.0"]);

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--stable", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "branch 'stable-v1.0' already exists",
        ));
}

#[test]
fn bump_stable_rejects_calver_scheme() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());

    // Write a .git-std.toml with calver scheme.
    std::fs::write(dir.path().join(".git-std.toml"), "scheme = \"calver\"\n").unwrap();

    add_commit(dir.path(), "a.txt", "feat: new feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--stable"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "--stable is not supported with scheme = \"calver\"",
        ));
}

#[test]
fn bump_stable_rejects_dirty_working_tree() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "feat: new feature");

    // Create an uncommitted file to make the working tree dirty.
    std::fs::write(dir.path().join("dirty.txt"), "uncommitted").unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--stable"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "working tree has uncommitted changes",
        ));
}

// --- Custom version file (regex) integration tests ---

#[test]
fn bump_custom_regex_version_file() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Write a custom version file.
    std::fs::write(dir.path().join("version.txt"), "version = \"1.0.0\"\n").unwrap();

    // Write .git-std.toml with a [[version_files]] entry.
    std::fs::write(
        dir.path().join(".git-std.toml"),
        r#"[[version_files]]
path = "version.txt"
regex = 'version = "(\d+\.\d+\.\d+)"'
"#,
    )
    .unwrap();

    add_commit(dir.path(), "a.txt", "feat: add feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.0.0 → 1.1.0"));

    // Verify the custom file was updated.
    let content = std::fs::read_to_string(dir.path().join("version.txt")).unwrap();
    assert!(
        content.contains("version = \"1.1.0\""),
        "custom version file should be updated, got: {content}"
    );

    // Cargo.toml should also be updated.
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"1.1.0\""));
}

#[test]
fn bump_multiple_custom_files() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Write two custom version files.
    std::fs::write(dir.path().join("version.txt"), "version = \"1.0.0\"\n").unwrap();
    std::fs::write(
        dir.path().join("Chart.yaml"),
        "name: my-chart\nversion: 1.0.0\nappVersion: 1.0.0\n",
    )
    .unwrap();

    // Config with two [[version_files]] entries.
    std::fs::write(
        dir.path().join(".git-std.toml"),
        r#"[[version_files]]
path = "version.txt"
regex = 'version = "(\d+\.\d+\.\d+)"'

[[version_files]]
path = "Chart.yaml"
regex = 'version:\s*(\S+)'
"#,
    )
    .unwrap();

    add_commit(dir.path(), "a.txt", "feat: add feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.0.0 → 1.1.0"));

    // Verify both custom files were updated.
    let version_txt = std::fs::read_to_string(dir.path().join("version.txt")).unwrap();
    assert!(
        version_txt.contains("version = \"1.1.0\""),
        "version.txt should be updated, got: {version_txt}"
    );

    let chart = std::fs::read_to_string(dir.path().join("Chart.yaml")).unwrap();
    assert!(
        chart.contains("version: 1.1.0"),
        "Chart.yaml version should be updated, got: {chart}"
    );
}

#[test]
fn bump_custom_file_missing() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Config points to a nonexistent file.
    std::fs::write(
        dir.path().join(".git-std.toml"),
        r#"[[version_files]]
path = "nonexistent.txt"
regex = 'version = "(\d+\.\d+\.\d+)"'
"#,
    )
    .unwrap();

    add_commit(dir.path(), "a.txt", "feat: add feature");

    // Should still succeed — missing custom files are skipped silently.
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.0.0 → 1.1.0"));

    // Cargo.toml should still be updated.
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"1.1.0\""));
}

#[test]
fn bump_dry_run_shows_custom_files() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Write a custom version file.
    std::fs::write(dir.path().join("version.txt"), "version = \"1.0.0\"\n").unwrap();

    // Config with a [[version_files]] entry.
    std::fs::write(
        dir.path().join(".git-std.toml"),
        r#"[[version_files]]
path = "version.txt"
regex = 'version = "(\d+\.\d+\.\d+)"'
"#,
    )
    .unwrap();

    add_commit(dir.path(), "a.txt", "feat: add feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("version.txt"))
        .stderr(predicate::str::contains("Cargo.toml"))
        .stderr(predicate::str::contains("Would update"));
}

// ── commit --dry-run with auto-discover scopes (#72) ────────────

#[test]
fn commit_dry_run_auto_scopes() {
    let dir = tempfile::tempdir().unwrap();
    init_commit_repo(dir.path());
    std::fs::create_dir_all(dir.path().join("crates/web")).unwrap();
    std::fs::create_dir_all(dir.path().join("crates/api")).unwrap();
    std::fs::write(dir.path().join(".git-std.toml"), "scopes = \"auto\"\n").unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "commit",
            "--type",
            "feat",
            "--scope",
            "web",
            "--message",
            "add page",
            "--dry-run",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("feat(web): add page"));
}

#[test]
fn completions_bash_outputs_script() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicates::str::contains("complete"));
}

#[test]
fn completions_zsh_outputs_script() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicates::str::contains("_git-std"));
}

#[test]
fn completions_fish_outputs_script() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicates::str::contains("complete"));
}
