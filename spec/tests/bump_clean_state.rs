#[path = "../support/mod.rs"]
mod support;

use std::process::Command;
use support::TestRepo;

fn fixture() -> TestRepo {
    let repo = TestRepo::new()
        .with_config("monorepo = false\n")
        .with_project_toml("1.0.0")
        .with_hooks_file("pre-bump", "! echo ran > hook-sentinel\n");
    repo.write("tracked.txt", "original\n");
    repo.write(".gitignore", "ignored/\n");
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "fix: prepare release"]);
    repo.create_tag("v1.0.0");
    repo
}

fn bump(repo: &TestRepo, args: &[&str]) -> std::process::Output {
    Command::new(TestRepo::bin_path())
        .current_dir(repo.path())
        .args(["bump", "--yes", "--release-as", "patch"])
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn dirty_bump_rejects_before_hooks_and_preserves_index_and_files() {
    for staged in [false, true] {
        for path in ["tracked.txt", "untracked.txt"] {
            for args in [vec![], vec!["--no-commit"], vec!["--format", "json"]] {
                let repo = fixture();
                repo.write(path, "user change\n");
                if staged {
                    repo.git(&["add", path]);
                    repo.write(path, "later user change\n");
                }
                let before = repo.snapshot_state();
                let index = repo.git(&["ls-files", "--stage"]);
                let output = bump(&repo, &args);
                assert_eq!(output.status.code(), Some(1), "{output:?}");
                if args.contains(&"json") {
                    let document: serde_json::Value =
                        serde_json::from_slice(&output.stdout).unwrap();
                    assert_eq!(document["diagnostics"][0]["code"], "GITSTD-BUMP-DIRTY");
                } else {
                    assert!(String::from_utf8_lossy(&output.stderr).contains("clean"));
                }
                assert_eq!(repo.snapshot_state(), before);
                assert_eq!(repo.git(&["ls-files", "--stage"]), index);
                assert!(!repo.path().join("hook-sentinel").exists());
            }
        }
    }
}

#[test]
fn hidden_untracked_files_still_block_bump() {
    let repo = fixture();
    repo.git(&["config", "status.showUntrackedFiles", "no"]);
    repo.write("untracked.txt", "user change\n");
    assert_eq!(bump(&repo, &[]).status.code(), Some(1));
    assert!(!repo.path().join("hook-sentinel").exists());
}

#[test]
fn ignored_artifacts_do_not_block_bump() {
    let repo = fixture();
    repo.write("ignored/artifact", "build output\n");
    let output = bump(&repo, &[]);
    assert!(output.status.success(), "{output:?}");
    assert!(
        !repo
            .git(&["ls-tree", "-r", "--name-only", "HEAD"])
            .contains("artifact")
    );
}

#[test]
fn dirty_preview_is_read_only() {
    let repo = fixture();
    repo.write("tracked.txt", "user change\n");
    let before = repo.snapshot_state();
    let output = bump(&repo, &["--dry-run"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(repo.snapshot_state(), before);
}

#[test]
fn dirty_stable_preview_is_read_only_but_apply_creates_no_branch() {
    let repo = fixture();
    repo.write("tracked.txt", "pending\n");
    let before = repo.snapshot_state();
    let branches = repo.git(&["branch", "--list"]);
    assert!(bump(&repo, &["--stable", "--dry-run"]).status.success());
    assert_eq!(bump(&repo, &["--stable"]).status.code(), Some(1));
    assert_eq!(repo.snapshot_state(), before);
    assert_eq!(repo.git(&["branch", "--list"]), branches);
}

#[test]
fn ignored_dirty_submodule_still_blocks_bump() {
    let source = TestRepo::new();
    source.write("tracked.txt", "original\n");
    source.git(&["add", "."]);
    source.git(&["commit", "-m", "chore: initialize submodule"]);
    let repo = fixture();
    repo.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        source.path().to_str().unwrap(),
        "child",
    ]);
    repo.git(&["commit", "-am", "chore: add submodule"]);
    repo.git(&["config", "submodule.child.ignore", "all"]);
    repo.git(&["-C", "child", "config", "status.showUntrackedFiles", "no"]);
    for path in ["child/tracked.txt", "child/untracked.txt"] {
        repo.write(path, "pending\n");
        assert_eq!(bump(&repo, &[]).status.code(), Some(1));
        assert!(!repo.path().join("hook-sentinel").exists());
        std::fs::remove_file(repo.path().join(path)).unwrap();
        if path.ends_with("/tracked.txt") {
            repo.write(path, "original\n");
        }
    }
}

#[test]
fn nested_submodule_ignore_settings_cannot_hide_pending_changes() {
    let leaf = TestRepo::new();
    leaf.write("tracked.txt", "original\n");
    leaf.git(&["add", "."]);
    leaf.git(&["commit", "-m", "chore: initialize leaf"]);
    let child = TestRepo::new();
    child.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        leaf.path().to_str().unwrap(),
        "leaf",
    ]);
    child.git(&["commit", "-am", "chore: add leaf"]);
    let repo = fixture();
    repo.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        child.path().to_str().unwrap(),
        "child",
    ]);
    repo.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "update",
        "--init",
        "--recursive",
    ]);
    repo.git(&["commit", "-am", "chore: add child"]);
    repo.git(&["-C", "child", "config", "submodule.leaf.ignore", "all"]);
    repo.write("child/leaf/tracked.txt", "pending\n");
    let before = repo.snapshot_state();
    assert_eq!(bump(&repo, &[]).status.code(), Some(1));
    assert_eq!(repo.snapshot_state(), before);
}
