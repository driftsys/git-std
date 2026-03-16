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
        .stderr_eq(file!["../snapshots/bump/dry_run_shows_plan.stderr.expected"]);
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
        .stderr_eq(file!["../snapshots/bump/no_bump_worthy_commits.stderr.expected"]);
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
        .stderr_eq(file!["../snapshots/bump/breaking_change_major.stderr.expected"]);
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
