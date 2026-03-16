#[path = "../support/mod.rs"]
mod support;

use snapbox::cmd::Command;
use snapbox::file;
use support::TestRepo;

/// Bump dry-run shows the version transition plan without writing.
#[test]
fn bump_dry_run_shows_plan() {
    let mut repo = TestRepo::new().with_cargo_toml("0.0.0");
    repo.add_commit("chore: init");
    repo.create_tag("v0.1.0");
    repo.add_commit("feat: add feature A");

    Command::new(TestRepo::bin_path())
        .args(["bump", "--dry-run"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/bump/dry_run_shows_plan.stderr.expected"
        ]);
}

/// Bump with no bump-worthy commits reports that and exits 0.
#[test]
fn bump_no_bump_worthy_commits() {
    let mut repo = TestRepo::new().with_cargo_toml("0.0.0");
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("chore: update deps");

    Command::new(TestRepo::bin_path())
        .args(["bump"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/bump/no_bump_worthy_commits.stderr.expected"
        ]);
}

/// Bump with a breaking change produces a major version bump.
#[test]
fn bump_breaking_change_major() {
    let mut repo = TestRepo::new().with_cargo_toml("0.0.0");
    repo.add_commit("chore: init");
    repo.create_tag("v1.2.3");
    repo.add_commit("feat!: remove old API");

    Command::new(TestRepo::bin_path())
        .args(["bump", "--dry-run"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/bump/breaking_change_major.stderr.expected"
        ]);
}

/// First release uses current version without bumping.
#[test]
fn bump_first_release() {
    let mut repo = TestRepo::new().with_cargo_toml("0.0.0");
    repo.add_commit("feat: initial feature");

    Command::new(TestRepo::bin_path())
        .args(["bump", "--first-release"])
        .current_dir(repo.path())
        .assert()
        .success();

    // CHANGELOG.md should be created.
    assert!(repo.path().join("CHANGELOG.md").exists());

    // Version should stay at 0.0.0 (first-release doesn't bump).
    let cargo = std::fs::read_to_string(repo.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"0.0.0\""));
}

/// `--release-as` forces a specific version, skipping calculation.
#[test]
fn bump_release_as_forced_version() {
    let mut repo = TestRepo::new().with_cargo_toml("0.0.0");
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("fix: patch something");

    Command::new(TestRepo::bin_path())
        .args(["bump", "--release-as", "3.0.0", "--dry-run"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/bump/release_as_forced_version.stderr.expected"
        ]);
}

/// `--prerelease rc` produces pre-release versions that increment.
#[test]
fn bump_prerelease_cycle() {
    let mut repo = TestRepo::new().with_cargo_toml("0.0.0");
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("feat: new feature");

    // First pre-release: 1.0.0 → 2.0.0-rc.0
    Command::new(TestRepo::bin_path())
        .args(["bump", "--prerelease", "rc", "--dry-run"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/bump/prerelease_cycle_first.stderr.expected"
        ]);

    // Run the actual bump to create the rc.0 tag.
    Command::new(TestRepo::bin_path())
        .args(["bump", "--prerelease", "rc", "--skip-changelog"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Second pre-release with another commit: 2.0.0-rc.0 → 2.0.0-rc.1
    repo.add_commit("feat: another feature");

    Command::new(TestRepo::bin_path())
        .args(["bump", "--prerelease", "rc", "--dry-run"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/bump/prerelease_cycle_second.stderr.expected"
        ]);
}

/// `--no-tag` creates a commit but skips tag creation.
#[test]
fn bump_no_tag_flag() {
    let mut repo = TestRepo::new().with_cargo_toml("0.0.0");
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("feat: new feature");

    Command::new(TestRepo::bin_path())
        .args(["bump", "--no-tag", "--skip-changelog"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Version file should be updated.
    let cargo = std::fs::read_to_string(repo.path().join("Cargo.toml")).unwrap();
    assert!(
        cargo.contains("version = \"1.1.0\""),
        "expected version 1.1.0, got: {cargo}"
    );

    // Tag should NOT exist.
    let output = std::process::Command::new("git")
        .current_dir(repo.path())
        .args(["tag", "-l"])
        .output()
        .unwrap();
    let tags = String::from_utf8_lossy(&output.stdout);
    assert!(
        !tags.contains("v1.1.0"),
        "tag v1.1.0 should not exist, found tags: {tags}"
    );
}

/// `--no-commit` updates files but skips commit and tag.
#[test]
fn bump_no_commit_flag() {
    let mut repo = TestRepo::new().with_cargo_toml("0.0.0");
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("feat: new feature");

    Command::new(TestRepo::bin_path())
        .args(["bump", "--no-commit", "--skip-changelog"])
        .current_dir(repo.path())
        .assert()
        .success();

    // Version file should be updated.
    let cargo = std::fs::read_to_string(repo.path().join("Cargo.toml")).unwrap();
    assert!(
        cargo.contains("version = \"1.1.0\""),
        "expected version 1.1.0, got: {cargo}"
    );

    // No new commit should have been created — HEAD message should still
    // be the feat commit, not a release commit.
    let msg_output = std::process::Command::new("git")
        .current_dir(repo.path())
        .args(["log", "-1", "--format=%s"])
        .output()
        .unwrap();
    let msg = String::from_utf8_lossy(&msg_output.stdout);
    assert!(
        !msg.contains("chore(release)"),
        "expected no release commit, got: {msg}"
    );

    // Tag should NOT exist.
    let output = std::process::Command::new("git")
        .current_dir(repo.path())
        .args(["tag", "-l"])
        .output()
        .unwrap();
    let tags = String::from_utf8_lossy(&output.stdout);
    assert!(
        !tags.contains("v1.1.0"),
        "tag v1.1.0 should not exist, found tags: {tags}"
    );
}

/// Dry-run detects both `Cargo.toml` and `package.json` version files.
#[test]
fn bump_multi_ecosystem() {
    let mut repo = TestRepo::new()
        .with_cargo_toml("1.0.0")
        .with_package_json("1.0.0");
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("feat: cross-platform feature");

    Command::new(TestRepo::bin_path())
        .args(["bump", "--dry-run"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file!["../snapshots/bump/multi_ecosystem.stderr.expected"]);
}

/// Dry-run with custom `[[version_files]]` shows the custom file in the plan.
#[test]
fn bump_dry_run_custom_version_file() {
    let mut repo = TestRepo::new().with_cargo_toml("1.0.0").with_config(
        r#"
[[version_files]]
path = "version.txt"
regex = 'version = "(\d+\.\d+\.\d+)"'
"#,
    );

    // Create the custom version file.
    std::fs::write(repo.path().join("version.txt"), "version = \"1.0.0\"\n").unwrap();

    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("feat: add feature");

    Command::new(TestRepo::bin_path())
        .args(["bump", "--dry-run"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/bump/dry_run_custom_version_file.stderr.expected"
        ]);
}

/// Calver scheme produces a date-based version (YYYY.MM.PATCH).
#[test]
fn bump_calver_scheme() {
    let mut repo = TestRepo::new()
        .with_cargo_toml("0.0.0")
        .with_config("scheme = \"calver\"\n");
    repo.add_commit("feat: initial feature");

    Command::new(TestRepo::bin_path())
        .args(["bump", "--dry-run"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file!["../snapshots/bump/calver_scheme.stderr.expected"]);
}

/// `--skip-changelog` dry-run omits the CHANGELOG.md line from the plan.
#[test]
fn bump_skip_changelog_dry_run() {
    let mut repo = TestRepo::new().with_cargo_toml("1.0.0");
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("feat: new feature");

    Command::new(TestRepo::bin_path())
        .args(["bump", "--skip-changelog", "--dry-run"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/bump/skip_changelog_dry_run.stderr.expected"
        ]);
}

/// Patch scheme bumps patch for a feat commit.
#[test]
fn bump_patch_scheme_dry_run() {
    let mut repo = TestRepo::new()
        .with_cargo_toml("0.0.0")
        .with_config("scheme = \"patch\"\n");
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("feat: new feature");

    Command::new(TestRepo::bin_path())
        .args(["bump", "--dry-run"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/bump/patch_scheme_dry_run.stderr.expected"
        ]);
}

/// `--stable --dry-run` shows the stable branch creation plan.
#[test]
fn bump_stable_dry_run() {
    let mut repo = TestRepo::new().with_cargo_toml("0.0.0");
    // Stage Cargo.toml so the working tree is clean (--stable requires it).
    std::process::Command::new("git")
        .current_dir(repo.path())
        .args(["add", "Cargo.toml"])
        .status()
        .unwrap();
    repo.add_commit("chore: init");
    repo.create_tag("v1.2.0");
    repo.add_commit("feat!: breaking change");

    Command::new(TestRepo::bin_path())
        .args(["bump", "--stable", "--dry-run"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file!["../snapshots/bump/stable_dry_run.stderr.expected"]);
}

/// `--skip-changelog` actually skips writing CHANGELOG.md.
#[test]
fn bump_skip_changelog_no_file() {
    let mut repo = TestRepo::new().with_cargo_toml("1.0.0");
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("feat: new feature");

    Command::new(TestRepo::bin_path())
        .args(["bump", "--skip-changelog"])
        .current_dir(repo.path())
        .assert()
        .success();

    assert!(
        !repo.path().join("CHANGELOG.md").exists(),
        "CHANGELOG.md should not be created when --skip-changelog is used"
    );
}
