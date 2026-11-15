#[path = "../support/mod.rs"]
mod support;

use std::process::Command;
use support::TestRepo;

fn prepared() -> TestRepo {
    let repo = TestRepo::new().with_project_toml("1.0.0");
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "chore: initialize"]);
    repo.create_tag("v1.0.0");
    repo.write("feature.txt", "new\n");
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "fix: add feature"]);
    let output = Command::new(TestRepo::bin_path())
        .current_dir(repo.path())
        .args(["bump", "--yes", "--no-tag"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    repo
}

#[test]
fn tag_requires_commit() {
    let repo = prepared();
    let output = Command::new(TestRepo::bin_path())
        .current_dir(repo.path())
        .arg("tag")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("<COMMIT>"),
        "{output:?}"
    );
}

#[test]
fn tag_no_second_bump_and_exact_historical_commit() {
    let repo = prepared();
    let release = repo.git(&["rev-parse", "HEAD"]);
    repo.write(
        "project.toml",
        "[project]\nname = \"example\"\nversion = \"2.0.0\"\n",
    );
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "feat: later changes"]);
    let head = repo.git(&["rev-parse", "HEAD"]);
    let index = repo.git(&["write-tree"]);
    let changelog = repo.read("CHANGELOG.md");
    let output = Command::new(TestRepo::bin_path())
        .current_dir(repo.path())
        .args(["tag", &release])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(repo.git(&["rev-parse", "v1.0.1^{commit}"]), release);
    assert_eq!(repo.git(&["cat-file", "-t", "v1.0.1"]), "tag");
    assert_eq!(repo.git(&["rev-parse", "HEAD"]), head);
    assert_eq!(repo.git(&["write-tree"]), index);
    assert_eq!(repo.read("CHANGELOG.md"), changelog);
    assert!(repo.read("project.toml").contains("2.0.0"));
}

#[test]
fn tag_dry_run_preserves_repository() {
    let repo = prepared();
    let head = repo.git(&["rev-parse", "HEAD"]);
    let refs = repo.git(&["show-ref"]);
    let output = Command::new(TestRepo::bin_path())
        .current_dir(repo.path())
        .args(["tag", &head, "--dry-run", "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["target_oid"], head);
    assert_eq!(value["tags"][0]["name"], "v1.0.1");
    assert_eq!(repo.git(&["show-ref"]), refs);
    assert_eq!(repo.git(&["status", "--porcelain"]), "");
}

#[test]
fn two_step_release_tags_rewritten_merge_commit() {
    for strategy in ["squash", "rebase"] {
        let repo = TestRepo::new().with_project_toml("1.0.0");
        repo.git(&["checkout", "-B", "main"]);
        repo.git(&["add", "."]);
        repo.git(&["commit", "-m", "chore: initial"]);
        repo.create_tag("v1.0.0");
        repo.write("feature.txt", "new");
        repo.git(&["add", "."]);
        repo.git(&["commit", "-m", "fix: feature"]);
        repo.git(&["checkout", "-b", "release/1.0.1"]);
        let out = Command::new(TestRepo::bin_path())
            .current_dir(repo.path())
            .args(["bump", "--yes", "--no-tag"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        let before = repo.git(&["rev-parse", "HEAD"]);
        repo.git(&["checkout", "main"]);
        repo.write("later.txt", "independent");
        repo.git(&["add", "."]);
        repo.git(&["commit", "-m", "docs: independent"]);
        if strategy == "squash" {
            repo.git(&["merge", "--squash", "release/1.0.1"]);
            repo.git(&["commit", "-m", "chore(release): 1.0.1 (#592)"]);
        } else {
            repo.git(&["checkout", "release/1.0.1"]);
            repo.git(&["rebase", "main"]);
            repo.git(&["checkout", "main"]);
            repo.git(&["merge", "--ff-only", "release/1.0.1"]);
        }
        let after = repo.git(&["rev-parse", "HEAD"]);
        assert_ne!(before, after);
        let out = Command::new(TestRepo::bin_path())
            .current_dir(repo.path())
            .args(["tag", &after])
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        assert_eq!(repo.git(&["rev-parse", "v1.0.1^{commit}"]), after);
        assert_eq!(repo.git(&["rev-parse", "HEAD"]), after);
    }
}
