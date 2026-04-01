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

    // Pre-1.0: feat (Minor) downshifts to Patch → 0.1.0 → 0.1.1.
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("0.1.0 → 0.1.1"))
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

/// A lock file for a missing tool emits a warning but does not fail the bump.
#[test]
fn bump_missing_tool_lock_file_warns_and_continues() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Write a pyproject.toml so the trigger for uv.lock is satisfied.
    std::fs::write(
        dir.path().join("pyproject.toml"),
        "[project]\nname = \"test\"\nversion = \"1.0.0\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join(".git-std.toml"),
        "[[version_files]]\npath = \"pyproject.toml\"\nregex = 'version = \"([^\"]+)\"'\n",
    )
    .unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-m", "chore: add pyproject"]);

    add_commit(dir.path(), "a.txt", "fix: small fix");

    // Write a uv.lock — override PATH to only include git's directory so `uv`
    // cannot be found regardless of the host environment, isolating the
    // "tool not on PATH" warning path.
    std::fs::write(dir.path().join("uv.lock"), "# placeholder\n").unwrap();

    // Build a minimal PATH that contains git (needed for bump internals)
    // but excludes ecosystem tools like uv, npm, etc.
    let git_dir = std::process::Command::new("which")
        .arg("git")
        .output()
        .map(|o| {
            let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
            std::path::PathBuf::from(p)
                .parent()
                .unwrap()
                .to_string_lossy()
                .to_string()
        })
        .unwrap();

    // Bump must still succeed.
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .env("PATH", &git_dir)
        .assert()
        .success();

    // Cargo.toml was updated — version bump happened.
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"1.0.1\""));
}

/// dry-run with a lock file present mentions "Would sync".
#[test]
fn bump_dry_run_shows_would_sync() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Write a pyproject.toml so the trigger for uv.lock is satisfied.
    std::fs::write(
        dir.path().join("pyproject.toml"),
        "[project]\nname = \"test\"\nversion = \"1.0.0\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join(".git-std.toml"),
        "[[version_files]]\npath = \"pyproject.toml\"\nregex = 'version = \"([^\"]+)\"'\n",
    )
    .unwrap();
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-m", "chore: add pyproject"]);

    add_commit(dir.path(), "a.txt", "feat: new feature");

    // Write a uv.lock so the dry-run path has something to report.
    std::fs::write(dir.path().join("uv.lock"), "# placeholder\n").unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Would sync"))
        .stderr(predicate::str::contains("uv.lock"));
}

/// dry-run with a lock file present does NOT mention "Would sync" when the
/// trigger version file is not in the version_files list.
#[test]
fn bump_dry_run_skips_untriggered_lock_file() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "feat: new feature");

    // Write a uv.lock on disk but NO pyproject.toml in version_files.
    std::fs::write(dir.path().join("uv.lock"), "# placeholder\n").unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Would sync:   uv.lock").not());
}

// ── Pre-1.0 semver convention ──────────────────────────────────

/// Pre-1.0: a breaking change bumps minor, not major.
#[test]
fn bump_pre1_breaking_bumps_minor() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v0.10.2");

    add_commit(dir.path(), "brk.txt", "feat!: remove old API");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("0.10.2 → 0.11.0"));

    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"0.11.0\""));
    assert!(tag_exists(dir.path(), "v0.11.0"));
}

/// Pre-1.0: a feat bumps patch, not minor.
#[test]
fn bump_pre1_feat_bumps_patch() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v0.10.2");

    add_commit(dir.path(), "ft.txt", "feat: add feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("0.10.2 → 0.10.3"));

    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"0.10.3\""));
}

/// Pre-1.0: a fix bumps patch.
#[test]
fn bump_pre1_fix_bumps_patch() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v0.10.2");

    add_commit(dir.path(), "fx.txt", "fix: handle edge case");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("0.10.2 → 0.10.3"));
}

/// Pre-1.0: --release-as still overrides computed version.
#[test]
fn bump_pre1_release_as_overrides() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v0.10.2");

    add_commit(dir.path(), "ra.txt", "feat: something");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--release-as", "1.0.0"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("0.10.2 → 1.0.0"));

    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"1.0.0\""));
    assert!(tag_exists(dir.path(), "v1.0.0"));
}

/// Pre-1.0: --dry-run shows the downshifted bump plan.
#[test]
fn bump_pre1_dry_run_shows_correct_plan() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v0.10.2");

    add_commit(dir.path(), "dr.txt", "feat!: breaking API change");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("0.10.2 → 0.11.0"))
        .stderr(predicate::str::contains("Would commit"))
        .stderr(predicate::str::contains("Would tag"));
}

/// Post-1.0: breaking change still bumps major (behaviour unchanged).
#[test]
fn bump_post1_breaking_bumps_major() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.2.3");

    add_commit(dir.path(), "brk2.txt", "feat!: remove old API");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.2.3 → 2.0.0"));
}

// ── Bump lifecycle hooks ───────────────────────────────────────

/// Helper: write a `.githooks/<name>.hooks` file with the given content.
fn write_hooks_file(dir: &std::path::Path, hook_name: &str, content: &str) {
    let hooks_dir = dir.join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(hooks_dir.join(format!("{hook_name}.hooks")), content).unwrap();
}

/// `pre-bump` with a required command that exits 0 — bump proceeds normally.
#[test]
fn bump_lifecycle_pre_bump_passes() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "fix: a fix");

    write_hooks_file(dir.path(), "pre-bump", "! true\n");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.0.0 → 1.0.1"));
}

/// `pre-bump` with a required command that exits non-zero — bump is aborted.
#[test]
fn bump_lifecycle_pre_bump_aborts_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "fix: a fix");

    write_hooks_file(dir.path(), "pre-bump", "! false\n");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .failure();

    // Cargo.toml must not have been updated.
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(
        !cargo.contains("version = \"1.0.1\""),
        "Cargo.toml must not be updated when pre-bump fails"
    );
    // No tag must exist.
    assert!(!tag_exists(dir.path(), "v1.0.1"), "tag must not exist");
}

/// `pre-bump` is skipped when `--dry-run` is used.
#[test]
fn bump_lifecycle_pre_bump_skipped_on_dry_run() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "fix: a fix");

    // A failing pre-bump hook must not abort dry-run.
    write_hooks_file(dir.path(), "pre-bump", "! false\n");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success();
}

/// `post-version` receives the new version as its first argument.
#[test]
fn bump_lifecycle_post_version_receives_version() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "fix: a fix");

    // Write the first positional arg ($1) to a file inside the tempdir.
    let sentinel = dir.path().join("post-version-arg.txt");
    let sentinel_str = sentinel.to_string_lossy();
    write_hooks_file(
        dir.path(),
        "post-version",
        &format!("! echo $1 > {sentinel_str}\n"),
    );

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success();

    let written = std::fs::read_to_string(&sentinel)
        .unwrap_or_default()
        .trim()
        .to_string();
    assert_eq!(written, "1.0.1", "post-version should receive '1.0.1'");
}

/// `post-version` aborts bump when it exits non-zero.
#[test]
fn bump_lifecycle_post_version_aborts_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "fix: a fix");

    write_hooks_file(dir.path(), "post-version", "! false\n");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .failure();

    // No tag should have been created.
    assert!(!tag_exists(dir.path(), "v1.0.1"), "tag must not exist");
}

/// `post-changelog` runs after the changelog is written.
#[test]
fn bump_lifecycle_post_changelog_runs() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "fix: a fix");

    // Touch a sentinel file inside the tempdir so we know the hook ran.
    let sentinel = dir.path().join("post-changelog-ran.txt");
    let sentinel_str = sentinel.to_string_lossy();
    write_hooks_file(
        dir.path(),
        "post-changelog",
        &format!("! touch {sentinel_str}\n"),
    );

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success();

    assert!(
        sentinel.exists(),
        "post-changelog sentinel file must exist after bump"
    );
}

/// `post-changelog` aborts bump when it exits non-zero.
#[test]
fn bump_lifecycle_post_changelog_aborts_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "fix: a fix");

    write_hooks_file(dir.path(), "post-changelog", "! false\n");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .failure();

    // No release commit should have been created.
    assert_ne!(
        head_message(dir.path()),
        "chore(release): 1.0.1",
        "release commit must not exist"
    );
}

/// `post-changelog` is skipped when `--skip-changelog` is used.
#[test]
fn bump_lifecycle_post_changelog_skipped_when_skip_changelog() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "fix: a fix");

    // Failing post-changelog hook must not abort bump when --skip-changelog is passed.
    write_hooks_file(dir.path(), "post-changelog", "! false\n");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .success();
}

/// `post-bump` runs after the release tag is created.
#[test]
fn bump_lifecycle_post_bump_runs_after_tag() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "fix: a fix");

    // Touch a sentinel file inside the tempdir.
    let sentinel = dir.path().join("post-bump-ran.txt");
    let sentinel_str = sentinel.to_string_lossy();
    write_hooks_file(
        dir.path(),
        "post-bump",
        &format!("! touch {sentinel_str}\n"),
    );

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Tag must already exist when post-bump runs (hook runs after tag).
    assert!(tag_exists(dir.path(), "v1.0.1"), "tag must exist");
    assert!(
        sentinel.exists(),
        "post-bump sentinel file must exist after bump"
    );
}

/// `post-bump` with advisory command (`?`) — failure is tolerated.
#[test]
fn bump_lifecycle_post_bump_advisory_tolerates_failure() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "fix: a fix");

    // Advisory command that always fails.
    write_hooks_file(dir.path(), "post-bump", "? false\n");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Tag should still have been created.
    assert!(tag_exists(dir.path(), "v1.0.1"), "tag must exist");
}

/// `post-bump` is skipped when `--no-commit` is used (no commit/tag created).
#[test]
fn bump_lifecycle_post_bump_skipped_on_no_commit() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "fix: a fix");

    // Failing post-bump hook must not abort bump when --no-commit is passed.
    write_hooks_file(dir.path(), "post-bump", "! false\n");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--no-commit"])
        .current_dir(dir.path())
        .assert()
        .success();
}

/// `GIT_STD_SKIP_HOOKS=1` skips all lifecycle hooks.
#[test]
fn bump_lifecycle_skip_hooks_env_var() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "fix: a fix");

    // All hooks fail — GIT_STD_SKIP_HOOKS=1 must suppress them all.
    for hook in ["pre-bump", "post-version", "post-changelog", "post-bump"] {
        write_hooks_file(dir.path(), hook, "! false\n");
    }

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .env("GIT_STD_SKIP_HOOKS", "1")
        .assert()
        .success();
}

/// `--dry-run` skips all lifecycle hooks (`post-version`, `post-changelog`,
/// `post-bump`).
#[test]
fn bump_lifecycle_dry_run_skips_all_hooks() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "fix: a fix");

    // All four hooks fail if executed — dry-run must not run any of them.
    for hook in ["pre-bump", "post-version", "post-changelog", "post-bump"] {
        write_hooks_file(dir.path(), hook, "! false\n");
    }

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success();
}

/// Cargo.lock is synced when `cargo` is available and Cargo.toml was updated.
#[test]
fn bump_cargo_lock_sync_happy_path() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Generate a real Cargo.lock so the lock sync has a file to update.
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), "").unwrap();
    std::process::Command::new("cargo")
        .args(["generate-lockfile"])
        .current_dir(dir.path())
        .status()
        .expect("cargo must be available");
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-m", "chore: add lock"]);

    add_commit(dir.path(), "a.txt", "fix: a bug");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Synced:"));

    // Verify the lock file was staged as part of the bump commit.
    let files_in_commit = git(
        dir.path(),
        &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"],
    );
    assert!(
        files_in_commit.contains("Cargo.lock"),
        "Cargo.lock should be staged in the bump commit, got: {files_in_commit}"
    );
}
