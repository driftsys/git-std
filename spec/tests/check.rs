#[path = "../support/mod.rs"]
mod support;

use snapbox::cmd::Command;
use snapbox::file;
use support::TestRepo;

#[test]
fn trycmd_check() {
    trycmd::TestCases::new().case("tests/cmd/check/*.toml");
}

/// `check --range` with a mix of valid and invalid commits reports both and exits 1.
#[test]
fn check_range_mixed_valid_invalid() {
    let mut repo = TestRepo::new();
    repo.add_commit("feat: initial");
    repo.add_commit("fix: valid one");
    repo.add_commit("invalid message");

    // Range from first commit to HEAD (all commits after first).
    let repo_git = git2::Repository::open(repo.path()).unwrap();
    let mut revwalk = repo_git.revwalk().unwrap();
    revwalk.push_head().unwrap();
    revwalk.set_sorting(git2::Sort::REVERSE).unwrap();
    let first_oid = revwalk.next().unwrap().unwrap();
    let range = format!("{}..HEAD", &first_oid.to_string()[..7]);

    Command::new(TestRepo::bin_path())
        .args(["check", "--range", &range])
        .current_dir(repo.path())
        .assert()
        .code(1)
        .stderr_eq(file![
            "../snapshots/check/range_mixed_valid_invalid.stderr.expected"
        ]);
}

/// `check --strict` rejects types not in the configured allowed list.
#[test]
fn check_strict_rejects_unknown_type() {
    let repo = TestRepo::new().with_config("types = [\"feat\", \"fix\"]\n");

    Command::new(TestRepo::bin_path())
        .args(["check", "--strict", "docs: update readme"])
        .current_dir(repo.path())
        .assert()
        .code(1)
        .stderr_eq(file![
            "../snapshots/check/strict_rejects_unknown_type.stderr.expected"
        ]);
}

/// `check --strict` with scopes configured requires a scope and rejects unknown scopes.
#[test]
fn check_strict_rejects_missing_scope() {
    let repo =
        TestRepo::new().with_config("types = [\"feat\", \"fix\"]\nscopes = [\"auth\", \"api\"]\n");

    Command::new(TestRepo::bin_path())
        .args(["check", "--strict", "feat: no scope provided"])
        .current_dir(repo.path())
        .assert()
        .code(1)
        .stderr_eq(file![
            "../snapshots/check/strict_rejects_missing_scope.stderr.expected"
        ]);
}

/// `check --strict` with scopes configured rejects unknown scopes.
#[test]
fn check_strict_rejects_unknown_scope() {
    let repo =
        TestRepo::new().with_config("types = [\"feat\", \"fix\"]\nscopes = [\"auth\", \"api\"]\n");

    Command::new(TestRepo::bin_path())
        .args(["check", "--strict", "feat(unknown): add login"])
        .current_dir(repo.path())
        .assert()
        .code(1)
        .stderr_eq(file![
            "../snapshots/check/strict_rejects_unknown_scope.stderr.expected"
        ]);
}

/// `check --strict --format json` returns structured errors for unknown types.
#[test]
fn check_strict_json_rejects_unknown_type() {
    let repo = TestRepo::new().with_config("types = [\"feat\", \"fix\"]\n");

    Command::new(TestRepo::bin_path())
        .args([
            "check", "--strict", "--format", "json", "docs: update readme",
        ])
        .current_dir(repo.path())
        .assert()
        .code(1)
        .stdout_eq(file![
            "../snapshots/check/strict_json_rejects_unknown_type.stdout.expected"
        ]);
}

/// `check --format json` with an invalid message returns structured errors.
#[test]
fn check_json_invalid_message() {
    Command::new(TestRepo::bin_path())
        .args(["check", "--format", "json", "bad message"])
        .assert()
        .code(1)
        .stdout_eq(file![
            "../snapshots/check/json_invalid_message.stdout.expected"
        ]);
}
