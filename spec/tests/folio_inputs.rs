#[path = "../support/mod.rs"]
mod support;

use serde_json::Value;
use snapbox::cmd::Command;
use support::TestRepo;

fn run_json(repo: &TestRepo, args: &[&str]) -> Value {
    let output = Command::new(TestRepo::bin_path())
        .args(args)
        .current_dir(repo.path())
        .output()
        .expect("git-std command");
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout must be JSON: {error}; stdout={}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn run_json_with_path(repo: &TestRepo, path: &str) -> Value {
    let output = Command::new(TestRepo::bin_path())
        .args(["bump", "--dry-run", "--format", "json"])
        .env("PATH", path)
        .current_dir(repo.path())
        .output()
        .expect("git-std command");
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).expect("JSON plan")
}

fn bump_fixture() -> TestRepo {
    let mut repo = TestRepo::new().with_config("monorepo = false\n");
    repo.write(
        "package.json",
        "{\"name\":\"fixture\",\"version\":\"1.0.0\"}\n",
    );
    repo.write("package-lock.json", "{\"lockfileVersion\":3}\n");
    repo.add_commit("chore: init");
    repo.create_tag("v1.0.0");
    repo.add_commit("feat: add feature");
    repo
}

#[test]
fn plan_identity_exposes_and_binds_documented_inputs() {
    let repo = bump_fixture();
    let initial = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    let inputs = initial["inputs"].as_object().expect("inputs object");
    let mut keys = inputs.keys().map(String::as_str).collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "branch",
            "config_sha256",
            "effective_date",
            "files",
            "head",
            "options",
            "path_sha256",
            "remotes_sha256",
            "tags_sha256",
            "tools",
        ]
    );
    let options = inputs["options"].as_object().expect("options object");
    let mut option_keys = options.keys().map(String::as_str).collect::<Vec<_>>();
    option_keys.sort_unstable();
    assert_eq!(
        option_keys,
        [
            "first_major_release",
            "first_release",
            "force",
            "minor",
            "no_commit",
            "no_tag",
            "packages",
            "prerelease",
            "push",
            "release_as",
            "sign",
            "skip_changelog",
            "skip_hooks",
            "stable",
            "version",
        ]
    );
    assert_eq!(options["version"], "1.1.0");
    assert_eq!(options["prerelease"], Value::Null);
    assert_eq!(options["release_as"], Value::Null);
    assert_eq!(options["first_release"], false);
    assert_eq!(options["first_major_release"], false);
    assert_eq!(options["no_tag"], false);
    assert_eq!(options["no_commit"], false);
    assert_eq!(options["skip_changelog"], false);
    assert_eq!(options["sign"], false);
    assert_eq!(options["force"], false);
    assert_eq!(options["stable"], Value::Null);
    assert_eq!(options["minor"], false);
    assert_eq!(options["packages"], serde_json::json!([]));
    assert_eq!(options["push"], Value::Null);
    assert_eq!(options["skip_hooks"], false);
    assert_eq!(
        inputs["tools"]
            .as_array()
            .expect("tools array")
            .iter()
            .map(|tool| tool["name"].as_str().expect("tool name"))
            .collect::<Vec<_>>(),
        [
            "cargo", "deno", "flutter", "gradle", "npm", "pnpm", "poetry", "uv", "yarn"
        ]
    );
    assert!(inputs["files"].as_array().is_some_and(|files| {
        files.iter().any(|file| {
            file["path"] == "package-lock.json"
                && file["sha256"].as_str().is_some_and(|hash| hash.len() == 64)
        })
    }));

    repo.create_tag("v0.9.0");
    let tagged = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    assert_ne!(initial["plan_id"], tagged["plan_id"]);

    repo.git(&[
        "remote",
        "add",
        "origin",
        "https://example.invalid/repo.git",
    ]);
    let remoted = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    assert_ne!(tagged["plan_id"], remoted["plan_id"]);

    repo.git(&["checkout", "-q", "-b", "release-test"]);
    let branched = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    assert_ne!(remoted["plan_id"], branched["plan_id"]);

    repo.write(
        "package-lock.json",
        "{\"lockfileVersion\":3,\"changed\":true}\n",
    );
    let locked = run_json(&repo, &["bump", "--dry-run", "--format", "json"]);
    assert_ne!(branched["plan_id"], locked["plan_id"]);

    let changed_path = run_json_with_path(&repo, "/usr/bin:/bin");
    assert_ne!(locked["plan_id"], changed_path["plan_id"]);

    let changed_options = run_json(
        &repo,
        &["bump", "--dry-run", "--format", "json", "--no-tag"],
    );
    assert_ne!(locked["plan_id"], changed_options["plan_id"]);

    let signed = run_json(&repo, &["bump", "--dry-run", "--format", "json", "--sign"]);
    assert_eq!(signed["inputs"]["options"]["sign"], true);
    assert_ne!(locked["plan_id"], signed["plan_id"]);
}

#[cfg(unix)]
#[test]
fn plan_records_the_resolved_tool_path() {
    use std::os::unix::fs::PermissionsExt;

    let repo = bump_fixture();
    let blocked = tempfile::tempdir().expect("non-executable tool directory");
    let blocked_cargo = blocked.path().join("cargo");
    std::fs::write(&blocked_cargo, "not executable\n").expect("non-executable cargo");
    let mut blocked_permissions = std::fs::metadata(&blocked_cargo)
        .expect("non-executable cargo metadata")
        .permissions();
    blocked_permissions.set_mode(0o001);
    std::fs::set_permissions(&blocked_cargo, blocked_permissions)
        .expect("non-executable cargo permissions");
    let tools = tempfile::tempdir().expect("executable tool directory");
    let cargo = tools.path().join("cargo");
    std::fs::write(&cargo, "#!/bin/sh\nexit 0\n").expect("fake cargo");
    let mut permissions = std::fs::metadata(&cargo)
        .expect("fake cargo metadata")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&cargo, permissions).expect("fake cargo permissions");
    let path = format!(
        "{}:{}:/usr/bin:/bin",
        blocked.path().display(),
        tools.path().display()
    );

    let plan = run_json_with_path(&repo, &path);
    let resolutions = plan["inputs"]["tools"].as_array().expect("tools array");
    let canonical_cargo = std::fs::canonicalize(&cargo).expect("canonical fake cargo path");
    assert!(resolutions.iter().any(|tool| {
        tool["name"] == "cargo" && tool["path"] == canonical_cargo.to_string_lossy().as_ref()
    }));
    assert!(
        resolutions
            .iter()
            .any(|tool| tool["name"] == "deno" && tool["path"].is_null())
    );
}
