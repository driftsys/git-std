#[path = "../support/merge_fixture.rs"]
mod merge_fixture;
#[path = "../support/mod.rs"]
mod support;

use merge_fixture::*;

#[test]
fn successful_fix_hook_preserves_merge_metadata_and_both_parents() {
    let repo = fixture("~ true\n");
    let expected_parents = parents(repo.path());
    repo.git(&["merge", "--no-commit", "feature"]);
    let before = metadata(repo.path());
    assert!(before.contains_key("MERGE_HEAD"));
    let staged = repo.git(&["diff", "--cached", "--binary"]);
    let output = run_hook(repo.path());
    assert!(output.status.success(), "{output:?}");
    assert_eq!(metadata(repo.path()), before);
    assert_eq!(repo.git(&["diff", "--cached", "--binary"]), staged);
    assert_merge_commit(repo.path(), &expected_parents);
}

#[test]
fn linked_worktree_fix_preserves_its_merge_and_the_other_worktree() {
    for use_reftable in [false, true] {
        let repo = fixture("~ true\n");
        if use_reftable {
            repo.git(&["refs", "migrate", "--ref-format=reftable"]);
        }
        let linked = tempfile::tempdir().unwrap();
        let checkout = linked.path().join("checkout");
        repo.git(&[
            "worktree",
            "add",
            "-b",
            "merging",
            checkout.to_str().unwrap(),
            "main",
        ]);
        let expected_parents = parents(&checkout);
        repo.git(&["merge", "--no-commit", "feature"]);
        git(&checkout, &["merge", "--no-commit", "feature"]);
        std::fs::write(
            git_dir(repo.path()).join("MERGE_MSG"),
            "main worktree message\n",
        )
        .unwrap();
        repo.git(&["update-ref", "AUTO_MERGE", "HEAD^{tree}"]);
        let main_ref = repo.git(&["rev-parse", "AUTO_MERGE"]);
        let linked_ref = git(&checkout, &["rev-parse", "AUTO_MERGE"]);
        let main_before = metadata(repo.path());
        let main_staged = repo.git(&["diff", "--cached", "--binary"]);
        let before = metadata(&checkout);
        let output = run_hook(&checkout);
        assert!(output.status.success(), "{output:?}");
        assert_eq!(metadata(&checkout), before);
        assert_eq!(metadata(repo.path()), main_before);
        assert_eq!(repo.git(&["diff", "--cached", "--binary"]), main_staged);
        assert_eq!(repo.git(&["rev-parse", "AUTO_MERGE"]), main_ref);
        assert_eq!(git(&checkout, &["rev-parse", "AUTO_MERGE"]), linked_ref);
        assert_merge_commit(&checkout, &expected_parents);
        assert_eq!(metadata(repo.path()), main_before);
    }
}

#[test]
fn failed_fix_hook_preserves_merge_and_user_content() {
    let repo = fixture("~ false\n");
    let expected_parents = parents(repo.path());
    repo.git(&["merge", "--no-commit", "feature"]);
    repo.write("shared.txt", "unstaged\n");
    repo.write("local.txt", "untracked\n");
    let before = metadata(repo.path());
    let staged = repo.git(&["diff", "--cached", "--binary"]);
    let output = run_hook(repo.path());
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["status"], "finding");
    assert_eq!(metadata(repo.path()), before);
    assert_eq!(repo.git(&["diff", "--cached", "--binary"]), staged);
    assert_eq!(repo.read("shared.txt"), "unstaged\n");
    assert_eq!(repo.read("local.txt"), "untracked\n");
    assert_merge_commit(repo.path(), &expected_parents);
}

#[test]
fn merge_fix_restage_excludes_unstaged_and_untracked_content() {
    let repo = fixture("~ printf 'formatted\\n' > feature.txt\n");
    let expected_parents = parents(repo.path());
    repo.git(&["merge", "--no-commit", "feature"]);
    repo.write("shared.txt", "unstaged\n");
    repo.write("local.txt", "untracked\n");
    let before = metadata(repo.path());
    let output = run_hook(repo.path());
    assert!(output.status.success(), "{output:?}");
    assert_eq!(metadata(repo.path()), before);
    assert_eq!(repo.git(&["show", ":feature.txt"]), "formatted");
    assert_eq!(repo.git(&["show", ":shared.txt"]), "base");
    assert_eq!(repo.read("shared.txt"), "unstaged\n");
    assert_eq!(repo.read("local.txt"), "untracked\n");
    assert_merge_commit(repo.path(), &expected_parents);
    assert_eq!(repo.git(&["show", "HEAD:shared.txt"]), "base");
}

#[test]
fn fix_hook_preserves_merge_autostash_until_merge_commit() {
    let repo = fixture("~ true\n");
    let expected_parents = parents(repo.path());
    repo.write("shared.txt", "autostashed\n");
    repo.git(&["merge", "--no-commit", "--autostash", "feature"]);
    let before = metadata(repo.path());
    assert!(before.contains_key("MERGE_AUTOSTASH"));
    let stash_before = repo.git(&["stash", "list", "--format=%H"]);
    let output = run_hook(repo.path());
    assert!(output.status.success(), "{output:?}");
    assert_eq!(metadata(repo.path()), before);
    assert_eq!(repo.git(&["stash", "list", "--format=%H"]), stash_before);
    assert_eq!(repo.read("shared.txt"), "base\n");
    assert_merge_commit(repo.path(), &expected_parents);
    assert_eq!(repo.read("shared.txt"), "autostashed\n");
}

#[cfg(unix)]
#[test]
fn setup_failure_restores_merge_metadata_and_keeps_recoverable_content() {
    for operation in ["stash-push", "stash-apply"] {
        let repo = fixture("~ true\n");
        repo.git(&["merge", "--no-commit", "feature"]);
        repo.write("shared.txt", "unstaged\n");
        repo.write("local.txt", "untracked\n");
        let before = metadata(repo.path());
        let staged = repo.git(&["diff", "--cached", "--binary"]);
        let output = run_with_git_failure(repo.path(), operation);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(document["status"], "error");
        assert_eq!(document["diagnostics"][0]["code"], "GITSTD-HOOK-RUN");
        assert_eq!(metadata(repo.path()), before, "{operation}");
        if operation == "stash-push" {
            assert_eq!(repo.git(&["diff", "--cached", "--binary"]), staged);
            assert_eq!(repo.read("shared.txt"), "unstaged\n");
            assert_eq!(repo.read("local.txt"), "untracked\n");
        } else {
            assert_eq!(repo.git(&["show", "refs/stash:shared.txt"]), "unstaged");
            assert_eq!(repo.git(&["show", "refs/stash^2:feature.txt"]), "feature");
            assert_eq!(repo.git(&["show", "refs/stash^3:local.txt"]), "untracked");
        }
    }
}

#[cfg(unix)]
#[test]
fn restoration_failure_reports_the_recovery_directory_and_original_bytes() {
    let repo = fixture("~ touch hook-ran\n");
    repo.git(&["merge", "--no-commit", "feature"]);
    let original = metadata(repo.path())["MERGE_HEAD"].clone();
    let output = run_with_git_failure(repo.path(), "stash-apply-merge-collision");
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
                .starts_with("git-std-merge-state-")
        })
        .expect("retained recovery directory");
    assert!(message.contains("refusing to replace"), "{message}");
    assert!(message.contains(recovery.to_str().unwrap()), "{message}");
    assert_eq!(
        std::fs::read(recovery.join("MERGE_HEAD")).unwrap(),
        original
    );
    assert_eq!(
        std::fs::read(git_dir(repo.path()).join("MERGE_HEAD")).unwrap(),
        b"unexpected\n"
    );
    assert!(!repo.path().join("hook-ran").exists());
}
