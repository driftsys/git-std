use std::path::Path;

use assert_cmd::Command;

#[test]
fn matching_glob_passes_all_staged_acmr_files() {
    let repo = tempfile::tempdir().expect("repository");
    init_repo(repo.path());

    std::fs::write(repo.path().join("modified.txt"), "original\n").expect("modified fixture");
    std::fs::write(repo.path().join("original.md"), "rename me\n").expect("rename fixture");
    git(repo.path(), &["add", "modified.txt", "original.md"]);
    git(repo.path(), &["commit", "-m", "chore: initialize"]);

    std::fs::write(repo.path().join("match.rs"), "fn main() {}\n").expect("matching addition");
    std::fs::write(repo.path().join("modified.txt"), "changed\n").expect("staged modification");
    git(repo.path(), &["mv", "original.md", "renamed.md"]);
    git(repo.path(), &["add", "match.rs", "modified.txt"]);

    let hooks_dir = repo.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).expect("hooks directory");
    std::fs::write(
        hooks_dir.join("pre-commit.hooks"),
        "printf '%s\\n' \"$@\" > received-files   *.rs\n",
    )
    .expect("pre-commit commands");

    run_pre_commit(repo.path());

    assert_eq!(
        std::fs::read_to_string(repo.path().join("received-files")).expect("received files"),
        "match.rs\nmodified.txt\nrenamed.md\n"
    );
}

#[test]
fn nonmatching_staged_files_skip_command_despite_unstaged_match() {
    let repo = tempfile::tempdir().expect("repository");
    init_repo(repo.path());

    std::fs::write(repo.path().join("tracked.rs"), "fn tracked() {}\n").expect("tracked fixture");
    std::fs::write(repo.path().join("staged.txt"), "original\n").expect("staged fixture");
    git(repo.path(), &["add", "tracked.rs", "staged.txt"]);
    git(repo.path(), &["commit", "-m", "chore: initialize"]);

    std::fs::write(
        repo.path().join("tracked.rs"),
        "fn tracked() { println!(\"unstaged\"); }\n",
    )
    .expect("unstaged matching change");
    std::fs::write(repo.path().join("staged.txt"), "changed\n").expect("nonmatching change");
    git(repo.path(), &["add", "staged.txt"]);

    let hooks_dir = repo.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).expect("hooks directory");
    std::fs::write(
        hooks_dir.join("pre-commit.hooks"),
        "touch should-not-run   *.rs\n",
    )
    .expect("pre-commit commands");

    run_pre_commit(repo.path());

    assert!(
        !repo.path().join("should-not-run").exists(),
        "an unstaged Rust change must not satisfy the pre-commit glob"
    );
}

fn init_repo(path: &Path) {
    git(path, &["init"]);
    git(path, &["config", "user.name", "Test"]);
    git(path, &["config", "user.email", "test@test.com"]);
}

fn run_pre_commit(path: &Path) {
    Command::cargo_bin("git-std")
        .expect("git-std binary")
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(path)
        .assert()
        .success();
}

fn git(path: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .expect("git command");
    assert!(
        output.status.success(),
        "git {args:?} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
