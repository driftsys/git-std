#[path = "../support/mod.rs"]
mod support;

use serde_json::Value;
use snapbox::cmd::Command;
use support::TestRepo;

fn json_stdout(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout must be exactly one JSON document: {error}; stdout={}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

#[test]
fn version_json_matches_v1_contract_shape() {
    let mut repo = TestRepo::new();
    repo.add_commit("chore: init");
    repo.create_tag("v1.2.3");

    let output = Command::new(TestRepo::bin_path())
        .args(["version", "--format", "json"])
        .current_dir(repo.path())
        .output()
        .expect("version command");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let document = json_stdout(&output);
    assert_eq!(document["schema_version"], "1.0.0");
    let tool_version = document["tool_version"].as_str().expect("tool version");
    assert!(semver_like(tool_version));
    assert_eq!(document["status"], "success");
    assert_eq!(document["version"], "1.2.3");
}

fn semver_like(version: &str) -> bool {
    let parts: Vec<_> = version.split('.').collect();
    parts.len() == 3 && parts.iter().all(|part| part.parse::<u64>().is_ok())
}

#[test]
fn bump_plan_json_contains_contract_and_effects() {
    let mut repo = TestRepo::new().with_cargo_toml("1.0.0");
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("feat: add contract");

    let output = Command::new(TestRepo::bin_path())
        .args(["bump", "--dry-run", "--format", "json"])
        .current_dir(repo.path())
        .output()
        .expect("bump command");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let document = json_stdout(&output);
    assert_eq!(document["schema_version"], "1.0.0");
    assert_eq!(document["status"], "planned");
    assert!(
        document["plan_id"]
            .as_str()
            .is_some_and(|value| value.starts_with("sha256:"))
    );
    assert!(document["inputs"].is_object());
    assert!(
        document["effects"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
    );
    assert!(document["fidelity"].is_object());
}

#[test]
fn bump_json_failure_is_parseable() {
    let repo = TestRepo::new();

    let output = Command::new(TestRepo::bin_path())
        .args(["bump", "--release-as", "not-a-version", "--format", "json"])
        .current_dir(repo.path())
        .output()
        .expect("bump command");

    assert!(!output.status.success());
    let document = json_stdout(&output);
    assert_eq!(document["status"], "error");
    assert!(document["diagnostics"][0]["code"].is_string());
}

#[test]
fn machine_failures_repeat_the_process_exit_code() {
    let repo = TestRepo::new();
    let output = Command::new(TestRepo::bin_path())
        .args(["version", "--format", "json"])
        .current_dir(repo.path())
        .output()
        .expect("version command");

    assert_eq!(output.status.code(), Some(2));
    let document = json_stdout(&output);
    assert_eq!(document["status"], "error");
    assert_eq!(document["exit_code"], 2);
}

#[test]
fn bump_json_noop_is_a_structured_skipped_result() {
    let mut repo = TestRepo::new().with_cargo_toml("1.0.0");
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");

    let output = Command::new(TestRepo::bin_path())
        .args(["bump", "--dry-run", "--format", "json"])
        .current_dir(repo.path())
        .output()
        .expect("bump command");

    assert!(output.status.success());
    assert_eq!(json_stdout(&output)["status"], "skipped");
}

#[test]
fn bump_json_captures_hook_stdout_and_structures_hook_failure() {
    let mut repo = TestRepo::new().with_cargo_toml("1.0.0");
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("fix: next release");
    repo.write(".githooks/pre-bump.hooks", "echo hook-output\n");

    let success = Command::new(TestRepo::bin_path())
        .args(["bump", "--format", "json", "--yes", "--no-commit"])
        .current_dir(repo.path())
        .output()
        .expect("bump command");
    assert!(success.status.success());
    assert_eq!(json_stdout(&success)["status"], "applied");
    assert!(!String::from_utf8_lossy(&success.stdout).contains("hook-output"));

    repo.write(".githooks/pre-bump.hooks", "false\n");
    let failure = Command::new(TestRepo::bin_path())
        .args([
            "bump",
            "--format",
            "json",
            "--yes",
            "--no-commit",
            "--release-as",
            "patch",
        ])
        .current_dir(repo.path())
        .output()
        .expect("bump command");
    assert_eq!(failure.status.code(), Some(2));
    let document = json_stdout(&failure);
    assert_eq!(document["status"], "error");
    assert_eq!(document["diagnostics"][0]["code"], "GITSTD-LIFECYCLE-HOOK");
}

#[test]
fn hook_run_json_is_one_document_and_executes_the_child() {
    let repo = TestRepo::new();
    repo.write(
        ".githooks/pre-commit.hooks",
        "printf 'hook-output\\n'; touch hook-ran\n",
    );

    let output = Command::new(TestRepo::bin_path())
        .args(["hook", "run", "pre-commit", "--format", "json"])
        .current_dir(repo.path())
        .output()
        .expect("hook command");

    assert!(output.status.success());
    let document = json_stdout(&output);
    assert_eq!(document["status"], "success");
    assert_eq!(output.stdout.first(), Some(&b'{'));
    assert_eq!(document["commands"][0]["exit_code"], 0);
    assert!(repo.path().join("hook-ran").exists());
}

#[test]
fn hook_list_and_run_json_remain_parseable() {
    let repo = TestRepo::new().with_hooks_file("pre-commit", "true\n?false\n");

    for args in [
        &["hook", "list", "--format", "json"][..],
        &["hook", "run", "pre-commit", "--format", "json"][..],
    ] {
        let output = Command::new(TestRepo::bin_path())
            .args(args)
            .current_dir(repo.path())
            .output()
            .expect("hook command");
        assert!(output.status.success());
        let _ = json_stdout(&output);
    }
}

#[test]
fn doctor_json_failure_is_parseable() {
    let repo = TestRepo::new().with_config("[[invalid toml = bad\n");

    let output = Command::new(TestRepo::bin_path())
        .args(["doctor", "--format", "json"])
        .current_dir(repo.path())
        .output()
        .expect("doctor command");

    assert!(!output.status.success());
    let document = json_stdout(&output);
    assert_eq!(document["status"], "fail");
    assert!(document["diagnostics"][0]["code"].is_string());
}

#[test]
fn json_usage_failure_keeps_stdout_parseable() {
    let repo = TestRepo::new();
    let output = Command::new(TestRepo::bin_path())
        .args(["bump", "--format", "json", "--not-a-real-option"])
        .current_dir(repo.path())
        .output()
        .expect("bump command");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        !output.stderr.is_empty(),
        "clap explanation belongs on stderr"
    );
    let document = json_stdout(&output);
    assert_eq!(document["status"], "error");
    assert_eq!(
        document["diagnostics"][0]["code"],
        "GITSTD-INVALID-ARGUMENT"
    );
}
