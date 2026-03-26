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

/// Helper: check if a tag exists.
fn tag_exists(dir: &Path, tag: &str) -> bool {
    std::process::Command::new("git")
        .current_dir(dir)
        .args(["rev-parse", "--verify", &format!("refs/tags/{tag}")])
        .output()
        .unwrap()
        .status
        .success()
}

/// Helper: check if a branch exists.
fn branch_exists(dir: &Path, branch: &str) -> bool {
    std::process::Command::new("git")
        .current_dir(dir)
        .args(["rev-parse", "--verify", &format!("refs/heads/{branch}")])
        .output()
        .unwrap()
        .status
        .success()
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

// --- Patch scheme integration tests (#138) ---

#[test]
fn bump_patch_scheme_produces_patch_from_feat() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Write a .git-std.toml with patch scheme.
    std::fs::write(dir.path().join(".git-std.toml"), "scheme = \"patch\"\n").unwrap();

    // Add a feat commit — should still produce a patch bump.
    add_commit(dir.path(), "a.txt", "feat: add feature A");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.0.0 \u{2192} 1.0.1"))
        .stderr(predicate::str::contains("patch"));

    // Verify tag was created.
    assert!(tag_exists(dir.path(), "v1.0.1"), "tag v1.0.1 should exist");
}

#[test]
fn bump_patch_scheme_rejects_breaking_without_force() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Write a .git-std.toml with patch scheme.
    std::fs::write(dir.path().join(".git-std.toml"), "scheme = \"patch\"\n").unwrap();

    // Add a breaking change commit.
    add_commit(dir.path(), "a.txt", "feat!: remove old API");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "breaking change not allowed on patch-only branch (use --force to override)",
        ));
}

#[test]
fn bump_patch_scheme_allows_breaking_with_force() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");

    // Write a .git-std.toml with patch scheme.
    std::fs::write(dir.path().join(".git-std.toml"), "scheme = \"patch\"\n").unwrap();

    // Add a breaking change commit.
    add_commit(dir.path(), "a.txt", "feat!: remove old API");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--force"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.0.0 \u{2192} 1.0.1"))
        .stderr(predicate::str::contains("patch"));

    // Verify tag was created.
    assert!(tag_exists(dir.path(), "v1.0.1"), "tag v1.0.1 should exist");
}

// --- Stable branch integration tests (#139) ---

#[test]
fn bump_stable_creates_branch_and_bumps_major() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "feat: new feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--stable", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Stable branch created"))
        .stderr(predicate::str::contains("stable-v1.0"))
        .stderr(predicate::str::contains("patch (patch-only bumps)"))
        .stderr(predicate::str::contains("Committed"))
        .stderr(predicate::str::contains("1.0.0 \u{2192} 2.0.0"))
        .stderr(predicate::str::contains("major"))
        .stderr(predicate::str::contains("Tagged"));

    // Verify the stable branch was created.
    assert!(
        branch_exists(dir.path(), "stable-v1.0"),
        "stable-v1.0 branch should exist"
    );

    // Verify HEAD is back on the original branch with the new tag.
    assert!(tag_exists(dir.path(), "v2.0.0"), "tag v2.0.0 should exist");

    // Verify main was bumped in Cargo.toml.
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(
        cargo.contains("version = \"2.0.0\""),
        "expected version 2.0.0, got: {cargo}"
    );

    // Verify the stable branch has .git-std.toml with scheme = "patch".
    let config_content = git(dir.path(), &["show", "stable-v1.0:.git-std.toml"]);
    assert!(
        config_content.contains("scheme = \"patch\""),
        "stable branch config should have scheme = \"patch\", got: {config_content}"
    );
}

#[test]
fn bump_stable_with_minor_flag() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "feat: new feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--stable", "--minor", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("1.0.0 \u{2192} 1.1.0"))
        .stderr(predicate::str::contains("minor"));

    // Verify the tag.
    assert!(tag_exists(dir.path(), "v1.1.0"), "tag v1.1.0 should exist");

    // Verify Cargo.toml.
    let cargo = std::fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
    assert!(
        cargo.contains("version = \"1.1.0\""),
        "expected version 1.1.0, got: {cargo}"
    );
}

#[test]
fn bump_stable_custom_branch_name() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "feat: new feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--stable", "my-release-branch", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("my-release-branch"));

    // Verify the custom branch was created.
    assert!(
        branch_exists(dir.path(), "my-release-branch"),
        "my-release-branch should exist"
    );
}

#[test]
fn bump_stable_dry_run() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "feat: new feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--stable", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Would create stable branch"))
        .stderr(predicate::str::contains("stable-v1.0"))
        .stderr(predicate::str::contains("Would commit"))
        .stderr(predicate::str::contains("Would tag"))
        .stderr(predicate::str::contains("Would advance"))
        .stderr(predicate::str::contains("1.0.0 \u{2192} 2.0.0"));

    // No branch should be created.
    assert!(
        !branch_exists(dir.path(), "stable-v1.0"),
        "stable-v1.0 branch should NOT exist in dry-run"
    );

    // No tag should be created.
    assert!(
        !tag_exists(dir.path(), "v2.0.0"),
        "tag v2.0.0 should NOT exist in dry-run"
    );
}

#[test]
fn bump_stable_rejects_existing_branch() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "feat: new feature");

    // Pre-create the branch that --stable would try to create.
    git(dir.path(), &["branch", "stable-v1.0"]);

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--stable", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "branch 'stable-v1.0' already exists",
        ));
}

#[test]
fn bump_stable_rejects_calver_scheme() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());

    // Write a .git-std.toml with calver scheme.
    std::fs::write(dir.path().join(".git-std.toml"), "scheme = \"calver\"\n").unwrap();

    add_commit(dir.path(), "a.txt", "feat: new feature");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--stable"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "--stable is not supported with scheme = \"calver\"",
        ));
}

#[test]
fn bump_stable_rejects_dirty_working_tree() {
    let dir = tempfile::tempdir().unwrap();
    init_bump_repo(dir.path());
    create_tag(dir.path(), "v1.0.0");
    add_commit(dir.path(), "a.txt", "feat: new feature");

    // Create an uncommitted file to make the working tree dirty.
    std::fs::write(dir.path().join("dirty.txt"), "uncommitted").unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--stable"])
        .current_dir(dir.path())
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "working tree has uncommitted changes",
        ));
}
