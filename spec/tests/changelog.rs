#[path = "../support/mod.rs"]
mod support;

use snapbox::cmd::Command;
use snapbox::file;
use support::TestRepo;

/// `changelog --stdout` prints the changelog to stdout.
#[test]
fn changelog_stdout_prints_to_stdout() {
    let mut repo = TestRepo::new().with_cargo_toml("0.0.0");
    repo.add_commit("chore: init");
    repo.create_tag("v0.1.0");
    repo.add_commit("feat: add feature A");
    repo.add_commit("fix: correct edge case");

    Command::new(TestRepo::bin_path())
        .args(["changelog", "--stdout"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout_eq(file![
            "../snapshots/changelog/stdout_prints_to_stdout.stdout.expected"
        ]);
}

/// `changelog --full` regenerates the entire changelog file.
#[test]
fn changelog_full_creates_file() {
    let mut repo = TestRepo::new().with_cargo_toml("0.0.0");
    repo.add_commit("feat: initial feature");
    repo.create_tag("v0.1.0");
    repo.add_commit("fix: a fix");
    repo.create_tag("v0.1.1");

    Command::new(TestRepo::bin_path())
        .args(["changelog", "--full"])
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
