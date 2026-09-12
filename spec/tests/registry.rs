#[path = "../support/mod.rs"]
mod support;

use serde_json::Value;
use snapbox::cmd::Command;
use std::collections::BTreeMap;
use support::TestRepo;

fn run(repo: &TestRepo, args: &[&str]) -> (std::process::ExitStatus, Value) {
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

fn assert_sarif_results_registered(registry: &Value, sarif: &Value) {
    let registered = registry["diagnostics"]
        .as_array()
        .expect("registry diagnostics");
    let described = sarif["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .expect("SARIF rules");
    for result in sarif["runs"][0]["results"].as_array().expect("results") {
        let rule_id = result["ruleId"].as_str().expect("result ruleId");
        assert!(registered.iter().any(|entry| entry["code"] == rule_id));
        assert!(described.iter().any(|rule| rule["id"] == rule_id));
    }
}

#[test]
fn registry_exposes_rules_diagnostics_and_effective_convention() {
    let repo = TestRepo::new()
        .with_config("types = [\"feat\", \"fix\"]\nscopes = [\"api\"]\nstrict = true\n");

    let (status, registry) = run(&repo, &["registry", "--format", "json"]);

    assert!(status.success());
    assert_eq!(registry["schema_version"], "1.0.0");
    assert!(registry["tool_version"].is_string());
    assert!(
        registry["rules"]
            .as_array()
            .is_some_and(|rules| !rules.is_empty())
    );
    assert_eq!(registry["facts"]["rules"], "static");
    assert_eq!(registry["facts"]["diagnostics"], "static");
    assert_eq!(
        registry["facts"]["effective_convention"],
        "resolved for current project"
    );
    let registered_codes = registry["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .filter_map(|entry| entry["code"].as_str())
        .collect::<Vec<_>>();
    for code in [
        "GITSTD-VERSION-NO-TAG",
        "GITSTD-BUMP-PLAN",
        "GITSTD-BUMP-PLAN-DIVERGED",
        "GITSTD-DOCTOR-CHECK-FAILED",
        "GITSTD-LINT-EMPTY-RANGE",
    ] {
        assert!(registered_codes.contains(&code), "missing {code}");
    }
    assert!(
        registry["diagnostics"]
            .as_array()
            .is_some_and(|codes| !codes.is_empty())
    );
    assert_eq!(registry["effective_convention"]["strict"], true);
    assert_eq!(
        registry["effective_convention"]["types"],
        serde_json::json!(["feat", "fix"])
    );
}

#[test]
fn registry_reports_default_lint_convention_when_strict_is_disabled() {
    let repo =
        TestRepo::new().with_config("types = [\"feat\"]\nscopes = [\"api\"]\nstrict = false\n");

    let (status, registry) = run(&repo, &["registry", "--format", "json"]);

    assert!(status.success());
    assert_eq!(registry["effective_convention"]["strict"], false);
    assert_eq!(registry["effective_convention"]["types"], Value::Null);
    assert_eq!(registry["effective_convention"]["scopes"], Value::Null);
    assert_eq!(registry["effective_convention"]["require_scope"], false);
}

#[test]
fn registry_resolves_project_configuration_from_a_subdirectory() {
    let repo = TestRepo::new()
        .with_config("types = [\"feat\", \"fix\"]\nscopes = [\"api\"]\nstrict = true\n");
    let nested = repo.path().join("nested/directory");
    std::fs::create_dir_all(&nested).expect("nested directory");

    let output = Command::new(TestRepo::bin_path())
        .args(["registry", "--format", "json"])
        .current_dir(nested)
        .output()
        .expect("registry command");
    let registry: Value = serde_json::from_slice(&output.stdout).expect("registry JSON");

    assert!(output.status.success(), "{registry}");
    assert_eq!(registry["effective_convention"]["strict"], true);
    assert_eq!(
        registry["effective_convention"]["types"],
        serde_json::json!(["feat", "fix"])
    );
    assert_eq!(
        registry["effective_convention"]["scopes"],
        serde_json::json!(["api", "release"])
    );
}

#[test]
fn lint_json_findings_use_registered_codes_and_exit_one() {
    let repo = TestRepo::new();
    let (registry_status, registry) = run(&repo, &["registry", "--format", "json"]);
    assert!(registry_status.success());
    let registered: Vec<&str> = registry["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .filter_map(|entry| entry["code"].as_str())
        .collect();

    let (status, result) = run(&repo, &["lint", "not conventional", "--format", "json"]);

    assert_eq!(status.code(), Some(1));
    assert_eq!(result["status"], "finding");
    assert_eq!(result["valid"], false);
    assert!(result["diagnostics"].as_array().is_some_and(|items| {
        !items.is_empty()
            && items
                .iter()
                .all(|item| registered.contains(&item["code"].as_str().unwrap_or_default()))
    }));
}

#[test]
fn lint_sarif_is_version_2_1_and_uses_exit_classes() {
    let repo = TestRepo::new();
    let (_, registry) = run(&repo, &["registry", "--format", "json"]);

    let (valid_status, valid) = run(&repo, &["lint", "feat: valid message", "--format", "sarif"]);
    assert!(valid_status.success());
    assert_eq!(valid["version"], "2.1.0");
    assert_eq!(valid["runs"][0]["results"], serde_json::json!([]));

    let (finding_status, finding) = run(&repo, &["lint", "not conventional", "--format", "sarif"]);
    assert_eq!(finding_status.code(), Some(1));
    assert!(
        finding["runs"][0]["results"]
            .as_array()
            .is_some_and(|results| !results.is_empty())
    );

    let (error_status, error) = run(
        &repo,
        &["lint", "--file", "missing.txt", "--format", "sarif"],
    );
    assert_eq!(error_status.code(), Some(2));
    assert!(
        error["runs"][0]["invocations"][0]["executionSuccessful"]
            .as_bool()
            .is_some_and(|successful| !successful)
    );
    assert_sarif_results_registered(&registry, &error);
}

#[test]
fn sarif_descriptors_match_the_registered_diagnostics() {
    let repo = TestRepo::new();
    let (_, registry) = run(&repo, &["registry", "--format", "json"]);
    let (_, sarif) = run(&repo, &["lint", "not conventional", "--format", "sarif"]);

    let registered = registry["diagnostics"]
        .as_array()
        .expect("registry diagnostics")
        .iter()
        .map(|definition| {
            (
                definition["code"].as_str().expect("diagnostic code"),
                definition["explanation"]
                    .as_str()
                    .expect("diagnostic explanation"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let described = sarif["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .expect("SARIF rules")
        .iter()
        .map(|descriptor| {
            (
                descriptor["id"].as_str().expect("descriptor id"),
                descriptor["shortDescription"]["text"]
                    .as_str()
                    .expect("descriptor explanation"),
            )
        })
        .collect::<BTreeMap<_, _>>();

    assert_eq!(described, registered);
}

#[test]
fn lint_sarif_usage_failure_is_parseable() {
    let repo = TestRepo::new();
    let (_, registry) = run(&repo, &["registry", "--format", "json"]);
    let output = Command::new(TestRepo::bin_path())
        .args(["lint", "--format", "sarif", "--not-a-real-option"])
        .current_dir(repo.path())
        .output()
        .expect("lint command");

    assert_eq!(output.status.code(), Some(2));
    let document: Value = serde_json::from_slice(&output.stdout).expect("SARIF JSON");
    assert_eq!(document["version"], "2.1.0");
    assert_eq!(
        document["runs"][0]["invocations"][0]["executionSuccessful"],
        false
    );
    assert_sarif_results_registered(&registry, &document);
}

#[test]
fn lint_json_and_sarif_use_the_rule_specific_codes() {
    let repo =
        TestRepo::new().with_config("types = [\"feat\"]\nscopes = [\"api\"]\nstrict = true\n");
    let long_header = format!("feat(api): {}", "x".repeat(100));
    let cases = [
        ("not conventional".to_string(), "GITSTD-COMMIT-PARSE"),
        ("fix(api): wrong type".to_string(), "GITSTD-COMMIT-TYPE"),
        (
            "feat: missing scope".to_string(),
            "GITSTD-COMMIT-SCOPE-REQUIRED",
        ),
        (
            "feat(other): wrong scope".to_string(),
            "GITSTD-COMMIT-SCOPE",
        ),
        (long_header, "GITSTD-COMMIT-HEADER-LENGTH"),
    ];

    for (message, expected) in cases {
        let (json_status, json) = run(&repo, &["lint", &message, "--format", "json"]);
        assert_eq!(json_status.code(), Some(1), "JSON case: {message}");
        assert!(
            json["diagnostics"]
                .as_array()
                .is_some_and(|items| items.len() == 1 && items[0]["code"] == expected),
            "missing {expected} for JSON case: {message}"
        );

        let (sarif_status, sarif) = run(&repo, &["lint", &message, "--format", "sarif"]);
        assert_eq!(sarif_status.code(), Some(1), "SARIF case: {message}");
        assert!(
            sarif["runs"][0]["results"]
                .as_array()
                .is_some_and(|items| items.len() == 1 && items[0]["ruleId"] == expected),
            "missing {expected} for SARIF case: {message}"
        );
    }
}

#[test]
fn reversed_empty_range_sarif_is_a_parseable_finding() {
    let mut repo = TestRepo::new();
    repo.add_commit("feat: initial");
    repo.add_commit("fix: second");
    let (_, registry) = run(&repo, &["registry", "--format", "json"]);

    let (status, sarif) = run(
        &repo,
        &["lint", "--range", "HEAD..HEAD~1", "--format", "sarif"],
    );

    assert_eq!(status.code(), Some(1));
    assert_eq!(sarif["version"], "2.1.0");
    assert_eq!(
        sarif["runs"][0]["invocations"][0]["executionSuccessful"],
        true
    );
    assert_eq!(
        sarif["runs"][0]["results"][0]["ruleId"],
        "GITSTD-LINT-EMPTY-RANGE"
    );
    let definition = registry["diagnostics"]
        .as_array()
        .expect("registry diagnostics")
        .iter()
        .find(|entry| entry["code"] == "GITSTD-LINT-EMPTY-RANGE")
        .expect("empty-range registry definition");
    assert_eq!(definition["class"], "finding");
    assert_eq!(
        definition["explanation"],
        "A lint range was empty while its inverse contained commits."
    );
    let descriptor = sarif["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .expect("SARIF rules")
        .iter()
        .find(|rule| rule["id"] == "GITSTD-LINT-EMPTY-RANGE")
        .expect("empty-range SARIF rule descriptor");
    assert_eq!(
        descriptor["shortDescription"]["text"],
        definition["explanation"]
    );
    for result in sarif["runs"][0]["results"].as_array().expect("results") {
        let rule_id = result["ruleId"].as_str().expect("result ruleId");
        assert!(
            registry["diagnostics"]
                .as_array()
                .expect("registry diagnostics")
                .iter()
                .any(|entry| entry["code"] == rule_id),
            "SARIF result {rule_id} must be registered"
        );
        assert!(
            sarif["runs"][0]["tool"]["driver"]["rules"]
                .as_array()
                .expect("SARIF rules")
                .iter()
                .any(|rule| rule["id"] == rule_id),
            "SARIF result {rule_id} must have a rule descriptor"
        );
    }
}

#[test]
fn ordinary_empty_range_sarif_is_a_parseable_success() {
    let mut repo = TestRepo::new();
    repo.add_commit("feat: initial");

    let (status, sarif) = run(
        &repo,
        &["lint", "--range", "HEAD..HEAD", "--format", "sarif"],
    );

    assert!(status.success());
    assert_eq!(sarif["version"], "2.1.0");
    assert_eq!(
        sarif["runs"][0]["invocations"][0]["executionSuccessful"],
        true
    );
    assert_eq!(sarif["runs"][0]["results"], serde_json::json!([]));
}
