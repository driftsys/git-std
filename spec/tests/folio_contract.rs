#[path = "../support/mod.rs"]
mod support;

use serde_json::Value;
use sha2::{Digest, Sha256};
use snapbox::cmd::Command;
use support::TestRepo;

fn run_json(repo: &TestRepo, args: &[&str]) -> (std::process::ExitStatus, Value) {
    let output = Command::new(TestRepo::bin_path())
        .args(args)
        .current_dir(repo.path())
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

fn run_json_with_env(
    repo: &TestRepo,
    args: &[&str],
    key: &str,
    value: &str,
) -> (std::process::ExitStatus, Value) {
    let output = Command::new(TestRepo::bin_path())
        .args(args)
        .env(key, value)
        .current_dir(repo.path())
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

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn bump_fixture() -> TestRepo {
    let mut repo = TestRepo::new().with_config(
        "monorepo = false\n\n[[version_files]]\npath = \"VERSION\"\nregex = '^(\\d+\\.\\d+\\.\\d+)$'\n",
    );
    repo.write(
        "Cargo.toml",
        "[workspace]\nmembers = [\"member\"]\n\n[workspace.package]\nversion = \"1.0.0\"\n",
    );
    repo.write(
        "member/Cargo.toml",
        "[package]\nname = \"member\"\nversion.workspace = true\nedition = \"2021\"\n",
    );
    repo.write("member/src/lib.rs", "pub fn answer() -> u8 { 42 }\n");
    repo.write(
        "package.json",
        "{\"name\":\"fixture\",\"version\":\"1.0.0\"}\n",
    );
    repo.write("project.toml", "name = \"fixture\"\nversion = \"1.0.0\"\n");
    repo.write("VERSION", "1.0.0\n");
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("feat: add feature");
    repo
}

#[test]
fn single_version_polyglot_contract() {
    let repo = bump_fixture();
    let (status, plan) = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);

    assert!(status.success());
    assert_eq!(plan["version"], "1.1.0");
    assert_eq!(plan["version_mismatches"], serde_json::json!([]));
    assert!(
        plan["version_observations"]
            .as_array()
            .is_some_and(|items| {
                items.iter().any(|item| {
                    item["path"] == "member/Cargo.toml"
                        && item["version"] == "1.0.0"
                        && item["source"] == "cargo_workspace"
                })
            })
    );
    let effects = plan["effects"].as_array().expect("effects array");
    assert_eq!(effects.iter().filter(|e| e["kind"] == "commit").count(), 1);
    assert_eq!(effects.iter().filter(|e| e["kind"] == "tag").count(), 1);
    assert_eq!(
        effects.iter().filter(|e| e["kind"] == "changelog").count(),
        1
    );
}

#[test]
fn single_version_reports_source_mismatches() {
    let repo = bump_fixture();
    repo.write(
        "package.json",
        "{\"name\":\"fixture\",\"version\":\"0.9.0\"}\n",
    );

    let (status, plan) = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);

    assert!(status.success());
    let mismatches = plan["version_mismatches"]
        .as_array()
        .expect("mismatch array");
    assert!(mismatches.iter().any(|mismatch| {
        mismatch["path"] == "package.json"
            && mismatch["observed"] == "0.9.0"
            && mismatch["canonical"] == "1.0.0"
    }));
}

#[test]
fn single_version_reports_pinned_cargo_member_mismatches() {
    let repo = bump_fixture();
    repo.write(
        "member/Cargo.toml",
        "[package]\nname = \"member\"\nversion = \"0.9.0\"\nedition = \"2021\"\n",
    );

    let (status, plan) = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);

    assert!(status.success());
    assert!(plan["version_mismatches"].as_array().is_some_and(|items| {
        items.iter().any(|item| {
            item["path"] == "member/Cargo.toml"
                && item["observed"] == "0.9.0"
                && item["canonical"] == "1.0.0"
        })
    }));
}

#[test]
fn unchanged_plan_can_be_applied() {
    let repo = bump_fixture();
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

    assert!(status.success());
    assert_eq!(result["plan_id"], plan_id);
    assert_eq!(result["status"], "applied");
}

#[test]
fn changed_planned_file_rejects_apply_without_mutation() {
    let repo = bump_fixture();
    let (_, plan) = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    let plan_id = plan["plan_id"].as_str().expect("plan id");
    repo.write(
        "package.json",
        "{\"name\":\"fixture\",\"version\":\"1.0.0\",\"changed\":true}\n",
    );
    let before = repo.snapshot_state();

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

    assert_eq!(status.code(), Some(1));
    assert_eq!(
        result["diagnostics"][0]["code"],
        "GITSTD-BUMP-PLAN-DIVERGED"
    );
    assert_eq!(repo.snapshot_state(), before);
}

#[test]
fn identical_inputs_produce_identical_plan_ids() {
    let repo = bump_fixture();
    let (_, first) = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    let (_, second) = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    assert_eq!(first["plan_id"], second["plan_id"]);
}

#[test]
fn exact_effects_include_predicted_hashes() {
    let repo = bump_fixture();
    let (_, plan) = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    let effects = plan["effects"].as_array().expect("effects array");

    let version = effects
        .iter()
        .find(|effect| effect["target"] == "project.toml" && effect["fidelity"] == "exact")
        .expect("exact project.toml effect");
    assert_eq!(
        version["after_sha256"],
        sha256(b"name = \"fixture\"\nversion = \"1.1.0\"\n")
    );

    let exact_hashes = effects
        .iter()
        .filter(|effect| effect["fidelity"] == "exact")
        .map(|effect| {
            (
                effect["target"]
                    .as_str()
                    .expect("effect target")
                    .to_string(),
                effect["after_sha256"]
                    .as_str()
                    .expect("exact after hash")
                    .to_string(),
            )
        })
        .collect::<Vec<_>>();
    let plan_id = plan["plan_id"].as_str().expect("plan id");
    let (status, _) = run_json(
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
    assert!(status.success());
    for (target, expected) in exact_hashes {
        let actual = std::fs::read(repo.path().join(&target))
            .unwrap_or_else(|error| panic!("cannot read {target}: {error}"));
        assert_eq!(
            sha256(&actual),
            expected,
            "wrong applied bytes for {target}"
        );
    }
    assert!(
        effects
            .iter()
            .filter(|effect| effect["kind"] == "lock_sync")
            .all(|effect| effect.get("after_sha256").is_none()
                && effect["fidelity"] == "conditional")
    );
}

#[test]
fn skipped_commit_tag_and_push_are_not_declared_as_effects() {
    let repo = bump_fixture();
    let (_, plan) = run_json(
        &repo,
        &[
            "bump",
            "--dry-run",
            "--format",
            "json",
            "--no-commit",
            "--push",
        ],
    );
    let effects = plan["effects"].as_array().expect("effects array");

    for kind in ["commit", "tag", "push"] {
        assert!(
            effects.iter().all(|effect| effect["kind"] != kind),
            "unexpected {kind} effect"
        );
    }
}

#[test]
fn requested_push_is_an_explicit_conditional_effect() {
    let repo = bump_fixture();
    let (_, plan) = run_json(&repo, &["bump", "--dry-run", "--format", "json", "--push"]);
    let push = plan["effects"]
        .as_array()
        .expect("effects array")
        .iter()
        .find(|effect| effect["kind"] == "push")
        .expect("push effect");

    assert_eq!(push["target"], "origin");
    assert_eq!(push["fidelity"], "conditional");
    assert_eq!(plan["fidelity"]["exact"], false);
}

#[test]
fn changed_head_rejects_apply_without_additional_mutation() {
    let mut repo = bump_fixture();
    let (_, plan) = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    let plan_id = plan["plan_id"].as_str().expect("plan id");
    repo.add_commit("fix: change plan inputs");
    assert_diverged_without_mutation(&repo, plan_id);
}

#[test]
fn changed_config_rejects_apply_without_mutation() {
    let repo = bump_fixture();
    let (_, plan) = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    let plan_id = plan["plan_id"].as_str().expect("plan id");
    let updated = format!("{}\n# changed after planning\n", repo.read(".git-std.toml"));
    repo.write(".git-std.toml", &updated);
    assert_diverged_without_mutation(&repo, plan_id);
}

#[test]
fn changed_lifecycle_hook_rejects_apply_without_mutation() {
    let repo = bump_fixture();
    repo.write(".githooks/pre-bump.hooks", "true\n");
    let (_, plan) = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    let plan_id = plan["plan_id"].as_str().expect("plan id");
    repo.write(".githooks/pre-bump.hooks", "true\n# changed\n");
    assert_diverged_without_mutation(&repo, plan_id);
}

#[test]
fn changed_hook_skip_environment_rejects_apply_without_mutation() {
    let repo = bump_fixture();
    repo.write(".githooks/pre-bump.hooks", "true\n");
    let (_, plan) = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    let plan_id = plan["plan_id"].as_str().expect("plan id");
    let before = repo.snapshot_state();

    let (status, result) = run_json_with_env(
        &repo,
        &[
            "bump",
            "--expect-plan",
            plan_id,
            "--format",
            "json",
            "--yes",
        ],
        "GIT_STD_SKIP_HOOKS",
        "1",
    );

    assert_eq!(status.code(), Some(1));
    assert_eq!(
        result["diagnostics"][0]["code"],
        "GITSTD-BUMP-PLAN-DIVERGED"
    );
    assert_eq!(repo.snapshot_state(), before);
}

#[test]
fn hook_skip_environment_removes_hook_effects_from_the_plan() {
    let repo = bump_fixture();
    let hook_names = ["pre-bump", "post-version", "post-changelog", "post-bump"];
    for hook in hook_names {
        repo.write(&format!(".githooks/{hook}.hooks"), "true\n");
    }
    let (_, unskipped) = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    assert_eq!(
        unskipped["effects"]
            .as_array()
            .expect("effects array")
            .iter()
            .filter(|effect| effect["kind"] == "hook")
            .map(|effect| {
                assert_eq!(effect["fidelity"], "opaque");
                effect["target"].as_str().expect("hook target")
            })
            .collect::<Vec<_>>(),
        hook_names
    );

    let (status, plan) = run_json_with_env(
        &repo,
        &["bump", "--dry-run", "--format", "json"],
        "GIT_STD_SKIP_HOOKS",
        "true",
    );

    assert!(status.success());
    assert_ne!(plan["plan_id"], unskipped["plan_id"]);
    assert_eq!(plan["inputs"]["options"]["skip_hooks"], true);
    assert!(
        plan["effects"]
            .as_array()
            .is_some_and(|effects| effects.iter().all(|effect| effect["kind"] != "hook"))
    );
}

#[test]
fn false_hook_skip_environment_values_preserve_guarded_hook_execution() {
    for value in ["0", "false"] {
        let repo = bump_fixture();
        repo.write(".githooks/pre-bump.hooks", "touch hook-ran\n");
        let (plan_status, plan) = run_json_with_env(
            &repo,
            &["bump", "--dry-run", "--format", "json"],
            "GIT_STD_SKIP_HOOKS",
            value,
        );
        assert!(plan_status.success(), "environment value {value}");
        assert_eq!(plan["inputs"]["options"]["skip_hooks"], false);
        assert!(plan["effects"].as_array().is_some_and(|effects| {
            effects
                .iter()
                .any(|effect| effect["kind"] == "hook" && effect["target"] == "pre-bump")
        }));

        let plan_id = plan["plan_id"].as_str().expect("plan id");
        let (apply_status, apply) = run_json_with_env(
            &repo,
            &[
                "bump",
                "--expect-plan",
                plan_id,
                "--format",
                "json",
                "--yes",
            ],
            "GIT_STD_SKIP_HOOKS",
            value,
        );
        assert!(apply_status.success(), "{value}: {apply}");
        assert_eq!(apply["plan_id"], plan["plan_id"]);
        assert!(
            repo.path().join("hook-ran").exists(),
            "environment value {value}"
        );
    }
}

#[test]
fn signed_commit_and_tag_effects_are_conditional() {
    let repo = bump_fixture();

    let (status, plan) = run_json(&repo, &["bump", "--dry-run", "--format", "json", "--sign"]);

    assert!(status.success(), "{plan}");
    for kind in ["commit", "tag"] {
        let effect = plan["effects"]
            .as_array()
            .expect("effects")
            .iter()
            .find(|effect| effect["kind"] == kind)
            .unwrap_or_else(|| panic!("missing {kind} effect"));
        assert_eq!(effect["fidelity"], "conditional", "{kind}: {effect}");
        assert!(
            effect["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("signing")),
            "{kind}: {effect}"
        );
    }
    assert_eq!(plan["fidelity"]["exact"], false);
}

#[test]
fn apply_reports_runtime_commit_and_tag_identifiers() {
    let repo = bump_fixture();
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

    assert!(status.success());
    let commit_oid = result["commit_oid"].as_str().expect("commit oid");
    let tag_oid = result["tag_oid"].as_str().expect("tag oid");
    assert_eq!(commit_oid, repo.git(&["rev-parse", "HEAD"]));
    assert_eq!(tag_oid, repo.git(&["rev-parse", "refs/tags/v1.1.0"]));
    assert_eq!(commit_oid, repo.git(&["rev-parse", "refs/tags/v1.1.0^{}"]));
}

fn assert_diverged_without_mutation(repo: &TestRepo, plan_id: &str) {
    let before = repo.snapshot_state();
    let (status, result) = run_json(
        repo,
        &[
            "bump",
            "--expect-plan",
            plan_id,
            "--format",
            "json",
            "--yes",
        ],
    );
    assert_eq!(status.code(), Some(1));
    assert_eq!(
        result["diagnostics"][0]["code"],
        "GITSTD-BUMP-PLAN-DIVERGED"
    );
    assert_eq!(repo.snapshot_state(), before);
}
