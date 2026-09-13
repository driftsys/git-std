#[path = "../support/mod.rs"]
mod support;

use serde_json::Value;
use snapbox::cmd::Command;
use support::TestRepo;

#[cfg(unix)]
fn run_with_git_failure(repo: &TestRepo, operation: &str) -> std::process::Output {
    use std::os::unix::fs::PermissionsExt;

    let wrapper_dir = tempfile::tempdir().expect("git wrapper directory");
    let wrapper = wrapper_dir.path().join("git");
    std::fs::copy(
        concat!(env!("CARGO_MANIFEST_DIR"), "/hooks/fixtures/git"),
        &wrapper,
    )
    .expect("copy git wrapper");
    let mut permissions = std::fs::metadata(&wrapper)
        .expect("git wrapper metadata")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&wrapper, permissions).expect("executable git wrapper");
    let real_git = std::process::Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .expect("resolve git");
    assert!(real_git.status.success());
    let real_git = String::from_utf8(real_git.stdout)
        .expect("UTF-8 git path")
        .trim()
        .to_string();
    let path = std::env::join_paths(std::iter::once(wrapper_dir.path().to_path_buf()).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .expect("wrapper PATH");

    Command::new(TestRepo::bin_path())
        .args(["hook", "run", "pre-commit", "--format", "json"])
        .env("PATH", path)
        .env("REAL_GIT", real_git)
        .env("FAIL_GIT_OPERATION", operation)
        .env("GIT_WRAPPER_STATE", wrapper_dir.path().join("state"))
        .current_dir(repo.path())
        .output()
        .expect("hook run")
}

fn assert_hook_run_error(output: &std::process::Output, message: &str) {
    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stderr.is_empty(),
        "machine mode must keep stderr empty: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout must be one JSON document: {error}; stdout={}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    assert_eq!(document["status"], "error");
    assert_eq!(document["exit_code"], 2);
    assert_eq!(document["diagnostics"][0]["code"], "GITSTD-HOOK-RUN");
    assert!(
        document["diagnostics"][0]["message"]
            .as_str()
            .is_some_and(|actual| actual.contains(message)),
        "unexpected diagnostic: {document}"
    );
}

fn staged_diff(repo: &TestRepo) -> Vec<u8> {
    let output = std::process::Command::new("git")
        .args(["diff", "--cached", "--binary", "--no-ext-diff"])
        .current_dir(repo.path())
        .output()
        .expect("staged diff");
    assert!(output.status.success(), "git diff --cached");
    output.stdout
}

fn stash_head(repo: &TestRepo) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", "refs/stash"])
        .current_dir(repo.path())
        .output()
        .expect("stash query");
    match output.status.code() {
        Some(0) => Some(
            String::from_utf8(output.stdout)
                .expect("UTF-8 stash SHA")
                .trim()
                .to_string(),
        ),
        Some(1) => None,
        code => panic!("stash query failed with {code:?}"),
    }
}

#[test]
fn fix_mode_setup_failure_is_one_structured_json_document() {
    let mut submodule = TestRepo::new();
    submodule.add_commit("chore: initialize submodule");
    let repo = TestRepo::new().with_hooks_file("pre-commit", "~ true\n");
    let added = std::process::Command::new("git")
        .args([
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            submodule.path().to_str().expect("UTF-8 fixture path"),
            "submodule",
        ])
        .current_dir(repo.path())
        .output()
        .expect("git submodule add");
    assert!(
        added.status.success(),
        "git submodule add: {}",
        String::from_utf8_lossy(&added.stderr)
    );

    let output = Command::new(TestRepo::bin_path())
        .args(["hook", "run", "pre-commit", "--format", "json"])
        .current_dir(repo.path())
        .output()
        .expect("hook run");

    assert_hook_run_error(&output, "submodule entries");
}

#[cfg(unix)]
#[test]
fn rename_unstage_failure_is_structured_json() {
    let mut repo = TestRepo::new().with_hooks_file("pre-commit", "~ true\n");
    repo.add_commit("chore: initialize");
    repo.git(&["mv", "file-1.txt", "renamed.txt"]);

    let output = run_with_git_failure(&repo, "unstage");

    assert_hook_run_error(&output, "failed to unstage renames");
}

#[cfg(unix)]
#[test]
fn stash_apply_failure_is_structured_json() {
    let mut repo = TestRepo::new().with_hooks_file("pre-commit", "~ true\n");
    repo.add_commit("chore: initialize");
    repo.write("file-1.txt", "staged\n");
    repo.git(&["add", "file-1.txt"]);
    repo.write("file-1.txt", "unstaged after staged version\n");

    let output = run_with_git_failure(&repo, "stash-apply");

    assert_hook_run_error(&output, "stash apply failed");
}

#[cfg(unix)]
#[test]
fn fail_fast_cleanup_failure_is_structured_json() {
    let mut repo = TestRepo::new().with_hooks_file("pre-commit", "~ false\n");
    repo.add_commit("chore: initialize");
    repo.write("file-1.txt", "staged\n");
    repo.git(&["add", "file-1.txt"]);

    let output = run_with_git_failure(&repo, "add");

    assert_hook_run_error(&output, "formatted changes may be lost");
}

#[cfg(unix)]
#[test]
fn final_restage_failure_is_structured_json() {
    let mut repo = TestRepo::new().with_hooks_file("pre-commit", "~ true\n");
    repo.add_commit("chore: initialize");
    repo.write("file-1.txt", "staged\n");
    repo.git(&["add", "file-1.txt"]);

    let output = run_with_git_failure(&repo, "add");

    assert_hook_run_error(&output, "formatted changes may be lost");
}

#[cfg(unix)]
#[test]
fn stash_push_failure_is_structured_json() {
    let mut repo = TestRepo::new().with_hooks_file("pre-commit", "~ true\n");
    repo.add_commit("chore: initialize");
    repo.write("file-1.txt", "staged\n");
    repo.git(&["add", "file-1.txt"]);
    repo.write("file-1.txt", "unstaged after staged version\n");

    let output = run_with_git_failure(&repo, "stash-push");

    assert_hook_run_error(&output, "failed to protect unstaged changes");
}

#[cfg(unix)]
#[test]
fn deletion_query_failure_preserves_a_staged_rename() {
    let mut repo = TestRepo::new().with_hooks_file("pre-commit", "~ true\n");
    repo.add_commit("chore: initialize");
    repo.git(&["mv", "file-1.txt", "renamed.txt"]);
    let before = staged_diff(&repo);

    let output = run_with_git_failure(&repo, "deletion-query");

    assert_hook_run_error(&output, "failed to inspect staged files");
    assert_eq!(
        staged_diff(&repo),
        before,
        "staged rename changed on failure"
    );
}

#[cfg(unix)]
#[test]
fn stash_push_failure_preserves_a_staged_rename() {
    let mut repo = TestRepo::new().with_hooks_file("pre-commit", "~ true\n");
    repo.add_commit("chore: initialize");
    repo.git(&["mv", "file-1.txt", "renamed.txt"]);
    let before = staged_diff(&repo);

    let output = run_with_git_failure(&repo, "stash-push");

    assert_hook_run_error(&output, "failed to protect unstaged changes");
    assert_eq!(
        staged_diff(&repo),
        before,
        "staged rename changed on failure"
    );
}

#[cfg(unix)]
#[test]
fn setup_failure_reports_the_original_and_rename_rollback_failures() {
    let mut repo = TestRepo::new().with_hooks_file("pre-commit", "~ true\n");
    repo.add_commit("chore: initialize");
    repo.git(&["mv", "file-1.txt", "renamed.txt"]);

    let output = run_with_git_failure(&repo, "stash-push-and-rollback");

    assert_hook_run_error(&output, "failed to protect unstaged changes");
    let document: Value = serde_json::from_slice(&output.stdout).expect("error document");
    assert!(
        document["diagnostics"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("failed to restore staged renames")),
        "rollback failure missing: {document}"
    );
}

#[cfg(unix)]
#[test]
fn stash_drop_failure_is_structured_json() {
    let mut repo = TestRepo::new().with_hooks_file("pre-commit", "~ true\n");
    repo.add_commit("chore: initialize");
    repo.write("file-1.txt", "staged\n");
    repo.git(&["add", "file-1.txt"]);
    repo.write("file-1.txt", "unstaged after staged version\n");

    let output = run_with_git_failure(&repo, "stash-drop");

    assert_hook_run_error(&output, "failed to drop fix-mode stash");
}

#[cfg(unix)]
#[test]
fn deletion_restage_failure_is_structured_json() {
    let mut repo = TestRepo::new().with_hooks_file("pre-commit", "~ true\n");
    repo.add_commit("chore: initialize");
    repo.git(&["rm", "file-1.txt"]);

    let output = run_with_git_failure(&repo, "update-index");

    assert_hook_run_error(&output, "staged deletions may be lost");
}

#[cfg(unix)]
#[test]
fn rename_restage_failure_is_structured_json() {
    let mut repo = TestRepo::new().with_hooks_file("pre-commit", "~ true\n");
    repo.add_commit("chore: initialize");
    repo.git(&["mv", "file-1.txt", "renamed.txt"]);

    let output = run_with_git_failure(&repo, "second-add");

    assert_hook_run_error(&output, "failed to re-stage renamed files");
}

#[cfg(unix)]
#[test]
fn staged_file_query_failure_is_structured_json() {
    let mut repo = TestRepo::new().with_hooks_file("pre-commit", "~ true\n");
    repo.add_commit("chore: initialize");
    repo.write("file-1.txt", "staged\n");
    repo.git(&["add", "file-1.txt"]);

    let output = run_with_git_failure(&repo, "staged-query");

    assert_hook_run_error(&output, "failed to inspect staged files");
}

#[cfg(unix)]
#[test]
fn unstaged_query_failure_cleans_up_stash_and_preserves_rename() {
    let mut repo = TestRepo::new().with_hooks_file("pre-commit", "~ true\n");
    repo.add_commit("chore: initialize");
    repo.git(&["mv", "file-1.txt", "renamed.txt"]);
    repo.write("unstaged.txt", "protect me\n");
    let before_diff = staged_diff(&repo);
    let before_stash = stash_head(&repo);

    let output = run_with_git_failure(&repo, "unstaged-query");

    assert_hook_run_error(&output, "failed to inspect unstaged files");
    assert_eq!(staged_diff(&repo), before_diff);
    assert_eq!(stash_head(&repo), before_stash);
    assert_eq!(repo.read("unstaged.txt"), "protect me\n");
}
