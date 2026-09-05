#[path = "../support/mod.rs"]
mod support;

use snapbox::cmd::Command;
use snapbox::file;
use support::TestRepo;

/// Run a git command in `dir`, failing the test with git's own stderr if it errors.
fn run_git(dir: &std::path::Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("failed to run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

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

/// An empty `lint --range` is a no-op: nothing to lint is not a failure (#545).
#[test]
fn lint_range_empty_exits_0() {
    let mut repo = TestRepo::new();
    repo.add_commit("feat: initial");

    Command::new(TestRepo::bin_path())
        .args(["lint", "--range", "HEAD..HEAD"])
        .current_dir(repo.path())
        .assert()
        .code(0)
        .stderr_eq(file!["../snapshots/check/range_empty.stderr.expected"]);
}

/// An empty `lint --range --format json` emits an empty array, not an error (#545).
#[test]
fn lint_range_empty_json_emits_empty_array() {
    let mut repo = TestRepo::new();
    repo.add_commit("feat: initial");

    Command::new(TestRepo::bin_path())
        .args(["lint", "--range", "HEAD..HEAD", "--format", "json"])
        .current_dir(repo.path())
        .assert()
        .code(0)
        .stdout_eq(file!["../snapshots/check/range_empty_json.stdout.expected"]);
}

/// An unresolvable range is still a usage error: exit 2 keeps its meaning (#545).
#[test]
fn lint_range_unresolvable_exits_2() {
    let mut repo = TestRepo::new();
    repo.add_commit("feat: initial");

    Command::new(TestRepo::bin_path())
        .args(["lint", "--range", "nosuchref..HEAD"])
        .current_dir(repo.path())
        .assert()
        .code(2)
        .stderr_eq(file![
            "../snapshots/check/range_unresolvable.stderr.expected"
        ]);
}

/// `lint --range --format json` over a non-empty range reports one entry per commit.
#[test]
fn lint_range_json_reports_each_commit() {
    let mut repo = TestRepo::new();
    repo.add_commit("feat: initial");
    repo.add_commit("fix: valid one");
    repo.add_commit("invalid message");

    Command::new(TestRepo::bin_path())
        .args(["lint", "--range", "HEAD~2..HEAD", "--format", "json"])
        .current_dir(repo.path())
        .assert()
        .code(1)
        .stdout_eq(file!["../snapshots/check/range_json.stdout.expected"]);
}

/// Endpoints in the wrong order also produce an empty range — that is a failure,
/// not a no-op, or a commit gate would pass having validated nothing (#545).
#[test]
fn lint_range_reversed_warns_and_exits_1() {
    let mut repo = TestRepo::new();
    repo.add_commit("feat: initial");
    repo.add_commit("fix: second");

    Command::new(TestRepo::bin_path())
        .args(["lint", "--range", "HEAD..HEAD~1"])
        .current_dir(repo.path())
        .assert()
        .code(1)
        .stderr_eq(file!["../snapshots/check/range_reversed.stderr.expected"]);
}

/// The motivating case: on `main`, `main..HEAD` is empty because the two refs
/// resolve to the same commit, not because the two names match textually (#545).
#[test]
fn lint_range_empty_named_refs_exits_0() {
    let mut repo = TestRepo::new();
    repo.add_commit("feat: initial");
    run_git(repo.path(), &["branch", "-M", "main"]);

    Command::new(TestRepo::bin_path())
        .args(["lint", "--range", "main..HEAD"])
        .current_dir(repo.path())
        .assert()
        .code(0)
        .stderr_eq(file![
            "../snapshots/check/range_empty_named_refs.stderr.expected"
        ]);
}

/// A range whose inverse git cannot resolve is reported as empty, not reversed —
/// the inverse of `a...b` is built naively and is not a valid range (#545).
#[test]
fn lint_range_empty_symmetric_difference_exits_0() {
    let mut repo = TestRepo::new();
    repo.add_commit("feat: initial");

    Command::new(TestRepo::bin_path())
        .args(["lint", "--range", "HEAD...HEAD"])
        .current_dir(repo.path())
        .assert()
        .code(0)
        .stderr_eq(file![
            "../snapshots/check/range_empty_symmetric.stderr.expected"
        ]);
}

/// A range with no `..` separator cannot be inverted, so it is reported as empty (#545).
#[test]
fn lint_range_empty_without_separator_exits_0() {
    let mut repo = TestRepo::new();
    repo.add_commit("feat: initial");

    Command::new(TestRepo::bin_path())
        .args(["lint", "--range", "^HEAD"])
        .current_dir(repo.path())
        .assert()
        .code(0)
        .stderr_eq(file![
            "../snapshots/check/range_empty_no_separator.stderr.expected"
        ]);
}

/// A checkout behind its base asked `<base>..HEAD` and got an honest empty
/// answer, so the gate stays green — only the endpoint order separates this from
/// reversed endpoints, which are the same state in git (#545).
#[test]
fn lint_range_behind_base_exits_0() {
    let mut repo = TestRepo::new();
    repo.add_commit("feat: initial");
    repo.add_commit("fix: second");
    run_git(repo.path(), &["branch", "-M", "main"]);
    run_git(repo.path(), &["checkout", "-q", "-b", "behind", "HEAD~1"]);

    Command::new(TestRepo::bin_path())
        .args(["lint", "--range", "main..HEAD"])
        .current_dir(repo.path())
        .assert()
        .code(0)
        .stderr_eq(file![
            "../snapshots/check/range_behind_base.stderr.expected"
        ]);
}

/// An omitted right endpoint is `HEAD`, so `<base>..` is the gate form too and
/// must not produce a hint naming the unreadable inverse `..<base>` (#545).
#[test]
fn lint_range_omitted_endpoint_behind_base_exits_0() {
    let mut repo = TestRepo::new();
    repo.add_commit("feat: initial");
    repo.add_commit("fix: second");
    run_git(repo.path(), &["branch", "-M", "main"]);
    run_git(repo.path(), &["checkout", "-q", "-b", "behind", "HEAD~1"]);

    Command::new(TestRepo::bin_path())
        .args(["lint", "--range", "main.."])
        .current_dir(repo.path())
        .assert()
        .code(0)
        .stderr_eq(file![
            "../snapshots/check/range_omitted_endpoint_behind.stderr.expected"
        ]);
}

/// A malformed range is a usage error, the second exit-2 case named in #545.
#[test]
fn lint_range_malformed_exits_2() {
    let mut repo = TestRepo::new();
    repo.add_commit("feat: initial");

    Command::new(TestRepo::bin_path())
        .args(["lint", "--range", "not..a..range"])
        .current_dir(repo.path())
        .assert()
        .code(2);
}

/// A reversed range in JSON mode reports the verdict through the exit code and
/// keeps stdout a valid array (#545).
#[test]
fn lint_range_reversed_json_emits_array_and_warns() {
    let mut repo = TestRepo::new();
    repo.add_commit("feat: initial");
    repo.add_commit("fix: second");

    Command::new(TestRepo::bin_path())
        .args(["lint", "--range", "HEAD..HEAD~1", "--format", "json"])
        .current_dir(repo.path())
        .assert()
        .code(1)
        .stdout_eq(file!["../snapshots/check/range_empty_json.stdout.expected"])
        .stderr_eq(file!["../snapshots/check/range_reversed.stderr.expected"]);
}

/// An annotated tag resolves to a tag object, not a commit, so the checkout
/// exemption must peel both sides before comparing them. `git std bump` creates
/// annotated tags, so this is every release tag in a git-std repository (#545).
#[test]
fn lint_range_annotated_tag_at_head_exits_0() {
    let mut repo = TestRepo::new();
    repo.add_commit("feat: initial");
    run_git(repo.path(), &["branch", "-M", "main"]);
    run_git(repo.path(), &["tag", "-a", "v1.0.0", "-m", "release"]);
    repo.add_commit("fix: second");
    run_git(repo.path(), &["checkout", "-q", "v1.0.0"]);

    Command::new(TestRepo::bin_path())
        .args(["lint", "--range", "main..v1.0.0"])
        .current_dir(repo.path())
        .assert()
        .code(0)
        .stderr_eq(file![
            "../snapshots/check/range_annotated_tag_at_head.stderr.expected"
        ]);
}
