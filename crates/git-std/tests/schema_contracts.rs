use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const CONTRACTS: &[&str] = &[
    "version",
    "bump",
    "lint",
    "registry",
    "hook-list",
    "hook-run",
    "doctor",
];

fn schema_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/v1/cli")
}

fn validator(name: &str) -> jsonschema::Validator {
    let schema_path = schema_root().join(format!("{name}.schema.json"));
    let schema: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&schema_path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", schema_path.display())),
    )
    .expect("schema JSON");
    jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&schema)
        .unwrap_or_else(|error| panic!("invalid {}: {error}", schema_path.display()))
}

fn assert_output_valid(name: &str, output: &Output) {
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "{name} stdout is not JSON: {error}; stdout={}",
                String::from_utf8_lossy(&output.stdout)
            )
        });
    let validator = validator(name);
    assert!(
        validator.is_valid(&document),
        "{name} output does not validate: {:?}",
        validator.iter_errors(&document).collect::<Vec<_>>()
    );
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("git command");
    assert!(output.status.success(), "git {args:?} failed");
}

fn fixture() -> tempfile::TempDir {
    let fixture = tempfile::tempdir().expect("fixture directory");
    git(fixture.path(), &["init", "-q", "-b", "main"]);
    git(fixture.path(), &["config", "user.name", "Test"]);
    git(
        fixture.path(),
        &["config", "user.email", "test@example.com"],
    );
    std::fs::write(
        fixture.path().join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"1.0.0\"\n",
    )
    .expect("Cargo.toml");
    std::fs::create_dir(fixture.path().join(".githooks")).expect("hooks directory");
    std::fs::write(fixture.path().join(".githooks/pre-commit.hooks"), "true\n").expect("hook file");
    git(fixture.path(), &["add", "."]);
    git(fixture.path(), &["commit", "-q", "-m", "chore: init"]);
    git(fixture.path(), &["tag", "v1.0.0"]);
    std::fs::write(fixture.path().join("feature"), "added\n").expect("feature file");
    git(fixture.path(), &["add", "."]);
    git(
        fixture.path(),
        &["commit", "-q", "-m", "feat: add contract"],
    );
    fixture
}

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_git-std"))
        .args(args)
        .current_dir(root)
        .output()
        .expect("git-std command")
}

#[test]
fn every_cli_example_validates_against_its_v1_schema() {
    for name in CONTRACTS {
        let schema_path = schema_root().join(format!("{name}.schema.json"));
        let example_path = schema_root().join("examples").join(format!("{name}.json"));
        let schema: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&schema_path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", schema_path.display())),
        )
        .expect("schema JSON");
        let example: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&example_path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", example_path.display())),
        )
        .expect("example JSON");
        let validator = jsonschema::draft202012::options()
            .should_validate_formats(true)
            .build(&schema)
            .unwrap_or_else(|error| panic!("invalid {}: {error}", schema_path.display()));
        assert!(
            validator.is_valid(&example),
            "{} does not validate against {}: {:?}",
            example_path.display(),
            schema_path.display(),
            validator.iter_errors(&example).collect::<Vec<_>>()
        );
    }
}

#[test]
fn bump_schema_rejects_malformed_plan_inputs_and_effects() {
    let example_path = schema_root().join("examples/bump.json");
    let example: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&example_path).expect("bump example"))
            .expect("bump example JSON");
    let validator = validator("bump");
    assert!(validator.is_valid(&example), "baseline bump example");

    let mut cases = Vec::new();

    for field in [
        "plan_id",
        "inputs",
        "effects",
        "fidelity",
        "version",
        "version_observations",
        "version_mismatches",
    ] {
        let mut missing_plan_evidence = example.clone();
        missing_plan_evidence
            .as_object_mut()
            .expect("bump result")
            .remove(field);
        cases.push((field, missing_plan_evidence));
    }

    let mut missing_input = example.clone();
    missing_input["inputs"]
        .as_object_mut()
        .expect("inputs")
        .remove("head");
    cases.push(("missing plan input", missing_input));

    for field in [
        "head",
        "tags_sha256",
        "remotes_sha256",
        "config_sha256",
        "path_sha256",
    ] {
        let mut malformed_hash = example.clone();
        malformed_hash["inputs"][field] = serde_json::json!("not-a-digest");
        cases.push((field, malformed_hash));
    }

    let option_names = example["inputs"]["options"]
        .as_object()
        .expect("options")
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    for option in &option_names {
        let mut missing_option = example.clone();
        missing_option["inputs"]["options"]
            .as_object_mut()
            .expect("options")
            .remove(option);
        cases.push(("missing normalized option", missing_option));
    }

    let mut mistyped_option = example.clone();
    mistyped_option["inputs"]["options"]["no_tag"] = serde_json::json!("false");
    cases.push(("mistyped normalized option", mistyped_option));

    let mut unknown_option = example.clone();
    unknown_option["inputs"]["options"]["future_behavior"] = serde_json::json!(true);
    cases.push(("unknown normalized option", unknown_option));

    let mut malformed_date = example.clone();
    malformed_date["inputs"]["effective_date"] = serde_json::json!("2026-99-99");
    cases.push(("malformed effective date", malformed_date));

    let mut impossible_date = example.clone();
    impossible_date["inputs"]["effective_date"] = serde_json::json!("2026-02-31");
    cases.push(("impossible effective date", impossible_date));

    let mut relative_tool = example.clone();
    relative_tool["inputs"]["tools"] =
        serde_json::json!([{ "name": "cargo", "path": "bin/cargo" }]);
    cases.push(("relative tool path", relative_tool));

    let mut missing_exact_hash = example.clone();
    missing_exact_hash["effects"][0]
        .as_object_mut()
        .expect("exact effect")
        .remove("after_sha256");
    cases.push(("exact effect without predicted hash", missing_exact_hash));

    let mut malformed_digest = example.clone();
    malformed_digest["inputs"]["files"] =
        serde_json::json!([{ "path": "Cargo.toml", "sha256": "not-a-digest" }]);
    cases.push(("malformed file digest", malformed_digest));

    let mut malformed_effect_hash = example.clone();
    malformed_effect_hash["effects"][0]["after_sha256"] = serde_json::json!("not-a-digest");
    cases.push(("malformed exact effect hash", malformed_effect_hash));

    for (case, document) in cases {
        assert!(
            !validator.is_valid(&document),
            "bump schema accepted {case}: {document}"
        );
    }
}

#[test]
fn schemas_declare_draft_2020_12_and_stable_ids() {
    for name in CONTRACTS {
        let path = schema_root().join(format!("{name}.schema.json"));
        let schema: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display())),
        )
        .expect("schema JSON");
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert_eq!(
            schema["$id"],
            format!("https://driftsys.github.io/git-std/schemas/v1/cli/{name}.schema.json")
        );
    }
}

#[test]
fn real_success_output_validates_against_published_schemas() {
    let fixture = fixture();
    for (name, args) in [
        ("version", &["version", "--format", "json"][..]),
        ("bump", &["bump", "--dry-run", "--format", "json"][..]),
        ("lint", &["lint", "feat: valid", "--format", "json"][..]),
        ("registry", &["registry", "--format", "json"][..]),
        ("hook-list", &["hook", "list", "--format", "json"][..]),
        (
            "hook-run",
            &["hook", "run", "pre-commit", "--format", "json"][..],
        ),
        ("doctor", &["doctor", "--format", "json"][..]),
    ] {
        let output = run(fixture.path(), args);
        assert_ne!(
            output.status.code(),
            Some(2),
            "{name} had an operational failure"
        );
        assert_output_valid(name, &output);
    }
}

#[test]
fn real_operational_failures_validate_against_published_schemas() {
    let fixture = fixture();
    let outside = tempfile::tempdir().expect("outside directory");
    for (name, root, args) in [
        (
            "version",
            outside.path(),
            &["version", "--format", "json"][..],
        ),
        (
            "bump",
            fixture.path(),
            &[
                "bump",
                "--release-as",
                "invalid-version",
                "--format",
                "json",
            ][..],
        ),
        (
            "lint",
            fixture.path(),
            &["lint", "--file", "missing", "--format", "json"][..],
        ),
        (
            "hook-list",
            outside.path(),
            &["hook", "list", "--format", "json"][..],
        ),
        (
            "hook-run",
            outside.path(),
            &["hook", "run", "pre-commit", "--format", "json"][..],
        ),
        (
            "doctor",
            outside.path(),
            &["doctor", "--format", "json"][..],
        ),
    ] {
        let output = run(root, args);
        assert!(!output.status.success(), "{name} unexpectedly succeeded");
        assert_output_valid(name, &output);
    }
}
