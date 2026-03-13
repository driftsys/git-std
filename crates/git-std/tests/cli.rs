use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn version_flag_prints_version() {
    Command::cargo_bin("git-std")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_lists_subcommands() {
    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .arg("--help")
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    for sub in [
        "commit",
        "check",
        "bump",
        "changelog",
        "hooks",
        "self-update",
    ] {
        assert!(
            stdout.contains(sub),
            "help output should list '{sub}' subcommand"
        );
    }
}

#[test]
fn unknown_subcommand_exits_2() {
    Command::cargo_bin("git-std")
        .unwrap()
        .arg("does-not-exist")
        .assert()
        .code(2);
}

#[test]
fn stub_subcommands_are_recognized() {
    for sub in ["hooks", "self-update"] {
        Command::cargo_bin("git-std")
            .unwrap()
            .arg(sub)
            .assert()
            .code(1)
            .stderr(predicates::str::contains("not yet implemented"));
    }
}

#[test]
fn commit_dry_run_prints_message() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["commit", "--type", "feat", "-m", "add login", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("feat: add login"));
}

#[test]
fn commit_dry_run_with_scope() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "commit",
            "--type",
            "fix",
            "--scope",
            "auth",
            "-m",
            "handle tokens",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("fix(auth): handle tokens"));
}

#[test]
fn commit_dry_run_with_breaking() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "commit",
            "--type",
            "feat",
            "-m",
            "remove legacy API",
            "--breaking",
            "removed v1 endpoints",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("feat!: remove legacy API"))
        .stdout(predicate::str::contains(
            "BREAKING CHANGE: removed v1 endpoints",
        ));
}

#[test]
fn commit_help_shows_flags() {
    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["commit", "--help"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    for flag in [
        "--type",
        "--scope",
        "--message",
        "--breaking",
        "--dry-run",
        "--amend",
        "--sign",
        "--all",
    ] {
        assert!(
            stdout.contains(flag),
            "commit help should list '{flag}' flag"
        );
    }
}

#[test]
fn commit_short_flags() {
    // -m alias for --message
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["commit", "--type", "feat", "-m", "short flag", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("feat: short flag"));
}

// --- Bump integration tests ---

/// Helper: initialise a git repo with a Cargo.toml and one commit.
fn init_bump_repo(dir: &Path) -> git2::Repository {
    let repo = git2::Repository::init(dir).unwrap();
    {
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();
    }

    // Write a minimal Cargo.toml.
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"test-pkg\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    // Stage and commit.
    {
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("Cargo.toml")).unwrap();
        index.write().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "chore: init", &tree, &[])
            .unwrap();
    }

    repo
}

/// Helper: add a commit to a repo.
fn add_commit(repo: &git2::Repository, dir: &Path, filename: &str, message: &str) {
    std::fs::write(dir.join(filename), message).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new(filename)).unwrap();
    index.write().unwrap();
    let tree_oid = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_oid).unwrap();
    let sig = repo.signature().unwrap();
    let parent = repo.head().unwrap().peel_to_commit().unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])
        .unwrap();
}

/// Helper: create an annotated tag.
fn create_tag(repo: &git2::Repository, name: &str) {
    let sig = repo.signature().unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let obj = head.as_object();
    repo.tag(name, obj, &sig, name, false).unwrap();
}

#[test]
fn bump_help_shows_flags() {
    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--help"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    for flag in [
        "--dry-run",
        "--prerelease",
        "--release-as",
        "--first-release",
        "--no-tag",
        "--no-commit",
        "--skip-changelog",
        "--sign",
    ] {
        assert!(stdout.contains(flag), "bump help should list '{flag}' flag");
    }
}

#[test]
fn bump_dry_run_shows_plan() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_bump_repo(dir.path());

    // Tag v0.1.0 on the init commit.
    create_tag(&repo, "v0.1.0");

    // Add a feat commit.
    add_commit(&repo, dir.path(), "a.txt", "feat: add feature A");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("0.1.0 → 0.2.0"))
        .stderr(predicate::str::contains("Would commit"))
        .stderr(predicate::str::contains("Would tag"));
}

#[test]
fn bump_dry_run_no_writes() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_bump_repo(dir.path());
    create_tag(&repo, "v0.1.0");
    add_commit(&repo, dir.path(), "a.txt", "feat: add feature A");

    // Read Cargo.toml before.
    let before = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Cargo.toml should be unchanged.
    let after = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert_eq!(before, after);

    // No CHANGELOG.md should be created.
    assert!(!dir.path().join("CHANGELOG.md").exists());
}

#[test]
fn bump_performs_full_workflow() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_bump_repo(dir.path());
    create_tag(&repo, "v1.0.0");

    // Add a fix commit.
    add_commit(&repo, dir.path(), "b.txt", "fix: handle edge case");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.0.0 → 1.0.1"))
        .stderr(predicate::str::contains("Committed"))
        .stderr(predicate::str::contains("Tagged"));

    // Verify Cargo.toml was updated.
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"1.0.1\""));

    // Verify CHANGELOG.md was created.
    assert!(dir.path().join("CHANGELOG.md").exists());

    // Verify the tag exists.
    let tag = repo.find_reference("refs/tags/v1.0.1");
    assert!(tag.is_ok(), "tag v1.0.1 should exist");

    // Verify commit message.
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(head.message().unwrap(), "chore(release): 1.0.1");
}

#[test]
fn bump_no_bump_worthy_commits() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_bump_repo(dir.path());
    create_tag(&repo, "v1.0.0");

    // Add a non-bump commit.
    add_commit(&repo, dir.path(), "c.txt", "chore: update deps");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("no bump-worthy commits"));
}

#[test]
fn bump_major_on_breaking_change() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_bump_repo(dir.path());
    create_tag(&repo, "v1.2.3");

    add_commit(&repo, dir.path(), "d.txt", "feat!: remove old API");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.2.3 → 2.0.0"));
}

#[test]
fn bump_no_commit_flag() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_bump_repo(dir.path());
    create_tag(&repo, "v1.0.0");
    add_commit(&repo, dir.path(), "e.txt", "feat: new thing");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--no-commit"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Cargo.toml should be updated.
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"1.1.0\""));

    // No release commit should exist.
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    assert_ne!(head.message().unwrap(), "chore(release): 1.1.0");

    // No tag should exist.
    assert!(repo.find_reference("refs/tags/v1.1.0").is_err());
}

#[test]
fn bump_skip_changelog() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_bump_repo(dir.path());
    create_tag(&repo, "v1.0.0");
    add_commit(&repo, dir.path(), "f.txt", "fix: a fix");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .success();

    // No CHANGELOG.md should be created.
    assert!(!dir.path().join("CHANGELOG.md").exists());
}

#[test]
fn bump_release_as() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_bump_repo(dir.path());
    create_tag(&repo, "v1.0.0");
    add_commit(&repo, dir.path(), "g.txt", "fix: a fix");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--release-as", "5.0.0"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.0.0 → 5.0.0"));

    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"5.0.0\""));
}

#[test]
fn bump_first_release() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_bump_repo(dir.path());
    // No tags — first release. Add a feat commit so the changelog has content.
    add_commit(&repo, dir.path(), "init.txt", "feat: initial feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--first-release"])
        .current_dir(dir.path())
        .assert()
        .success();

    // CHANGELOG.md should be created.
    assert!(dir.path().join("CHANGELOG.md").exists());

    // Version should stay at 0.0.0 (first-release doesn't bump).
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("version = \"0.0.0\""));
}
