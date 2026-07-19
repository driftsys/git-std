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

    // Should still succeed — missing custom files are skipped, but a warning is emitted.
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.0.0 → 1.1.0"))
        .stderr(predicate::str::contains("warning:"))
        .stderr(predicate::str::contains("file not found"));

    // Cargo.toml should still be updated.
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"1.1.0\""));
}

/// Custom [[version_files]] pointing to a subdirectory Cargo.toml triggers
/// Cargo.lock sync (the name ends with "Cargo.toml" even though it's not
/// the root Cargo.toml).
#[test]
fn bump_custom_cargo_toml_triggers_lock_sync() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["config", "user.email", "test@test.com"]);

    // Simulate a workspace: root Cargo.toml has no [package], crate is in a subdir.
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/my-crate\"]\nresolver = \"3\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("crates/my-crate")).unwrap();
    std::fs::write(
        dir.path().join("crates/my-crate/Cargo.toml"),
        "[package]\nname = \"my-crate\"\nversion = \"1.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    // Cargo.lock must exist for the sync to be attempted.
    std::fs::write(dir.path().join("Cargo.lock"), "# placeholder\n").unwrap();

    // Config points to the subdirectory Cargo.toml via custom regex.
    std::fs::write(
        dir.path().join(".git-std.toml"),
        "[[version_files]]\npath = \"crates/my-crate/Cargo.toml\"\nregex = '(?m)^version = \"([^\"]+)\"'\n",
    )
    .unwrap();

    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-m", "chore: init"]);
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "fix: a fix");

    // Dry-run should report "Would sync: Cargo.lock".
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Would sync"))
        .stderr(predicate::str::contains("Cargo.lock"));
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

// --- Glob pattern integration tests ---

#[test]
fn bump_glob_version_files_updates_all_matches() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Write several files matching a glob pattern.
    std::fs::create_dir_all(dir.path().join("skills/alpha")).unwrap();
    std::fs::create_dir_all(dir.path().join("skills/beta")).unwrap();
    std::fs::write(dir.path().join("skills/alpha/SKILL.md"), "version: 1.0.0\n").unwrap();
    std::fs::write(dir.path().join("skills/beta/SKILL.md"), "version: 1.0.0\n").unwrap();

    // Config with a glob [[version_files]] entry.
    std::fs::write(
        dir.path().join(".git-std.toml"),
        r#"[[version_files]]
path = "skills/*/SKILL.md"
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

    let alpha = std::fs::read_to_string(dir.path().join("skills/alpha/SKILL.md")).unwrap();
    assert!(
        alpha.contains("version: 1.1.0"),
        "alpha SKILL.md should be updated, got: {alpha}"
    );
    let beta = std::fs::read_to_string(dir.path().join("skills/beta/SKILL.md")).unwrap();
    assert!(
        beta.contains("version: 1.1.0"),
        "beta SKILL.md should be updated, got: {beta}"
    );
}

#[test]
fn bump_glob_version_files_zero_matches_warns() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Config with a glob [[version_files]] entry that matches nothing.
    std::fs::write(
        dir.path().join(".git-std.toml"),
        r#"[[version_files]]
path = "skills/*/SKILL.md"
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
        .stderr(predicate::str::contains("1.0.0 → 1.1.0"))
        .stderr(predicate::str::contains(
            "warning: no files matched version_files glob: skills/*/SKILL.md",
        ));

    // Cargo.toml should still be updated.
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"1.1.0\""));
}

#[test]
fn bump_glob_version_files_invalid_pattern_warns_once() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // An unterminated `[` character class is invalid glob syntax, not a
    // valid-but-empty glob — it must not also trigger the zero-match warning.
    std::fs::write(
        dir.path().join(".git-std.toml"),
        r#"[[version_files]]
path = "skills/[/SKILL.md"
regex = 'version:\s*(\S+)'
"#,
    )
    .unwrap();

    add_commit(dir.path(), "a.txt", "feat: add feature");

    let output = Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--skip-changelog"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid version_files glob pattern"),
        "expected invalid-pattern warning, got: {stderr}"
    );
    assert!(
        !stderr.contains("no files matched version_files glob"),
        "should not also emit the redundant zero-match warning, got: {stderr}"
    );

    // Cargo.toml should still be updated — the invalid custom entry is
    // non-fatal.
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"1.1.0\""));
}

#[test]
fn bump_dry_run_shows_glob_matched_files() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    std::fs::create_dir_all(dir.path().join("skills/alpha")).unwrap();
    std::fs::create_dir_all(dir.path().join("skills/beta")).unwrap();
    std::fs::write(dir.path().join("skills/alpha/SKILL.md"), "version: 1.0.0\n").unwrap();
    std::fs::write(dir.path().join("skills/beta/SKILL.md"), "version: 1.0.0\n").unwrap();

    std::fs::write(
        dir.path().join(".git-std.toml"),
        r#"[[version_files]]
path = "skills/*/SKILL.md"
regex = 'version:\s*(\S+)'
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
        .stderr(predicate::str::contains("skills/alpha/SKILL.md"))
        .stderr(predicate::str::contains("skills/beta/SKILL.md"))
        .stderr(predicate::str::contains("Would update"));
}

#[test]
fn bump_glob_version_files_brace_alternation() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Two files with different extensions, both matched by a brace group.
    std::fs::write(dir.path().join("version.md"), "version: 1.0.0\n").unwrap();
    std::fs::write(dir.path().join("version.txt"), "version: 1.0.0\n").unwrap();
    // A third file that should NOT be matched.
    std::fs::write(dir.path().join("version.json"), "version: 1.0.0\n").unwrap();

    std::fs::write(
        dir.path().join(".git-std.toml"),
        r#"[[version_files]]
path = "version.{md,txt}"
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

    let md = std::fs::read_to_string(dir.path().join("version.md")).unwrap();
    assert!(
        md.contains("version: 1.1.0"),
        "version.md should be updated, got: {md}"
    );
    let txt = std::fs::read_to_string(dir.path().join("version.txt")).unwrap();
    assert!(
        txt.contains("version: 1.1.0"),
        "version.txt should be updated, got: {txt}"
    );
    let json = std::fs::read_to_string(dir.path().join("version.json")).unwrap();
    assert!(
        json.contains("version: 1.0.0"),
        "version.json should be unchanged (not part of the brace group), got: {json}"
    );
}

#[test]
fn bump_plain_version_skipped_when_specific_ecosystem_present() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path()); // creates Cargo.toml with version = "0.0.0"
    create_tag(dir.path(), "v1.0.0");

    // Patch the Cargo.toml version to match the tag.
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"test-pkg\"\nversion = \"1.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    // Add a plain VERSION file alongside Cargo.toml.
    std::fs::write(dir.path().join("VERSION"), "1.0.0\n").unwrap();

    git(dir.path(), &["add", "."]);
    git(
        dir.path(),
        &["commit", "-m", "chore: set version files to 1.0.0"],
    );

    add_commit(dir.path(), "a.txt", "feat: add feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Cargo.toml should be updated.
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(
        cargo.contains("version = \"1.1.0\""),
        "Cargo.toml should be updated to 1.1.0"
    );

    // VERSION should NOT be updated — Plain is skipped when Rust matched.
    let version_file = std::fs::read_to_string(dir.path().join("VERSION")).unwrap();
    assert_eq!(
        version_file.trim(),
        "1.0.0",
        "VERSION should be unchanged when Rust ecosystem matched"
    );
}

#[test]
fn bump_dry_run_plain_skipped_when_specific_ecosystem_present() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"test-pkg\"\nversion = \"1.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    std::fs::write(dir.path().join("VERSION"), "1.0.0\n").unwrap();

    git(dir.path(), &["add", "."]);
    git(
        dir.path(),
        &["commit", "-m", "chore: set version files to 1.0.0"],
    );

    add_commit(dir.path(), "a.txt", "feat: add feature");

    // Dry-run should show Cargo.toml but NOT VERSION.
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Cargo.toml"))
        .stderr(predicate::str::contains("VERSION").not());
}
