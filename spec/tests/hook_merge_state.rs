#[path = "../support/mod.rs"]
mod support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use support::TestRepo;

const MERGE_FILES: [&str; 7] = [
    "MERGE_HEAD",
    "MERGE_MSG",
    "MERGE_MODE",
    "AUTO_MERGE",
    "MERGE_AUTOSTASH",
    "MERGE_RR",
    "SQUASH_MSG",
];

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn git_dir(root: &Path) -> PathBuf {
    PathBuf::from(git(root, &["rev-parse", "--absolute-git-dir"]))
}

fn metadata(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let directory = git_dir(root);
    MERGE_FILES
        .into_iter()
        .filter_map(|name| {
            let path = directory.join(name);
            path.exists()
                .then(|| (name.to_string(), std::fs::read(path).unwrap()))
        })
        .collect()
}

fn fixture(hook: &str) -> TestRepo {
    let repo = TestRepo::new().with_hooks_file("pre-commit", hook);
    repo.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);
    repo.git(&["config", "core.hooksPath", ".disabled-hooks"]);
    repo.git(&["config", "commit.gpgsign", "false"]);
    repo.write("shared.txt", "base\n");
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "chore: base"]);
    repo.git(&["switch", "-c", "feature"]);
    repo.write("feature.txt", "feature\n");
    repo.git(&["add", "feature.txt"]);
    repo.git(&["commit", "-m", "feat: feature"]);
    repo.git(&["switch", "main"]);
    repo.write("main.txt", "main\n");
    repo.git(&["add", "main.txt"]);
    repo.git(&["commit", "-m", "feat: main"]);
    repo
}

fn run_hook(root: &Path) -> Output {
    Command::new(TestRepo::bin_path())
        .args(["hook", "run", "pre-commit", "--format", "json"])
        .current_dir(root)
        .output()
        .unwrap()
}

fn assert_merge_commit(root: &Path, parents: &[String; 2]) {
    git(root, &["commit", "-m", "feat: merge"]);
    assert_eq!(
        git(root, &["show", "-s", "--format=%P", "HEAD"]),
        parents.join(" ")
    );
}

fn parents(root: &Path) -> [String; 2] {
    [
        git(root, &["rev-parse", "HEAD"]),
        git(root, &["rev-parse", "feature"]),
    ]
}

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
    let repo = fixture("~ true\n");
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
    let main_before = metadata(repo.path());
    let main_staged = repo.git(&["diff", "--cached", "--binary"]);
    let before = metadata(&checkout);
    let output = run_hook(&checkout);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(metadata(&checkout), before);
    assert_eq!(metadata(repo.path()), main_before);
    assert_eq!(repo.git(&["diff", "--cached", "--binary"]), main_staged);
    assert_merge_commit(&checkout, &expected_parents);
    assert_eq!(metadata(repo.path()), main_before);
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
fn run_with_git_failure(root: &Path, operation: &str) -> Output {
    use std::os::unix::fs::PermissionsExt;
    let wrappers = tempfile::tempdir().unwrap();
    let wrapper = wrappers.path().join("git");
    std::fs::copy(
        concat!(env!("CARGO_MANIFEST_DIR"), "/hooks/fixtures/git"),
        &wrapper,
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
    let real_git = Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .unwrap();
    assert!(real_git.status.success());
    let path = std::env::join_paths(
        std::iter::once(wrappers.path().to_path_buf())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    Command::new(TestRepo::bin_path())
        .args(["hook", "run", "pre-commit", "--format", "json"])
        .env("PATH", path)
        .env(
            "REAL_GIT",
            String::from_utf8(real_git.stdout).unwrap().trim(),
        )
        .env("FAIL_GIT_OPERATION", operation)
        .current_dir(root)
        .output()
        .unwrap()
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
