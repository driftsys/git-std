//! #609 — A staged rename whose index path changes only the case of a
//! directory, while the directory on disk keeps its old case.
//!
//! `git mv dir/a.txt DIR/c.txt` leaves `DIR/c.txt` in the index and the file
//! in `dir/` on disk when `dir/` already exists on a case-insensitive
//! filesystem. The fix-mode stash cannot hold that state, so the hook stops
//! before it changes anything. On a case-sensitive filesystem `DIR/` is a
//! separate directory and the hook succeeds.

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

/// `core.ignorecase` states the issue names. `git init` on a case-insensitive
/// filesystem writes `true`, which Git handles by itself.
#[derive(Clone, Copy)]
enum IgnoreCase {
    Unset,
    False,
}

/// Create a repository with `dir/a.txt` and `dir/b.txt` and a fix-mode hook.
///
/// Returns `true` when `DIR` already names `dir`, which is a case-insensitive
/// filesystem. Otherwise it creates `DIR`, because `git mv` needs the target
/// directory there.
fn init_repo(path: &Path, ignore_case: IgnoreCase) -> bool {
    git(path, &["init"]);
    git(path, &["config", "user.name", "Test"]);
    git(path, &["config", "user.email", "test@test.com"]);
    match ignore_case {
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
    for file in ["dir/a.txt", "dir/b.txt"] {
        std::fs::write(path.join(file), format!("{file}\n")).unwrap();
        git(path, &["add", file]);
    }
    git(path, &["commit", "-m", "initial"]);
    let hooks_dir = path.join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "~ true\n").unwrap();
    let is_case_insensitive = path.join("DIR").is_dir();
    if !is_case_insensitive {
        std::fs::create_dir(path.join("DIR")).unwrap();
    }
    is_case_insensitive
}

/// Names and content of every file under `dir/` or `DIR/`, read from the
/// directory listing so that the exact on-disk case shows.
fn work_tree_files(path: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    for directory in ["dir", "DIR"] {
        let Ok(entries) = std::fs::read_dir(path.join(directory)) else {
            continue;
        };
        for entry in entries.map(Result::unwrap) {
            let content = std::fs::read_to_string(entry.path()).unwrap();
            files.push((
                entry.file_name().to_string_lossy().into_owned(),
                content.replace('\n', "|"),
            ));
        }
    }
    files.sort();
    files.dedup();
    files
}

/// Everything the hook must leave alone when it stops early.
#[derive(PartialEq, Debug)]
struct Snapshot {
    renames: String,
    index: String,
    status: String,
    files: Vec<(String, String)>,
    stash: String,
}

fn snapshot(path: &Path) -> Snapshot {
    Snapshot {
        renames: git(path, &["diff", "--cached", "--name-status"]),
        index: git(path, &["ls-files", "--stage"]),
        status: git(path, &["status", "--porcelain"]),
        files: work_tree_files(path),
        stash: git(path, &["stash", "list"]),
    }
}

fn run_hook(path: &Path) -> assert_cmd::assert::Assert {
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(path)
        .assert()
}

/// The command of the hint line that contains `needle`, without the prefix.
fn printed_command(stderr: &str, needle: &str) -> String {
    let line = stderr
        .lines()
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no hint with {needle}: {stderr}"));
    line[line.find(needle).unwrap()..].to_string()
}

fn run_shell(path: &Path, command: &str) {
    let status = std::process::Command::new("sh")
        .current_dir(path)
        .args(["-c", command])
        .status()
        .unwrap();
    assert!(status.success(), "printed command failed: {command}");
}

/// Move the index entries under `dir/` to `DIR/` file by file, as the message
/// tells the user to, but keep the staged blobs so that nothing else is staged.
fn move_index_entries_to_new_case(path: &Path) {
    let listing = git(path, &["ls-files", "--stage", "--", "dir"]);
    let mut input = String::new();
    for line in listing.lines() {
        let (info, name) = line.split_once('\t').unwrap();
        let (mode_and_blob, _stage) = info.rsplit_once(' ').unwrap();
        let new_name = name.replacen("dir/", "DIR/", 1);
        input.push_str(&format!("{mode_and_blob} 0\t{new_name}\n"));
        input.push_str(&format!("0 {} 0\t{name}\n", "0".repeat(40)));
    }
    let mut child = std::process::Command::new("git")
        .current_dir(path)
        .args(["update-index", "--index-info"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(&mut child.stdin.take().unwrap(), input.as_bytes()).unwrap();
    assert!(child.wait().unwrap().success());
}

/// Run the `mv` command the message prints, move the index entries that still
/// have the old case, then run the hook again.
///
/// The index entries need a move only when the directory holds files that the
/// commit does not rename.
fn recover_and_rerun(path: &Path, before: &Snapshot, stderr: &str, dirty: bool) {
    run_shell(
        path,
        &printed_command(stderr, "mv -- 'dir' 'dir.git-std-tmp'"),
    );
    let has_old_case = git(path, &["ls-files", "--stage"]).contains("\tdir/");
    if has_old_case {
        move_index_entries_to_new_case(path);
    }

    if dirty {
        // The printed commands alone must not stage the edit or the new file.
        assert_eq!(git(path, &["diff", "--name-only"]), "DIR/b.txt");
        assert_eq!(
            git(path, &["ls-files", "--others", "--exclude-standard", "DIR"]),
            "DIR/untracked.txt",
            "the untracked file stays untracked"
        );
        assert_eq!(
            git(path, &["show", ":DIR/b.txt"]),
            "dir/b.txt",
            "the staged blob has the committed content"
        );
    }

    run_hook(path).success();

    assert_eq!(git(path, &["stash", "list"]), "", "no leftover stash");
    if dirty {
        // Fix mode stages edits of staged files (#607), so only the content
        // and the untracked file are checked after the hook.
        // The stash dance may bring the directory back under its old case,
        // so compare the names ignoring case.
        let untracked = git(path, &["ls-files", "--others", "--exclude-standard"]);
        assert!(
            untracked
                .lines()
                .any(|name| name.eq_ignore_ascii_case("dir/untracked.txt")),
            "the untracked file stays untracked: {untracked}"
        );
        assert_eq!(
            std::fs::read_to_string(path.join("DIR/b.txt")).unwrap(),
            "edited\n",
            "the edit stays in the work tree"
        );
    } else {
        assert_eq!(git(path, &["diff", "--name-only"]), "", "no unstaged edits");
    }
    let index = git(path, &["ls-files", "--stage"]);
    assert!(!index.contains("\tdir/"), "one case in the index: {index}");
    if !has_old_case {
        assert_eq!(index, before.index, "index preserved");
        assert_eq!(
            git(path, &["diff", "--cached", "--name-status"]),
            before.renames,
            "renames preserved"
        );
    }
}

/// Stage the renames, run the hook and check the outcome for the filesystem.
///
/// With `dirty`, `dir/b.txt` has an unstaged edit and `dir/untracked.txt` is an
/// untracked file, both unrelated to the renames.
fn run_scenario(ignore_case: IgnoreCase, stage: fn(&Path), mismatched_path: &str, dirty: bool) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    let is_case_insensitive = init_repo(path, ignore_case);
    stage(path);
    if dirty {
        std::fs::write(path.join("dir/b.txt"), "edited\n").unwrap();
        std::fs::write(path.join("dir/untracked.txt"), "new\n").unwrap();
    }
    let before = snapshot(path);
    assert!(
        before.renames.starts_with('R'),
        "expected staged renames: {}",
        before.renames
    );

    let assert = run_hook(path);

    if is_case_insensitive {
        let output = assert.failure().get_output().clone();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        assert!(
            stderr.contains("whose case differs on disk") && stderr.contains(mismatched_path),
            "message must name the path: {stderr}"
        );
        assert!(
            stderr.contains("mv -- 'dir' 'dir.git-std-tmp' && mv -- 'dir.git-std-tmp' 'DIR'"),
            "message must give the fix: {stderr}"
        );
        assert!(
            stderr.contains("if `git status` still lists files under `dir`")
                && stderr.contains("review `git diff --cached`"),
            "message must give the fix: {stderr}"
        );
        assert!(
            stderr.contains("GIT_STD_SKIP_HOOKS=1"),
            "skip hint: {stderr}"
        );
        assert_eq!(snapshot(path), before, "the hook must change nothing");
        recover_and_rerun(path, &before, &stderr, dirty);
    } else {
        assert.success();
        assert_eq!(git(path, &["stash", "list"]), "", "no leftover stash");
        assert_eq!(
            git(path, &["diff", "--cached", "--name-status"]),
            before.renames
        );
    }
}

fn swap(path: &Path) {
    git(path, &["mv", "dir/a.txt", "dir/tmp.txt"]);
    git(path, &["mv", "dir/b.txt", "DIR/a.txt"]);
    git(path, &["mv", "dir/tmp.txt", "DIR/b.txt"]);
}

fn same_name(path: &Path) {
    git(path, &["mv", "dir/a.txt", "dir/tmp.txt"]);
    git(path, &["mv", "dir/tmp.txt", "DIR/a.txt"]);
}

fn full_case(path: &Path) {
    git(path, &["mv", "dir/a.txt", "dir/tmp.txt"]);
    git(path, &["mv", "dir/tmp.txt", "DIR/A.txt"]);
}

/// `dir/a.txt` to `DIR/B.txt` together with `dir/b.txt` to `dir/A.txt`.
fn mixed(path: &Path) {
    git(path, &["mv", "dir/a.txt", "dir/tmp.txt"]);
    git(path, &["mv", "dir/b.txt", "dir/A.txt"]);
    git(path, &["mv", "dir/tmp.txt", "DIR/B.txt"]);
}

/// Generate one test per `core.ignorecase` state for a staging function.
macro_rules! for_each_state {
    ($stage:ident, $path:expr, $unset:ident, $false_value:ident) => {
        #[test]
        fn $unset() {
            run_scenario(IgnoreCase::Unset, $stage, $path, false);
        }

        #[test]
        fn $false_value() {
            run_scenario(IgnoreCase::False, $stage, $path, false);
        }
    };
}

for_each_state!(
    swap,
    "DIR/a.txt",
    swap_unset_ignorecase,
    swap_false_ignorecase
);
for_each_state!(
    same_name,
    "DIR/a.txt",
    same_name_unset_ignorecase,
    same_name_false_ignorecase
);
for_each_state!(
    full_case,
    "DIR/A.txt",
    full_case_unset_ignorecase,
    full_case_false_ignorecase
);
for_each_state!(
    mixed,
    "DIR/B.txt",
    mixed_unset_ignorecase,
    mixed_false_ignorecase
);

/// The recovery must not stage an unrelated edit or an untracked file.
#[test]
fn recovery_keeps_unrelated_changes_unstaged_unset_ignorecase() {
    run_scenario(IgnoreCase::Unset, same_name, "DIR/a.txt", true);
}

#[test]
fn recovery_keeps_unrelated_changes_unstaged_false_ignorecase() {
    run_scenario(IgnoreCase::False, same_name, "DIR/a.txt", true);
}

/// An existing `dir.git-std-tmp` is never the temporary name of the printed
/// command: `mv` would move `dir` into it.
fn existing_temporary_name_is_not_reused(ignore_case: IgnoreCase) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    if !init_repo(path, ignore_case) {
        return;
    }
    same_name(path);
    std::fs::create_dir(path.join("dir.git-std-tmp")).unwrap();
    std::fs::write(path.join("dir.git-std-tmp/keep.txt"), "keep\n").unwrap();

    let output = run_hook(path).failure().get_output().clone();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    assert!(
        stderr.contains("mv -- 'dir' 'dir.git-std-tmp-2' && mv -- 'dir.git-std-tmp-2' 'DIR'"),
        "another temporary name: {stderr}"
    );
    run_shell(
        path,
        &printed_command(&stderr, "mv -- 'dir' 'dir.git-std-tmp-2'"),
    );
    assert_eq!(
        std::fs::read_to_string(path.join("dir.git-std-tmp/keep.txt")).unwrap(),
        "keep\n",
        "the existing directory is untouched"
    );
    assert!(path.join("DIR/b.txt").exists());
    assert!(!path.join("dir.git-std-tmp-2").exists());
}

#[test]
fn existing_temporary_name_is_not_reused_unset_ignorecase() {
    existing_temporary_name_is_not_reused(IgnoreCase::Unset);
}

#[test]
fn existing_temporary_name_is_not_reused_false_ignorecase() {
    existing_temporary_name_is_not_reused(IgnoreCase::False);
}
