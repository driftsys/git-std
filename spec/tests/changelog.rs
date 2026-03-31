#[path = "../support/mod.rs"]
mod support;

use snapbox::cmd::Command;
use snapbox::file;
use support::TestRepo;

/// `changelog` prints the changelog to stdout by default.
#[test]
fn changelog_stdout_prints_to_stdout() {
    let mut repo = TestRepo::new().with_cargo_toml("0.0.0");
    repo.add_commit("chore: init");
    repo.create_tag("v0.1.0");
    repo.add_commit("feat: add feature A");
    repo.add_commit("fix: correct edge case");

    Command::new(TestRepo::bin_path())
        .args(["changelog"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout_eq(file![
            "../snapshots/changelog/stdout_prints_to_stdout.stdout.expected"
        ]);
}

/// `changelog --full -w` regenerates the entire changelog file.
#[test]
fn changelog_full_creates_file() {
    let mut repo = TestRepo::new().with_cargo_toml("0.0.0");
    repo.add_commit("feat: initial feature");
    repo.create_tag("v0.1.0");
    repo.add_commit("fix: a fix");
    repo.create_tag("v0.1.1");

    Command::new(TestRepo::bin_path())
        .args(["changelog", "--full", "-w"])
        .current_dir(repo.path())
        .assert()
        .success();

    let changelog = std::fs::read_to_string(repo.path().join("CHANGELOG.md")).unwrap();
    assert!(
        changelog.contains("# Changelog"),
        "full changelog should have a heading"
    );
    assert!(
        changelog.contains("0.1.1"),
        "full changelog should contain v0.1.1 section"
    );
    assert!(
        changelog.contains("Bug Fixes"),
        "full changelog should contain Bug Fixes section"
    );
}

/// `changelog --range v1.0.0..v2.0.0` prints only commits between the two tags.
#[test]
fn changelog_range_between_tags() {
    let mut repo = TestRepo::new().with_cargo_toml("0.0.0");
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("feat: add feature A");
    repo.add_commit("fix: correct edge case");
    repo.create_tag("v2.0.0");
    repo.add_commit("feat: post-release feature");

    Command::new(TestRepo::bin_path())
        .args(["changelog", "--range", "v1.0.0..v2.0.0"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout_eq(file![
            "../snapshots/changelog/range_between_tags.stdout.expected"
        ]);
}

/// `changelog` with no commits after the latest tag reports no unreleased changes.
#[test]
fn changelog_no_unreleased_changes() {
    let mut repo = TestRepo::new().with_cargo_toml("1.0.0");
    repo.add_commit("feat: initial");
    repo.create_tag("v1.0.0");

    Command::new(TestRepo::bin_path())
        .args(["changelog"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/changelog/no_unreleased_changes.stderr.expected"
        ]);
}
