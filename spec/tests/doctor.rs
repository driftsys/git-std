#[path = "../support/mod.rs"]
mod support;

use snapbox::cmd::Command;
use support::TestRepo;

/// `doctor` exits 0 in a fully-configured git repo (no problems).
#[test]
fn doctor_exits_0_in_basic_repo() {
    let repo = TestRepo::new().with_hooks_setup();

    Command::new(TestRepo::bin_path())
        .args(["doctor"])
        .current_dir(repo.path())
        .assert()
        .success();
}

/// `doctor` shows Status section with git and git-std versions.
#[test]
fn doctor_shows_status_section() {
    let repo = TestRepo::new();

    let output = Command::new(TestRepo::bin_path())
        .args(["doctor"])
        .current_dir(repo.path())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Status"), "should show Status section");
    assert!(stderr.contains("git "), "should show git version");
    assert!(stderr.contains("git-std "), "should show git-std version");
}

/// `doctor` shows Configuration section with scheme.
#[test]
fn doctor_shows_configuration_section() {
    let repo = TestRepo::new();

    let output = Command::new(TestRepo::bin_path())
        .args(["doctor"])
        .current_dir(repo.path())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Configuration"),
        "should show Configuration section"
    );
    assert!(stderr.contains("scheme"), "should show scheme key");
    assert!(
        stderr.contains("semver"),
        "should show default scheme value"
    );
}

/// `doctor` shows Hooks section when `.hooks` files are configured.
#[test]
fn doctor_shows_hooks_section() {
    let repo =
        TestRepo::new().with_hooks_file("pre-commit", "! cargo fmt --check\n? git lfs install\n");

    let output = Command::new(TestRepo::bin_path())
        .args(["doctor"])
        .current_dir(repo.path())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Hooks"), "should show Hooks section");
    assert!(stderr.contains("pre-commit"), "should show hook name");
    assert!(
        stderr.contains("!  cargo fmt --check"),
        "should show fail-fast command with sigil"
    );
    assert!(
        stderr.contains("?  git lfs install"),
        "should show advisory command with sigil"
    );
}

/// `doctor` shows disabled label for hooks without a shim.
#[test]
fn doctor_shows_disabled_hook() {
    let repo = TestRepo::new().with_hooks_file("commit-msg", "! git std lint -f\n");
    // No shim → hook is disabled.

    let output = Command::new(TestRepo::bin_path())
        .args(["doctor"])
        .current_dir(repo.path())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("commit-msg (disabled)"),
        "should show disabled label: {stderr}"
    );
}

/// `doctor` with invalid `.git-std.toml` exits 1 and shows a hint.
#[test]
fn doctor_shows_hint_for_invalid_toml() {
    let repo = TestRepo::new().with_config("[[invalid toml = bad\n");

    let output = Command::new(TestRepo::bin_path())
        .args(["doctor"])
        .current_dir(repo.path())
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(1),
        "should exit 1 on config error"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("hint:"), "should show hint");
    assert!(
        stderr.contains(".git-std.toml invalid"),
        "hint should mention toml error"
    );
}

/// `doctor --format json` outputs to stdout and stderr is empty.
#[test]
fn doctor_json_format() {
    let repo = TestRepo::new().with_hooks_setup();

    let output = Command::new(TestRepo::bin_path())
        .args(["doctor", "--format", "json"])
        .current_dir(repo.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "stderr should be empty in JSON mode"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("should be valid JSON");
    assert_eq!(parsed["status"], "pass");
    let sections = parsed["sections"].as_object().expect("sections object");
    assert!(sections.contains_key("status"));
    assert!(sections.contains_key("hooks"));
    assert!(sections.contains_key("configuration"));
}

/// `doctor` skips git-lfs in Status when `.gitattributes` has no filter=lfs.
#[test]
fn doctor_skips_lfs_without_filter() {
    let repo = TestRepo::new();
    std::fs::write(repo.path().join(".gitattributes"), "*.png binary\n").unwrap();

    let output = Command::new(TestRepo::bin_path())
        .args(["doctor"])
        .current_dir(repo.path())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("git-lfs"),
        "should not show git-lfs when no filter=lfs"
    );
}

/// `doctor` includes git-lfs in Status when `.gitattributes` has filter=lfs.
#[test]
fn doctor_shows_lfs_with_filter() {
    let repo = TestRepo::new();
    std::fs::write(repo.path().join(".gitattributes"), "*.bin filter=lfs\n").unwrap();

    let output = Command::new(TestRepo::bin_path())
        .args(["doctor"])
        .current_dir(repo.path())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("git-lfs"),
        "should show git-lfs when .gitattributes has filter=lfs"
    );
}
