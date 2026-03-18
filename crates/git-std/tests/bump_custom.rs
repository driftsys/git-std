use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

/// Helper: run a git command and return stdout.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Helper: initialise a git repo with a Cargo.toml and one commit.
fn init_bump_repo(dir: &Path) {
    git(dir, &["init"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@test.com"]);

    // Write a minimal Cargo.toml.
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"test-pkg\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    git(dir, &["add", "Cargo.toml"]);
    git(dir, &["commit", "-m", "chore: init"]);
}

/// Helper: add a commit to a repo.
fn add_commit(dir: &Path, filename: &str, message: &str) {
    std::fs::write(dir.join(filename), message).unwrap();
    git(dir, &["add", filename]);
    git(dir, &["commit", "-m", message]);
}

/// Helper: create an annotated tag.
fn create_tag(dir: &Path, name: &str) {
    git(dir, &["tag", "-a", name, "-m", name]);
}

// --- Custom version file (regex) integration tests ---

#[test]
fn bump_custom_regex_version_file() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Write a custom version file.
    std::fs::write(dir.path().join("version.txt"), "version = \"1.0.0\"\n").unwrap();

    // Write .git-std.toml with a [[version_files]] entry.
    std::fs::write(
        dir.path().join(".git-std.toml"),
        r#"[[version_files]]
path = "version.txt"
regex = 'version = "(\d+\.\d+\.\d+)"'
"#,
    )
    .unwrap();

    add_commit(dir.path(), "a.txt", "feat: add feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.0.0 → 1.1.0"));

    // Verify the custom file was updated.
    let content = std::fs::read_to_string(dir.path().join("version.txt")).unwrap();
    assert!(
        content.contains("version = \"1.1.0\""),
        "custom version file should be updated, got: {content}"
    );

    // Cargo.toml should also be updated.
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"1.1.0\""));
}

#[test]
fn bump_multiple_custom_files() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Write two custom version files.
    std::fs::write(dir.path().join("version.txt"), "version = \"1.0.0\"\n").unwrap();
    std::fs::write(
        dir.path().join("Chart.yaml"),
        "name: my-chart\nversion: 1.0.0\nappVersion: 1.0.0\n",
    )
    .unwrap();

    // Config with two [[version_files]] entries.
    std::fs::write(
        dir.path().join(".git-std.toml"),
        r#"[[version_files]]
path = "version.txt"
regex = 'version = "(\d+\.\d+\.\d+)"'

[[version_files]]
path = "Chart.yaml"
regex = 'version:\s*(\S+)'
"#,
    )
    .unwrap();

    add_commit(dir.path(), "a.txt", "feat: add feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.0.0 → 1.1.0"));

    // Verify both custom files were updated.
    let version_txt = std::fs::read_to_string(dir.path().join("version.txt")).unwrap();
    assert!(
        version_txt.contains("version = \"1.1.0\""),
        "version.txt should be updated, got: {version_txt}"
    );

    let chart = std::fs::read_to_string(dir.path().join("Chart.yaml")).unwrap();
    assert!(
        chart.contains("version: 1.1.0"),
        "Chart.yaml version should be updated, got: {chart}"
    );
}

#[test]
fn bump_custom_file_missing() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Config points to a nonexistent file.
    std::fs::write(
        dir.path().join(".git-std.toml"),
        r#"[[version_files]]
path = "nonexistent.txt"
regex = 'version = "(\d+\.\d+\.\d+)"'
"#,
    )
    .unwrap();

    add_commit(dir.path(), "a.txt", "feat: add feature");

    // Should still succeed — missing custom files are skipped silently.
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.0.0 → 1.1.0"));

    // Cargo.toml should still be updated.
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"1.1.0\""));
}

#[test]
fn bump_dry_run_shows_custom_files() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Write a custom version file.
    std::fs::write(dir.path().join("version.txt"), "version = \"1.0.0\"\n").unwrap();

    // Config with a [[version_files]] entry.
    std::fs::write(
        dir.path().join(".git-std.toml"),
        r#"[[version_files]]
path = "version.txt"
regex = 'version = "(\d+\.\d+\.\d+)"'
"#,
    )
    .unwrap();

    add_commit(dir.path(), "a.txt", "feat: add feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("version.txt"))
        .stderr(predicate::str::contains("Cargo.toml"))
        .stderr(predicate::str::contains("Would update"));
}
