#[path = "../support/mod.rs"]
mod support;

use serde_json::Value;
use sha2::{Digest, Sha256};
use snapbox::cmd::Command;
use support::TestRepo;

fn run_json(repo: &TestRepo, args: &[&str]) -> (std::process::ExitStatus, Value) {
    run_json_from(repo.path(), args)
}

fn run_json_from(directory: &std::path::Path, args: &[&str]) -> (std::process::ExitStatus, Value) {
    let output = Command::new(TestRepo::bin_path())
        .args(args)
        .current_dir(directory)
        .output()
        .expect("git-std command");
    let document = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout must be JSON: {error}; stdout={}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    (output.status, document)
}

#[test]
fn guarded_release_commits_root_paths_from_a_subdirectory() {
    let repo = simple_fixture();
    let nested = repo.path().join("nested/directory");
    std::fs::create_dir_all(&nested).expect("nested working directory");
    let (_, plan) = run_json_from(&nested, &["bump", "--dry-run", "--format", "json"]);
    let plan_id = plan["plan_id"].as_str().expect("plan id");

    let (status, result) = run_json_from(
        &nested,
        &[
            "bump",
            "--expect-plan",
            plan_id,
            "--format",
            "json",
            "--yes",
        ],
    );

    assert!(status.success(), "{result}");
    assert!(repo.read("Cargo.toml").contains("version = \"1.1.0\""));
    assert!(
        repo.git(&["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"])
            .lines()
            .any(|path| path == "Cargo.toml")
    );
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn simple_fixture() -> TestRepo {
    let mut repo = TestRepo::new().with_config("monorepo = false\n");
    repo.write(
        "Cargo.toml",
        "[package]\nname = \"fixture\"\nversion = \"1.0.0\"\n",
    );
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("feat: add feature");
    repo
}

#[test]
fn release_commit_excludes_unrelated_staged_files() {
    let repo = simple_fixture();
    let (_, plan) = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    let plan_id = plan["plan_id"].as_str().expect("plan id");
    repo.write("unrelated.txt", "keep staged\n");
    repo.git(&["add", "unrelated.txt"]);

    let (status, result) = run_json(
        &repo,
        &[
            "bump",
            "--expect-plan",
            plan_id,
            "--format",
            "json",
            "--yes",
        ],
    );

    assert!(status.success(), "{result}");
    let committed = repo.git(&["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]);
    assert!(!committed.lines().any(|path| path == "unrelated.txt"));
    assert_eq!(
        repo.git(&["diff", "--cached", "--name-only"]),
        "unrelated.txt"
    );
}

#[test]
fn cargo_workspace_exact_hash_matches_all_apply_rewrites() {
    let mut repo = TestRepo::new().with_config("monorepo = false\n");
    repo.write(
        "Cargo.toml",
        "[workspace]\nmembers = [\"member\"]\n\n[workspace.package]\nversion = \"1.0.0\"\n\n[workspace.dependencies]\nmember = { version = \"1.0.0\", path = \"member\" }\n",
    );
    repo.write(
        "member/Cargo.toml",
        "[package]\nname = \"member\"\nversion.workspace = true\n",
    );
    repo.write("member/src/lib.rs", "pub fn value() -> u8 { 1 }\n");
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("feat: add feature");

    let (_, plan) = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    let cargo_effect = plan["effects"]
        .as_array()
        .expect("effects")
        .iter()
        .find(|effect| effect["target"] == "Cargo.toml")
        .expect("Cargo.toml effect");
    assert_eq!(cargo_effect["fidelity"], "exact");
    let expected = cargo_effect["after_sha256"]
        .as_str()
        .expect("predicted Cargo.toml hash");
    let plan_id = plan["plan_id"].as_str().expect("plan id");

    let (status, result) = run_json(
        &repo,
        &[
            "bump",
            "--expect-plan",
            plan_id,
            "--format",
            "json",
            "--yes",
        ],
    );

    assert!(status.success(), "{result}");
    assert_eq!(sha256(repo.read("Cargo.toml").as_bytes()), expected);
    assert!(
        repo.read("Cargo.toml")
            .contains("version = \"1.1.0\", path")
    );
}

#[test]
fn publish_false_workspace_member_is_not_a_planned_or_applied_effect() {
    let mut repo = TestRepo::new().with_config("monorepo = false\n");
    repo.write(
        "Cargo.toml",
        "[workspace]\nmembers = [\"internal\"]\n\n[workspace.package]\nversion = \"1.0.0\"\n",
    );
    repo.write(
        "internal/Cargo.toml",
        "[package]\nname = \"internal\"\nversion = \"7.4.2\"\npublish = false\n",
    );
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("feat: add feature");

    let (_, plan) = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    assert!(plan["effects"].as_array().is_some_and(|effects| {
        effects
            .iter()
            .all(|effect| effect["target"] != "internal/Cargo.toml")
    }));
    let plan_id = plan["plan_id"].as_str().expect("plan id");

    let (status, result) = run_json(
        &repo,
        &[
            "bump",
            "--expect-plan",
            plan_id,
            "--format",
            "json",
            "--yes",
        ],
    );

    assert!(status.success(), "{result}");
    assert!(
        repo.read("internal/Cargo.toml")
            .contains("version = \"7.4.2\"")
    );
}

#[test]
fn workspace_dependency_only_root_is_an_exact_planned_and_applied_effect() {
    let mut repo = TestRepo::new().with_config("monorepo = false\n");
    repo.write(
        "Cargo.toml",
        "[workspace]\nmembers = [\"member\"]\n\n[workspace.dependencies]\nmember = { version = \"1.0.0\", path = \"member\" }\n",
    );
    repo.write(
        "member/Cargo.toml",
        "[package]\nname = \"member\"\nversion.workspace = true\n",
    );
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("feat: add feature");

    let (_, plan) = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    let cargo_effect = plan["effects"]
        .as_array()
        .expect("effects")
        .iter()
        .find(|effect| effect["target"] == "Cargo.toml")
        .expect("root dependency effect");
    assert_eq!(cargo_effect["fidelity"], "exact");
    let expected = cargo_effect["after_sha256"]
        .as_str()
        .expect("predicted root hash");
    let plan_id = plan["plan_id"].as_str().expect("plan id");

    let (status, result) = run_json(
        &repo,
        &[
            "bump",
            "--expect-plan",
            plan_id,
            "--format",
            "json",
            "--yes",
        ],
    );

    assert!(status.success(), "{result}");
    assert_eq!(sha256(repo.read("Cargo.toml").as_bytes()), expected);
    assert!(repo.read("Cargo.toml").contains("version = \"1.1.0\""));
}

#[test]
fn release_without_changelog_content_declares_no_changelog_effect() {
    let mut repo = TestRepo::new().with_config("monorepo = false\n");
    repo.write(
        "Cargo.toml",
        "[package]\nname = \"fixture\"\nversion = \"1.0.0\"\n",
    );
    repo.write(
        ".githooks/post-changelog.hooks",
        "touch post-changelog-ran\n",
    );
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("chore: maintenance only");

    let (_, plan) = run_json(
        &repo,
        &[
            "bump",
            "--release-as",
            "1.0.1",
            "--dry-run",
            "--format",
            "json",
        ],
    );
    assert!(
        plan["effects"]
            .as_array()
            .expect("effects")
            .iter()
            .all(|effect| effect["kind"] != "changelog" && effect["target"] != "post-changelog")
    );
    assert_eq!(plan["changelog"], false);
    let plan_id = plan["plan_id"].as_str().expect("plan id");

    let (status, result) = run_json(
        &repo,
        &[
            "bump",
            "--release-as",
            "1.0.1",
            "--expect-plan",
            plan_id,
            "--format",
            "json",
            "--yes",
        ],
    );

    assert!(status.success(), "{result}");
    assert_eq!(result["changelog"], false);
    assert!(!repo.path().join("CHANGELOG.md").exists());
    assert!(!repo.path().join("post-changelog-ran").exists());
}

#[test]
fn workspace_dependency_root_and_pinned_member_are_both_committed() {
    let mut repo = TestRepo::new().with_config("monorepo = false\n");
    repo.write(
        "Cargo.toml",
        "[workspace]\nmembers = [\"member\"]\n\n[workspace.dependencies]\nmember = { version = \"1.0.0\", path = \"member\" }\n",
    );
    repo.write(
        "member/Cargo.toml",
        "[package]\nname = \"member\"\nversion = \"1.0.0\"\n",
    );
    repo.write("member/src/lib.rs", "pub fn value() -> u8 { 1 }\n");
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("feat: add feature");

    let (_, plan) = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    let plan_id = plan["plan_id"].as_str().expect("plan id");
    let (status, result) = run_json(
        &repo,
        &[
            "bump",
            "--expect-plan",
            plan_id,
            "--format",
            "json",
            "--yes",
        ],
    );

    assert!(status.success(), "{result}");
    let committed = repo.git(&["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]);
    assert!(committed.lines().any(|path| path == "Cargo.toml"));
    assert!(committed.lines().any(|path| path == "member/Cargo.toml"));
    assert!(
        repo.git(&["show", "HEAD:Cargo.toml"])
            .contains("version = \"1.1.0\"")
    );
    assert!(
        repo.git(&["show", "HEAD:member/Cargo.toml"])
            .contains("version = \"1.1.0\"")
    );
}

#[test]
fn generated_changelog_is_included_in_the_release_commit() {
    let repo = simple_fixture();
    let (_, plan) = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    let plan_id = plan["plan_id"].as_str().expect("plan id");

    let (status, result) = run_json(
        &repo,
        &[
            "bump",
            "--expect-plan",
            plan_id,
            "--format",
            "json",
            "--yes",
        ],
    );

    assert!(status.success(), "{result}");
    assert!(
        repo.git(&["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"])
            .lines()
            .any(|path| path == "CHANGELOG.md")
    );
}
