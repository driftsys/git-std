#[path = "../support/merge_fixture.rs"]
mod merge_fixture;
#[path = "../support/mod.rs"]
mod support;

use merge_fixture::*;

use std::process::Command;

#[test]
fn reftable_preserves_conflict_tree_and_merge_parents() {
    for hook in ["~ true\n", "~ false\n"] {
        let repo = fixture(hook);
        repo.git(&["refs", "migrate", "--ref-format=reftable"]);
        repo.git(&["checkout", "feature"]);
        repo.write("shared.txt", "feature conflict\n");
        repo.git(&["commit", "-am", "feat: feature conflict"]);
        repo.git(&["checkout", "main"]);
        repo.write("shared.txt", "main conflict\n");
        repo.git(&["commit", "-am", "feat: main conflict"]);
        let expected_parents = parents(repo.path());
        let merge = Command::new("git")
            .args(["merge", "--no-commit", "feature"])
            .current_dir(repo.path())
            .output()
            .unwrap();
        assert_eq!(merge.status.code(), Some(1));
        let original = repo.git(&["rev-parse", "AUTO_MERGE"]);
        assert!(!git_dir(repo.path()).join("AUTO_MERGE").exists());
        repo.write("shared.txt", "resolved\n");
        repo.git(&["add", "shared.txt"]);
        let output = run_hook(repo.path());
        assert_eq!(
            output.status.code(),
            Some(if hook.contains("false") { 1 } else { 0 }),
            "{output:?}"
        );
        assert_eq!(repo.git(&["rev-parse", "AUTO_MERGE"]), original);
        assert_merge_commit(repo.path(), &expected_parents);
    }
}

#[test]
fn reftable_preserves_merge_autostash_until_merge_commit() {
    let repo = fixture("~ true\n");
    repo.git(&["refs", "migrate", "--ref-format=reftable"]);
    let expected_parents = parents(repo.path());
    repo.write("shared.txt", "autostashed\n");
    repo.git(&["merge", "--no-commit", "--autostash", "feature"]);
    let original = repo.git(&["rev-parse", "MERGE_AUTOSTASH"]);
    assert!(!git_dir(repo.path()).join("MERGE_AUTOSTASH").exists());
    let stash_before = repo.git(&["stash", "list", "--format=%H"]);
    let output = run_hook(repo.path());
    assert!(output.status.success(), "{output:?}");
    assert_eq!(repo.git(&["rev-parse", "MERGE_AUTOSTASH"]), original);
    assert_eq!(repo.git(&["stash", "list", "--format=%H"]), stash_before);
    assert_eq!(repo.read("shared.txt"), "base\n");
    assert_merge_commit(repo.path(), &expected_parents);
    assert_eq!(repo.read("shared.txt"), "autostashed\n");
}

#[cfg(unix)]
#[test]
fn reftable_capture_failure_rolls_back_earlier_ref_removal() {
    let repo = fixture("~ touch hook-ran\n");
    repo.git(&["refs", "migrate", "--ref-format=reftable"]);
    repo.write("shared.txt", "autostashed\n");
    repo.git(&["merge", "--no-commit", "--autostash", "feature"]);
    let auto_merge = repo.git(&["rev-parse", "AUTO_MERGE"]);
    let autostash = repo.git(&["rev-parse", "MERGE_AUTOSTASH"]);
    let before = metadata(repo.path());
    let output = run_with_git_failure(repo.path(), "delete-merge-autostash");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert_eq!(repo.git(&["rev-parse", "AUTO_MERGE"]), auto_merge);
    assert_eq!(repo.git(&["rev-parse", "MERGE_AUTOSTASH"]), autostash);
    assert_eq!(metadata(repo.path()), before);
    assert_eq!(repo.read("shared.txt"), "base\n");
    assert!(!repo.path().join("hook-ran").exists());
}

#[cfg(unix)]
#[test]
fn reftable_restoration_conflict_retains_original_oid_and_reports_path() {
    let repo = fixture("~ true\n");
    repo.git(&["refs", "migrate", "--ref-format=reftable"]);
    repo.write("shared.txt", "autostashed\n");
    repo.git(&["merge", "--no-commit", "--autostash", "feature"]);
    let original = repo.git(&["rev-parse", "MERGE_AUTOSTASH"]);
    let output = run_with_git_failure(repo.path(), "stash-apply-ref-collision");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let message = document["diagnostics"][0]["message"].as_str().unwrap();
    let recovery = std::fs::read_dir(git_dir(repo.path()))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("git-std-merge-refs-")
        })
        .expect("retained reference recovery directory");
    assert!(
        message.contains("refusing to replace merge reference MERGE_AUTOSTASH"),
        "{message}"
    );
    assert!(message.contains(recovery.to_str().unwrap()), "{message}");
    assert_eq!(
        std::fs::read_to_string(recovery.join("MERGE_AUTOSTASH")).unwrap(),
        format!("{original}\n")
    );
    assert_eq!(
        repo.git(&["rev-parse", "MERGE_AUTOSTASH"]),
        repo.git(&["rev-parse", "HEAD"])
    );
}

#[cfg(unix)]
#[test]
fn reftable_failed_apply_removes_generated_ref_when_original_was_absent() {
    let repo = fixture("~ true\n");
    repo.git(&["refs", "migrate", "--ref-format=reftable"]);
    repo.git(&["merge", "--no-commit", "feature"]);
    repo.git(&["update-ref", "-d", "AUTO_MERGE"]);
    let output = run_with_git_failure(repo.path(), "stash-apply-generate-auto");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let absent = Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", "AUTO_MERGE"])
        .current_dir(repo.path())
        .status()
        .unwrap();
    assert_eq!(absent.code(), Some(1));
    assert!(metadata(repo.path()).contains_key("MERGE_HEAD"));
}

#[cfg(unix)]
#[test]
fn reftable_failed_apply_restores_the_existing_auto_merge_ref() {
    let repo = fixture("~ true\n");
    repo.git(&["refs", "migrate", "--ref-format=reftable"]);
    repo.git(&["merge", "--no-commit", "feature"]);
    let original = repo.git(&["rev-parse", "AUTO_MERGE"]);
    assert_ne!(original, repo.git(&["rev-parse", "HEAD^{tree}"]));
    let output = run_with_git_failure(repo.path(), "stash-apply-generate-auto");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert_eq!(repo.git(&["rev-parse", "AUTO_MERGE"]), original);
}

#[cfg(unix)]
#[test]
fn reftable_capture_and_restore_refuse_updates_racing_with_observation() {
    for operation in ["capture-merge-ref-race", "restore-merge-ref-race"] {
        let repo = fixture("~ true\n");
        repo.git(&["refs", "migrate", "--ref-format=reftable"]);
        repo.git(&["merge", "--no-commit", "feature"]);
        let original = repo.git(&["rev-parse", "AUTO_MERGE"]);
        let concurrent = repo.git(&["rev-parse", "HEAD^{tree}"]);
        assert_ne!(original, concurrent);
        let output = run_with_git_failure(repo.path(), operation);
        assert_eq!(output.status.code(), Some(2), "{operation}: {output:?}");
        assert_eq!(repo.git(&["rev-parse", "AUTO_MERGE"]), concurrent);
        let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let message = document["diagnostics"][0]["message"].as_str().unwrap();
        assert!(
            message.contains("failed to update merge reference AUTO_MERGE"),
            "{message}"
        );
        let recovery = std::fs::read_dir(git_dir(repo.path()))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("git-std-merge-refs-")
            })
            .expect("retained reference recovery directory");
        assert!(message.contains(recovery.to_str().unwrap()), "{message}");
        assert_eq!(
            std::fs::read_to_string(recovery.join("AUTO_MERGE")).unwrap(),
            format!("{original}\n")
        );
    }
}
