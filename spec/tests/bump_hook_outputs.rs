#[path = "../support/mod.rs"]
mod support;

use std::process::Command;
use support::TestRepo;

fn fixture(hook: &str, commands: &str) -> TestRepo {
    let repo = TestRepo::new()
        .with_config("monorepo = false\n")
        .with_project_toml("1.0.0")
        .with_hooks_file(hook, commands);
    repo.write("metadata.txt", "1.0.0\n");
    repo.write("obsolete.txt", "old\n");
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "chore: initial release"]);
    repo.create_tag("v1.0.0");
    repo.write("feature.txt", "feature\n");
    repo.git(&["add", "feature.txt"]);
    repo.git(&["commit", "-m", "fix: add feature"]);
    repo
}

fn bump(repo: &TestRepo, args: &[&str]) -> std::process::Output {
    Command::new(TestRepo::bin_path())
        .current_dir(repo.path())
        .args(["bump", "--yes"])
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn release_includes_exact_hook_index_and_tag_tree() {
    for hook in ["post-version", "post-changelog"] {
        let repo = fixture(
            hook,
            "! echo released > metadata.txt; echo new > created.txt; git add metadata.txt created.txt; git rm obsolete.txt; echo later > metadata.txt; echo artifact > unstaged.txt\n",
        );
        let output = bump(&repo, &["--format", "json"]);
        assert!(output.status.success(), "{output:?}");
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["commit_oid"], repo.git(&["rev-parse", "HEAD"]));
        assert_eq!(repo.git(&["show", "HEAD:metadata.txt"]), "released");
        assert_eq!(repo.git(&["show", "v1.0.1:created.txt"]), "new");
        assert_eq!(
            repo.git(&["rev-parse", "HEAD^{tree}"]),
            repo.git(&["rev-parse", "v1.0.1^{tree}"])
        );
        let paths = repo.git(&["ls-tree", "-r", "--name-only", "HEAD"]);
        assert!(
            !paths
                .lines()
                .any(|p| p == "obsolete.txt" || p == "unstaged.txt")
        );
        assert!(paths.lines().any(|p| p == "CHANGELOG.md"));
        assert!(repo.git(&["show", "HEAD:project.toml"]).contains("1.0.1"));
        assert_eq!(repo.read("metadata.txt"), "later\n");
        assert_eq!(repo.git(&["diff", "--cached", "--name-only"]), "");
    }
}

#[test]
fn no_commit_preserves_hook_staging_without_staging_managed_files() {
    let repo = fixture(
        "post-version",
        "! echo new > created.txt; git add created.txt\n",
    );
    let head = repo.git(&["rev-parse", "HEAD"]);
    let output = bump(&repo, &["--no-commit"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(repo.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(repo.git(&["tag", "--list"]), "v1.0.0");
    assert_eq!(
        repo.git(&["diff", "--cached", "--name-only"]),
        "created.txt"
    );
    assert!(repo.read("project.toml").contains("1.0.1"));
}

#[test]
fn failed_hook_preserves_outputs_without_commit_or_tag() {
    let repo = fixture(
        "post-version",
        "! echo new > created.txt; git add created.txt; false\n",
    );
    let head = repo.git(&["rev-parse", "HEAD"]);
    let output = bump(&repo, &[]);
    assert!(!output.status.success());
    assert_eq!(repo.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(repo.git(&["tag", "--list"]), "v1.0.0");
    assert_eq!(repo.git(&["show", ":created.txt"]), "new");
    assert!(repo.read("project.toml").contains("1.0.1"));
}

#[test]
fn skipped_hook_creates_no_extra_output() {
    let repo = fixture(
        "post-version",
        "! echo new > created.txt; git add created.txt\n",
    );
    let output = Command::new(TestRepo::bin_path())
        .current_dir(repo.path())
        .args(["bump", "--yes"])
        .env("GIT_STD_SKIP_HOOKS", "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(!repo.path().join("created.txt").exists());
}

#[test]
fn hook_outputs_from_linked_worktree_subdirectory_are_committed() {
    let repo = fixture(
        "post-version",
        "! git -C \"$(git rev-parse --show-toplevel)\" rm obsolete.txt\n",
    );
    let parent = tempfile::tempdir().unwrap();
    let worktree = parent.path().join("linked");
    repo.git(&[
        "worktree",
        "add",
        "-b",
        "release-test",
        worktree.to_str().unwrap(),
    ]);
    let nested = worktree.join("nested");
    std::fs::create_dir(&nested).unwrap();
    let output = Command::new(TestRepo::bin_path())
        .current_dir(&nested)
        .args(["bump", "--yes"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let paths = repo.git(&["ls-tree", "-r", "--name-only", "v1.0.1"]);
    assert!(!paths.lines().any(|p| p == "obsolete.txt"));
}

#[cfg(unix)]
#[test]
fn signed_release_includes_hook_staging() {
    let repo = fixture(
        "post-version",
        "! echo released > metadata.txt; echo new > created.txt; git add metadata.txt created.txt; git rm obsolete.txt; echo later > metadata.txt; echo incidental > feature.txt; echo artifact > unstaged.txt\n",
    );
    let keys = tempfile::tempdir().unwrap();
    let key = keys.path().join("signing-key");
    let output = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-f"])
        .arg(&key)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    repo.git(&["config", "gpg.format", "ssh"]);
    repo.git(&["config", "user.signingkey", key.to_str().unwrap()]);
    let output = bump(&repo, &["--sign"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(repo.git(&["show", "v1.0.1:created.txt"]), "new");
    assert_eq!(repo.git(&["show", "HEAD:metadata.txt"]), "released");
    assert_eq!(repo.git(&["show", "HEAD:feature.txt"]), "feature");
    assert_eq!(repo.read("metadata.txt"), "later\n");
    assert_eq!(repo.read("feature.txt"), "incidental\n");
    assert_eq!(
        repo.git(&["rev-parse", "HEAD^{tree}"]),
        repo.git(&["rev-parse", "v1.0.1^{tree}"])
    );
    let paths = repo.git(&["ls-tree", "-r", "--name-only", "HEAD"]);
    assert!(
        !paths
            .lines()
            .any(|path| path == "obsolete.txt" || path == "unstaged.txt")
    );
    assert_eq!(repo.git(&["diff", "--cached", "--name-only"]), "");
    assert!(
        repo.git(&["cat-file", "commit", "HEAD"])
            .contains("gpgsig -----BEGIN SSH SIGNATURE-----")
    );
    assert!(
        repo.git(&["cat-file", "tag", "v1.0.1"])
            .contains("-----BEGIN SSH SIGNATURE-----")
    );
}

#[cfg(unix)]
#[test]
fn git_commit_hooks_stage_outputs_and_failure_preserves_index() {
    use std::os::unix::fs::PermissionsExt;
    for fails in [false, true] {
        let repo = fixture(
            "post-version",
            "! echo new > created.txt; git add created.txt\n",
        );
        let script = repo.path().join(".git/hooks/pre-commit");
        std::fs::write(&script, format!("#!/bin/sh\necho commit-hook > commit-output.txt\ngit add commit-output.txt\nexit {}\n", u8::from(fails))).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let head = repo.git(&["rev-parse", "HEAD"]);
        let output = bump(&repo, &[]);
        if fails {
            assert!(!output.status.success());
            assert_eq!(repo.git(&["rev-parse", "HEAD"]), head);
            assert_eq!(repo.git(&["tag", "--list"]), "v1.0.0");
            assert_eq!(repo.git(&["show", ":created.txt"]), "new");
            assert_eq!(repo.git(&["show", ":commit-output.txt"]), "commit-hook");
        } else {
            assert!(output.status.success(), "{output:?}");
            assert_eq!(repo.git(&["show", "HEAD:commit-output.txt"]), "commit-hook");
            assert_eq!(repo.git(&["show", "HEAD:created.txt"]), "new");
        }
    }
}
