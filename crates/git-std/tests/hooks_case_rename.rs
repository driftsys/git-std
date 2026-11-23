use std::path::Path;

use assert_cmd::Command;

/// How `core.ignorecase` is set in the test repository.
///
/// `git init` on a case-insensitive filesystem writes `core.ignorecase=true`.
/// Issue #604 was reported with the setting absent, and `false` is a
/// third state, so each scenario runs with all three.
#[derive(Clone, Copy)]
enum IgnoreCase {
    /// Keep whatever `git init` wrote.
    Default,
    /// Remove the key.
    Unset,
    /// Set the key to `false`.
    False,
}

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

fn init_repo(path: &Path, ignore_case: IgnoreCase, files: &[&str], hook_line: &str) {
    git(path, &["init"]);
    git(path, &["config", "user.name", "Test"]);
    git(path, &["config", "user.email", "test@test.com"]);
    match ignore_case {
        IgnoreCase::Default => {}
        IgnoreCase::Unset => {
            // Exits non-zero when the key is absent; that is fine here.
            let _ = std::process::Command::new("git")
                .current_dir(path)
                .args(["config", "--unset", "core.ignorecase"])
                .status();
        }
        IgnoreCase::False => {
            git(path, &["config", "core.ignorecase", "false"]);
        }
    }
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

fn stage_case_only_rename(path: &Path) {
    git(path, &["mv", "dir/rules.md", "dir/tmp.md"]);
    git(path, &["mv", "dir/tmp.md", "dir/RULES.md"]);
}

const APPEND_FORMATTER: &str = "~ for f in \"$@\"; do echo 'formatted' >> \"$f\"; done\n";

/// Generate one test per `core.ignorecase` state for a scenario function.
macro_rules! for_each_ignore_case {
    ($scenario:ident, $default:ident, $unset:ident, $false_value:ident) => {
        #[test]
        fn $default() {
            $scenario(IgnoreCase::Default);
        }

        #[test]
        fn $unset() {
            $scenario(IgnoreCase::Unset);
        }

        #[test]
        fn $false_value() {
            $scenario(IgnoreCase::False);
        }
    };
}

/// #604 — A case-only rename survives the fix-mode stash dance.
///
/// On a case-insensitive filesystem, `dir/rules.md` and `dir/RULES.md` are
/// the same file on disk. The hook must keep the staged rename, leave no
/// stash entry behind, and succeed.
fn case_only_rename_survives(ignore_case: IgnoreCase) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path, ignore_case, &["dir/rules.md"], "~ true\n");
    stage_case_only_rename(path);
    let before = git(path, &["diff", "--cached", "--name-status"]);
    assert!(before.starts_with('R'), "expected staged rename: {before}");

    run_hook(path);
    assert_clean_after_hook(path, &before);
}
for_each_ignore_case!(
    case_only_rename_survives,
    case_only_rename_default_ignorecase,
    case_only_rename_unset_ignorecase,
    case_only_rename_false_ignorecase
);

/// #604 — A formatter that edits the case-renamed file is restaged.
fn formatter_edit_is_restaged(ignore_case: IgnoreCase) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path, ignore_case, &["dir/rules.md"], APPEND_FORMATTER);
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
for_each_ignore_case!(
    formatter_edit_is_restaged,
    formatted_case_only_rename_default_ignorecase,
    formatted_case_only_rename_unset_ignorecase,
    formatted_case_only_rename_false_ignorecase
);

/// #604 — A real `git commit` through the installed hook succeeds.
fn commit_through_installed_hook(ignore_case: IgnoreCase) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path, ignore_case, &["dir/rules.md"], APPEND_FORMATTER);
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
    assert_eq!(
        git(path, &["ls-tree", "--name-only", "HEAD", "dir/"]),
        "dir/RULES.md"
    );
    let blob = git(path, &["show", "HEAD:dir/RULES.md"]);
    assert!(blob.ends_with("formatted"), "blob not formatted: {blob}");
}
for_each_ignore_case!(
    commit_through_installed_hook,
    commit_case_only_rename_default_ignorecase,
    commit_case_only_rename_unset_ignorecase,
    commit_case_only_rename_false_ignorecase
);

/// #604 — A swap of two names that differ in case also survives
/// (`a.txt` to `B.txt` together with `b.txt` to `A.txt`).
fn swapping_renames_survive(ignore_case: IgnoreCase) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path, ignore_case, &["dir/a.txt", "dir/b.txt"], "~ true\n");

    git(path, &["mv", "dir/a.txt", "dir/tmp.txt"]);
    git(path, &["mv", "dir/b.txt", "dir/A.txt"]);
    git(path, &["mv", "dir/tmp.txt", "dir/B.txt"]);
    let before = git(path, &["diff", "--cached", "--name-status"]);
    assert!(before.starts_with('R'), "expected staged renames: {before}");

    run_hook(path);
    assert_clean_after_hook(path, &before);
}
for_each_ignore_case!(
    swapping_renames_survive,
    case_swapping_renames_default_ignorecase,
    case_swapping_renames_unset_ignorecase,
    case_swapping_renames_false_ignorecase
);

/// #604 — The reverse direction (`RULES.md` to `rules.md`) also survives.
fn upper_to_lower_rename_survives(ignore_case: IgnoreCase) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path, ignore_case, &["dir/RULES.md"], "~ true\n");

    git(path, &["mv", "dir/RULES.md", "dir/tmp.md"]);
    git(path, &["mv", "dir/tmp.md", "dir/rules.md"]);
    let before = git(path, &["diff", "--cached", "--name-status"]);
    assert!(before.starts_with('R'), "expected staged rename: {before}");

    run_hook(path);
    assert_clean_after_hook(path, &before);
}
for_each_ignore_case!(
    upper_to_lower_rename_survives,
    upper_to_lower_rename_default_ignorecase,
    upper_to_lower_rename_unset_ignorecase,
    upper_to_lower_rename_false_ignorecase
);

/// #604 — A case-only rename together with an ordinary rename.
fn mixed_renames_survive(ignore_case: IgnoreCase) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(
        path,
        ignore_case,
        &["dir/rules.md", "dir/old.txt"],
        "~ true\n",
    );
    stage_case_only_rename(path);
    git(path, &["mv", "dir/old.txt", "dir/new.txt"]);
    let before = git(path, &["diff", "--cached", "--name-status"]);
    assert_eq!(before.lines().count(), 2, "expected two renames: {before}");

    run_hook(path);
    assert_clean_after_hook(path, &before);
}
for_each_ignore_case!(
    mixed_renames_survive,
    mixed_renames_default_ignorecase,
    mixed_renames_unset_ignorecase,
    mixed_renames_false_ignorecase
);

/// #604 — The hook keeps a colliding rename target staged while it runs the
/// commands, on any filesystem.
///
/// On a case-sensitive filesystem there is no real collision, so the other
/// tests pass without the fix. This one observes the index while a command
/// runs: a target that stays staged shows as `A  dir/RULES.md`, while an
/// unstaged target comes back from the stash as `?? dir/RULES.md`.
#[test]
fn colliding_rename_target_stays_staged_while_commands_run() {
    let dir = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    let record = out_dir.path().join("status.txt");
    let hook_line = format!("~ git status --short > {}\n", record.display());
    init_repo(
        dir.path(),
        IgnoreCase::Default,
        &["dir/rules.md"],
        &hook_line,
    );
    stage_case_only_rename(dir.path());

    run_hook(dir.path());

    let seen = std::fs::read_to_string(&record).unwrap();
    assert!(
        seen.lines()
            .any(|line| line.starts_with("A ") && line.ends_with("dir/RULES.md")),
        "target must stay staged while commands run, saw:\n{seen}"
    );
}

/// #387 — An ordinary rename is still unstaged while the commands run, so
/// its target comes back from the stash as an untracked file.
#[test]
fn ordinary_rename_target_is_unstaged_while_commands_run() {
    let dir = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    let record = out_dir.path().join("status.txt");
    let hook_line = format!("~ git status --short > {}\n", record.display());
    init_repo(
        dir.path(),
        IgnoreCase::Default,
        &["dir/old.txt"],
        &hook_line,
    );
    git(dir.path(), &["mv", "dir/old.txt", "dir/new.txt"]);

    run_hook(dir.path());

    let seen = std::fs::read_to_string(&record).unwrap();
    assert!(
        seen.lines()
            .any(|line| line.starts_with("??") && line.ends_with("dir/new.txt")),
        "ordinary target must be unstaged while commands run, saw:\n{seen}"
    );
    assert_eq!(
        git(dir.path(), &["diff", "--cached", "--name-status"]),
        "R100\tdir/old.txt\tdir/new.txt"
    );
}
