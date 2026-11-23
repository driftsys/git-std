use std::path::Path;

use assert_cmd::Command;

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

fn init_repo(path: &Path, files: &[&str], hook_line: &str) {
    git(path, &["init"]);
    git(path, &["config", "user.name", "Test"]);
    git(path, &["config", "user.email", "test@test.com"]);
    std::fs::create_dir_all(path.join("dir")).unwrap();
    for file in files {
        std::fs::write(path.join(file), format!("{file}\n")).unwrap();
        git(path, &["add", file]);
    }
    git(path, &["commit", "-m", "initial"]);
    let hooks_dir = path.join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(hooks_dir.join("pre-commit.hooks"), hook_line).unwrap();
}

fn run_hook(path: &Path) {
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(path)
        .assert()
        .success();
}

fn assert_clean_after_hook(path: &Path, before: &str) {
    assert_eq!(
        git(path, &["diff", "--cached", "--name-status"]),
        before,
        "staged renames must be unchanged"
    );
    assert_eq!(
        git(path, &["diff", "--name-only"]),
        "",
        "no unstaged changes"
    );
    assert_eq!(git(path, &["stash", "list"]), "", "no leftover stash");
}

/// #604 — A case-only rename survives the fix-mode stash dance.
///
/// On a case-insensitive filesystem, `dir/rules.md` and `dir/RULES.md` are
/// the same file on disk. The hook must keep the staged rename, leave no
/// stash entry behind, and succeed. On a case-sensitive filesystem the test
/// exercises the same flow without the case collision and passes as well.
/// The test does not set `core.ignorecase`; the fix must not depend on it.
#[test]
fn hooks_run_fix_mode_preserves_case_only_rename() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path, &["dir/rules.md"], "~ true\n");

    git(path, &["mv", "dir/rules.md", "dir/tmp.md"]);
    git(path, &["mv", "dir/tmp.md", "dir/RULES.md"]);
    let before = git(path, &["diff", "--cached", "--name-status"]);
    assert!(before.starts_with('R'), "expected staged rename: {before}");

    run_hook(path);
    assert_clean_after_hook(path, &before);
}

fn stage_case_only_rename(path: &Path) {
    git(path, &["mv", "dir/rules.md", "dir/tmp.md"]);
    git(path, &["mv", "dir/tmp.md", "dir/RULES.md"]);
}

const APPEND_FORMATTER: &str = "~ for f in \"$@\"; do echo 'formatted' >> \"$f\"; done\n";

/// #604 — A formatter that edits the case-renamed file is restaged.
#[test]
fn hooks_run_fix_mode_formats_case_only_rename() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path, &["dir/rules.md"], APPEND_FORMATTER);
    stage_case_only_rename(path);

    run_hook(path);

    let status = git(path, &["diff", "--cached", "--name-status"]);
    assert!(
        status.starts_with('R') && status.ends_with("dir/rules.md\tdir/RULES.md"),
        "rename must stay staged: {status}"
    );
    assert_eq!(git(path, &["diff", "--name-only"]), "");
    assert_eq!(git(path, &["stash", "list"]), "");
    let blob = git(path, &["show", ":dir/RULES.md"]);
    assert!(blob.ends_with("formatted"), "blob not formatted: {blob}");
}

/// #604 — A real `git commit` through the installed hook succeeds.
#[test]
fn hooks_run_fix_mode_commits_case_only_rename() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path, &["dir/rules.md"], APPEND_FORMATTER);
    let bin = assert_cmd::cargo::cargo_bin("git-std");
    let shim_path = path.join(".githooks/pre-commit");
    std::fs::write(
        &shim_path,
        format!(
            "#!/bin/sh\nexec \"{}\" hook run pre-commit -- \"$@\"\n",
            bin.display()
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&shim_path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    git(path, &["config", "core.hooksPath", ".githooks"]);
    stage_case_only_rename(path);

    git(path, &["commit", "-m", "chore: rename"]);

    assert_eq!(git(path, &["stash", "list"]), "");
    assert_eq!(
        git(path, &["status", "--porcelain", "--untracked-files=no"]),
        ""
    );
    let blob = git(path, &["show", "HEAD:dir/RULES.md"]);
    assert!(blob.ends_with("formatted"), "blob not formatted: {blob}");
}

/// #604 — A swap of two names that differ in case also survives.
#[test]
fn hooks_run_fix_mode_preserves_case_swapping_renames() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path, &["dir/a.txt", "dir/b.txt"], "~ true\n");

    // On a case-sensitive filesystem `DIR` is a separate directory that must exist.
    std::fs::create_dir_all(path.join("DIR")).unwrap();
    git(path, &["mv", "dir/a.txt", "dir/tmp.txt"]);
    git(path, &["mv", "dir/b.txt", "DIR/a.txt"]);
    git(path, &["mv", "dir/tmp.txt", "DIR/b.txt"]);
    let before = git(path, &["diff", "--cached", "--name-status"]);
    assert!(before.starts_with('R'), "expected staged renames: {before}");

    run_hook(path);
    assert_clean_after_hook(path, &before);
}
