#[path = "../support/mod.rs"]
mod support;

use snapbox::cmd::Command;
use support::TestRepo;

/// Basic version query returns the tag version.
#[test]
fn version_prints_current() {
    let mut repo = TestRepo::new();
    repo.add_commit("chore: init");
    repo.create_tag("v1.2.3");

    Command::new(TestRepo::bin_path())
        .args(["version"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout_matches("1.2.3\n");
}

/// When no tags exist, `git std version` exits with a non-zero code.
#[test]
fn version_no_tag_exits_nonzero() {
    let mut repo = TestRepo::new();
    repo.add_commit("chore: init");

    Command::new(TestRepo::bin_path())
        .args(["version"])
        .current_dir(repo.path())
        .assert()
        .failure();
}

/// `--next` resolves the next semver given a feat commit since the tag.
#[test]
fn version_next_semver() {
    let mut repo = TestRepo::new();
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("feat: add new feature");

    Command::new(TestRepo::bin_path())
        .args(["version", "--next"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout_matches("1.1.0\n");
}

/// `--label` reports the bump level name (minor) for a feat commit.
#[test]
fn version_label_minor() {
    let mut repo = TestRepo::new();
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("feat: add new feature");

    Command::new(TestRepo::bin_path())
        .args(["version", "--label"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout_matches("minor\n");
}

/// `--code` returns the numeric version code for the current tag.
#[test]
fn version_code_semver() {
    let mut repo = TestRepo::new();
    repo.add_commit("chore: init");
    repo.create_tag("v1.2.3");

    Command::new(TestRepo::bin_path())
        .args(["version", "--code"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stdout_matches("10020399\n");
}

/// `--format json` output contains the expected top-level fields.
#[test]
fn version_format_json_has_fields() {
    let mut repo = TestRepo::new();
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");

    let output = Command::new(TestRepo::bin_path())
        .args(["version", "--format", "json"])
        .current_dir(repo.path())
        .output()
        .expect("failed to run git std version --format json");

    assert!(output.status.success(), "command should exit successfully");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"version\""),
        "json output should contain 'version' field, got: {stdout}"
    );
    assert!(
        stdout.contains("\"code\""),
        "json output should contain 'code' field, got: {stdout}"
    );
    assert!(
        stdout.contains("\"next\""),
        "json output should contain 'next' field, got: {stdout}"
    );
}

/// `--describe` at an exact tag shows the version without a dev suffix.
#[test]
fn version_describe_at_tag() {
    let mut repo = TestRepo::new();
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");

    let output = Command::new(TestRepo::bin_path())
        .args(["version", "--describe"])
        .current_dir(repo.path())
        .output()
        .expect("failed to run git std version --describe");

    assert!(output.status.success(), "command should exit successfully");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("1.0.0"),
        "describe output should contain '1.0.0', got: {stdout}"
    );
    assert!(
        !stdout.contains("dev"),
        "describe output at exact tag should not contain 'dev', got: {stdout}"
    );
}
