#[path = "../support/mod.rs"]
mod support;

use snapbox::cmd::Command;
use snapbox::file;
use support::TestRepo;

#[test]
fn trycmd_lint() {
    trycmd::TestCases::new().case("tests/cmd/lint/*.toml");
}

/// `lint --range` with a mix of valid and invalid commits reports both and exits 1.
#[test]
fn lint_range_mixed_valid_invalid() {
    let mut repo = TestRepo::new();
    repo.add_commit("feat: initial");
    repo.add_commit("fix: valid one");
    repo.add_commit("invalid message");

    // Range from first commit to HEAD (all commits after first).
    let output = std::process::Command::new("git")
        .current_dir(repo.path())
        .args(["rev-list", "--reverse", "HEAD"])
        .output()
        .unwrap();
    let first_oid = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap()
        .to_string();
    let range = format!("{}..HEAD", &first_oid[..7]);

    Command::new(TestRepo::bin_path())
        .args(["lint", "--range", &range])
        .current_dir(repo.path())
        .assert()
        .code(1)
        .stderr_eq(file![
            "../snapshots/check/range_mixed_valid_invalid.stderr.expected"
        ]);
}

/// `lint --strict` rejects types not in the configured allowed list.
#[test]
fn lint_strict_rejects_unknown_type() {
    let repo = TestRepo::new().with_config("types = [\"feat\", \"fix\"]\n");

    Command::new(TestRepo::bin_path())
        .args(["lint", "--strict", "docs: update readme"])
        .current_dir(repo.path())
        .assert()
        .code(1)
        .stderr_eq(file![
            "../snapshots/check/strict_rejects_unknown_type.stderr.expected"
        ]);
}

/// `lint --strict` with scopes configured requires a scope and rejects unknown scopes.
#[test]
fn lint_strict_rejects_missing_scope() {
    let repo =
        TestRepo::new().with_config("types = [\"feat\", \"fix\"]\nscopes = [\"auth\", \"api\"]\n");

    Command::new(TestRepo::bin_path())
        .args(["lint", "--strict", "feat: no scope provided"])
        .current_dir(repo.path())
        .assert()
        .code(1)
        .stderr_eq(file![
            "../snapshots/check/strict_rejects_missing_scope.stderr.expected"
        ]);
}

/// `lint --strict` with scopes configured rejects unknown scopes.
#[test]
fn lint_strict_rejects_unknown_scope() {
    let repo =
        TestRepo::new().with_config("types = [\"feat\", \"fix\"]\nscopes = [\"auth\", \"api\"]\n");

    Command::new(TestRepo::bin_path())
        .args(["lint", "--strict", "feat(unknown): add login"])
        .current_dir(repo.path())
        .assert()
        .code(1)
        .stderr_eq(file![
            "../snapshots/check/strict_rejects_unknown_scope.stderr.expected"
        ]);
}

/// `lint --strict --format json` returns structured errors for unknown types.
#[test]
fn lint_strict_json_rejects_unknown_type() {
    let repo = TestRepo::new().with_config("types = [\"feat\", \"fix\"]\n");

    Command::new(TestRepo::bin_path())
        .args([
            "lint",
            "--strict",
            "--format",
            "json",
            "docs: update readme",
        ])
        .current_dir(repo.path())
        .assert()
        .code(1)
        .stdout_eq(file![
            "../snapshots/check/strict_json_rejects_unknown_type.stdout.expected"
        ]);
}

/// `lint --format json` with an invalid message returns structured errors.
#[test]
fn lint_json_invalid_message() {
    Command::new(TestRepo::bin_path())
        .args(["lint", "--format", "json", "bad message"])
        .assert()
        .code(1)
        .stdout_eq(file![
            "../snapshots/check/json_invalid_message.stdout.expected"
        ]);
}
