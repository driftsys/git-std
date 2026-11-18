use crate::support::TestRepo;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const MERGE_FILES: [&str; 7] = [
    "MERGE_HEAD",
    "MERGE_MSG",
    "MERGE_MODE",
    "AUTO_MERGE",
    "MERGE_AUTOSTASH",
    "MERGE_RR",
    "SQUASH_MSG",
];

/// Run Git successfully in a fixture checkout.
pub(crate) fn git(root: &Path, args: &[&str]) -> String {
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

/// Locate the current worktree metadata directory.
pub(crate) fn git_dir(root: &Path) -> PathBuf {
    PathBuf::from(git(root, &["rev-parse", "--absolute-git-dir"]))
}

/// Read the loose merge metadata without changing it.
pub(crate) fn metadata(root: &Path) -> BTreeMap<String, Vec<u8>> {
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

/// Create divergent branches with a tracked pre-commit hook.
pub(crate) fn fixture(hook: &str) -> TestRepo {
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

/// Run the fixture pre-commit hook as a JSON command.
pub(crate) fn run_hook(root: &Path) -> Output {
    Command::new(TestRepo::bin_path())
        .args(["hook", "run", "pre-commit", "--format", "json"])
        .current_dir(root)
        .output()
        .unwrap()
}

/// Create a commit and verify its exact parent identities.
pub(crate) fn assert_merge_commit(root: &Path, parents: &[String; 2]) {
    git(root, &["commit", "-m", "feat: merge"]);
    assert_eq!(
        git(root, &["show", "-s", "--format=%P", "HEAD"]),
        parents.join(" ")
    );
}

/// Read the two intended merge parent identities.
pub(crate) fn parents(root: &Path) -> [String; 2] {
    [
        git(root, &["rev-parse", "HEAD"]),
        git(root, &["rev-parse", "feature"]),
    ]
}

#[cfg(unix)]
/// Run the hook with one controlled Git failure.
pub(crate) fn run_with_git_failure(root: &Path, operation: &str) -> Output {
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
