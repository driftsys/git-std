use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

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

/// Helper: get HEAD commit message.
fn head_message(dir: &Path) -> String {
    git(dir, &["log", "-1", "--format=%B"]).trim().to_string()
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
