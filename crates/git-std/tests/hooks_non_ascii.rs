//! #606 — The fix-mode stash dance with non-ASCII, space and quote characters
//! in staged path names.
//!
//! Git quotes such names with octal escapes in its default output. The hook
//! must read raw names, pass them back to Git unchanged, and leave the staged
//! change exactly as the user staged it.

use std::path::Path;

use assert_cmd::Command;

/// Run Git with every path argument read as a literal name, so that a name
/// such as `a*.txt` is one file and not a pattern.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .env("GIT_LITERAL_PATHSPECS", "1")
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
    String::from_utf8_lossy(&output.stdout)
        .trim_end_matches('\n')
        .to_string()
}

/// `core.ignorecase` states a scenario runs with.
#[derive(Clone, Copy)]
enum IgnoreCase {
    /// Keep whatever `git init` wrote.
    Default,
    /// Remove the key.
    Unset,
    /// Set the key to `false`.
    False,
}

/// Create a repository that commits `files`, each with its own name as
/// content, and a pre-commit hook with the given line.
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
    for file in files {
        std::fs::write(path.join(file), format!("{file}\n")).unwrap();
        git(path, &["add", "--", file]);
    }
    git(path, &["commit", "-m", "initial"]);
    let hooks_dir = path.join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(hooks_dir.join("pre-commit.hooks"), hook_line).unwrap();
}

fn run_hook(path: &Path) -> assert_cmd::assert::Assert {
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(path)
        .assert()
}

/// The staged changes, with raw names, one `status<TAB>path...` line each.
///
/// Reads `-z` output, because Git quotes a name that holds a double quote
/// even with `core.quotePath=false`.
fn staged(path: &Path) -> String {
    let output = std::process::Command::new("git")
        .current_dir(path)
        .args(["diff", "--cached", "--name-status", "-z"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut fields = text.split('\0').filter(|field| !field.is_empty());
    let mut lines = Vec::new();
    while let Some(status) = fields.next() {
        let count = if status.starts_with('R') || status.starts_with('C') {
            2
        } else {
            1
        };
        let mut line = status.to_string();
        for _ in 0..count {
            line.push('\t');
            line.push_str(fields.next().unwrap());
        }
        lines.push(line);
    }
    lines.sort();
    lines.join("\n")
}

/// The unstaged changes and untracked files, with raw names.
fn unstaged(path: &Path) -> String {
    let changes = git(
        path,
        &["-c", "core.quotePath=false", "diff", "--name-status"],
    );
    let untracked = git(
        path,
        &[
            "-c",
            "core.quotePath=false",
            "ls-files",
            "--others",
            "--exclude-standard",
        ],
    );
    // The hook file is untracked in every scenario.
    let untracked: Vec<&str> = untracked
        .lines()
        .filter(|name| *name != ".githooks/pre-commit.hooks")
        .collect();
    format!("{changes}{}", untracked.join("\n"))
}

/// Names and content of every file in the work tree root, read from the
/// directory listing so that the exact on-disk name shows. The `.git` and
/// `.githooks` entries are left out.
fn work_tree(path: &Path) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = std::fs::read_dir(path)
        .unwrap()
        .map(Result::unwrap)
        .filter(|entry| entry.path().is_file())
        .map(|entry| {
            let content = std::fs::read_to_string(entry.path()).unwrap();
            (
                entry.file_name().to_string_lossy().into_owned(),
                content.replace('\n', "|"),
            )
        })
        .collect();
    files.sort();
    files
}

fn expected_files(files: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut expected: Vec<(String, String)> = files
        .iter()
        .map(|(name, content)| (name.to_string(), content.to_string()))
        .collect();
    expected.sort();
    expected
}

fn assert_clean_after_hook(path: &Path) {
    assert_eq!(unstaged(path), "", "no unstaged change, no untracked file");
    assert_eq!(git(path, &["stash", "list"]), "", "no leftover stash");
}

#[test]
fn staged_renames_with_non_ascii_names_keep_the_rename() {
    let repo = tempfile::tempdir().unwrap();
    let files = ["café.txt", "my café.txt", "quo\"te é.txt"];
    init_repo(repo.path(), IgnoreCase::Default, &files, "~ true\n");
    git(repo.path(), &["mv", "--", "café.txt", "thé.txt"]);
    git(repo.path(), &["mv", "--", "my café.txt", "my thé.txt"]);
    git(repo.path(), &["mv", "--", "quo\"te é.txt", "quo\"te è.txt"]);
    let before = staged(repo.path());

    run_hook(repo.path()).success();

    assert_eq!(staged(repo.path()), before);
    assert_eq!(
        before,
        "R100\tcafé.txt\tthé.txt\nR100\tmy café.txt\tmy thé.txt\n\
         R100\tquo\"te é.txt\tquo\"te è.txt"
    );
    assert_clean_after_hook(repo.path());
    assert_eq!(
        work_tree(repo.path()),
        expected_files(&[
            ("thé.txt", "café.txt|"),
            ("my thé.txt", "my café.txt|"),
            ("quo\"te è.txt", "quo\"te é.txt|"),
        ])
    );
}

#[test]
fn staged_deletions_with_non_ascii_names_stay_staged() {
    let repo = tempfile::tempdir().unwrap();
    let files = ["café.txt", "my café.txt", "quo\"te é.txt", "keep.txt"];
    init_repo(repo.path(), IgnoreCase::Default, &files, "~ true\n");
    git(repo.path(), &["rm", "-q", "--", "café.txt"]);
    git(repo.path(), &["rm", "-q", "--", "my café.txt"]);
    git(repo.path(), &["rm", "-q", "--", "quo\"te é.txt"]);
    let before = staged(repo.path());

    run_hook(repo.path()).success();

    assert_eq!(staged(repo.path()), before);
    assert_eq!(before, "D\tcafé.txt\nD\tmy café.txt\nD\tquo\"te é.txt");
    assert_clean_after_hook(repo.path());
    assert_eq!(
        work_tree(repo.path()),
        expected_files(&[("keep.txt", "keep.txt|")])
    );
}

#[test]
fn a_formatter_edit_of_a_non_ascii_staged_file_is_staged() {
    let repo = tempfile::tempdir().unwrap();
    let hook_line = "~ for f in \"$@\"; do echo formatted >> \"$f\"; done   *.txt\n".to_string();
    let files = ["café.txt", "my café.txt", "other.md"];
    init_repo(repo.path(), IgnoreCase::Default, &files, &hook_line);
    std::fs::write(repo.path().join("café.txt"), "edited\n").unwrap();
    std::fs::write(repo.path().join("my café.txt"), "edited\n").unwrap();
    git(repo.path(), &["add", "--", "café.txt", "my café.txt"]);

    run_hook(repo.path()).success();

    assert_eq!(staged(repo.path()), "M\tcafé.txt\nM\tmy café.txt");
    assert_eq!(
        git(repo.path(), &["show", ":café.txt"]),
        "edited\nformatted"
    );
    assert_eq!(
        git(repo.path(), &["show", ":my café.txt"]),
        "edited\nformatted"
    );
    assert_clean_after_hook(repo.path());
    assert_eq!(
        work_tree(repo.path()),
        expected_files(&[
            ("café.txt", "edited|formatted|"),
            ("my café.txt", "edited|formatted|"),
            ("other.md", "other.md|"),
        ])
    );
}

/// A staged rename that changes only the case of a non-ASCII name (#604
/// with #606).
fn case_only_rename_keeps_the_rename(ignore_case: IgnoreCase, from: &str, to: &str) {
    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path(), ignore_case, &[from], "~ true\n");
    git(repo.path(), &["mv", "--", from, "tmp.md"]);
    git(repo.path(), &["mv", "--", "tmp.md", to]);
    let before = staged(repo.path());
    assert_eq!(before, format!("R100\t{from}\t{to}"));

    run_hook(repo.path()).success();

    assert_eq!(staged(repo.path()), before);
    assert_clean_after_hook(repo.path());
    assert_eq!(
        work_tree(repo.path()),
        expected_files(&[(to, &format!("{from}|"))])
    );
}

#[test]
fn case_only_rename_of_a_non_ascii_name_with_ignorecase_unset() {
    case_only_rename_keeps_the_rename(IgnoreCase::Unset, "É.md", "é.md");
    case_only_rename_keeps_the_rename(IgnoreCase::Unset, "Café.md", "CAFÉ.md");
}

#[test]
fn case_only_rename_of_a_non_ascii_name_with_ignorecase_false() {
    case_only_rename_keeps_the_rename(IgnoreCase::False, "É.md", "é.md");
    case_only_rename_keeps_the_rename(IgnoreCase::False, "Café.md", "CAFÉ.md");
}

#[test]
fn case_only_rename_of_a_non_ascii_name_with_the_default_ignorecase() {
    case_only_rename_keeps_the_rename(IgnoreCase::Default, "É.md", "é.md");
}

/// A staged rename into a directory whose case, and only its case, differs
/// from the directory on disk (#609 with #606).
fn directory_case_mismatch(ignore_case: IgnoreCase) {
    let repo = tempfile::tempdir().unwrap();
    let path = repo.path();
    git(path, &["init"]);
    git(path, &["config", "user.name", "Test"]);
    git(path, &["config", "user.email", "test@test.com"]);
    match ignore_case {
        IgnoreCase::Default => {}
        IgnoreCase::Unset => {
            let _ = std::process::Command::new("git")
                .current_dir(path)
                .args(["config", "--unset", "core.ignorecase"])
                .status();
        }
        IgnoreCase::False => {
            git(path, &["config", "core.ignorecase", "false"]);
        }
    }
    std::fs::create_dir(path.join("café")).unwrap();
    std::fs::write(path.join("café/a.txt"), "a\n").unwrap();
    git(path, &["add", "--", "café/a.txt"]);
    git(path, &["commit", "-m", "initial"]);
    std::fs::create_dir(path.join(".githooks")).unwrap();
    std::fs::write(path.join(".githooks/pre-commit.hooks"), "~ true\n").unwrap();
    let is_case_insensitive = path.join("CAFÉ").is_dir();
    if !is_case_insensitive {
        std::fs::create_dir(path.join("CAFÉ")).unwrap();
    }
    git(path, &["mv", "--", "café/a.txt", "CAFÉ/c.txt"]);
    let before = staged(path);
    assert_eq!(before, "R100\tcafé/a.txt\tCAFÉ/c.txt");

    if is_case_insensitive {
        let assert = run_hook(path).failure();
        let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
        assert!(stderr.contains("whose case differs on disk"), "{stderr}");
        assert!(stderr.contains("CAFÉ/c.txt"), "{stderr}");
        assert_eq!(staged(path), before, "the index is unchanged");
        assert_eq!(git(path, &["stash", "list"]), "", "no stash");
    } else {
        run_hook(path).success();
        assert_eq!(staged(path), before);
        assert_clean_after_hook(path);
        assert_eq!(
            std::fs::read_to_string(path.join("CAFÉ/c.txt")).unwrap(),
            "a\n"
        );
    }
}

#[test]
fn directory_case_mismatch_with_ignorecase_unset() {
    directory_case_mismatch(IgnoreCase::Unset);
}

#[test]
fn directory_case_mismatch_with_ignorecase_false() {
    directory_case_mismatch(IgnoreCase::False);
}

/// Commit `files`, then edit `edited` in the work tree without staging it.
fn commit_with_unstaged_edit(path: &Path, files: &[&str], edited: &str, hook_line: &str) {
    init_repo(path, IgnoreCase::Default, files, hook_line);
    std::fs::write(path.join(edited), "unstaged edit\n").unwrap();
}

/// Assert that the unrelated file keeps its index entry and its unstaged
/// edit.
fn assert_unrelated_edit_untouched(path: &Path, name: &str) {
    assert_eq!(
        git(path, &["show", &format!(":{name}")]),
        name,
        "the index entry of {name} is unchanged"
    );
    assert_eq!(
        std::fs::read_to_string(path.join(name)).unwrap(),
        "unstaged edit\n"
    );
    assert_eq!(git(path, &["diff", "--name-only"]), name);
    assert_eq!(git(path, &["stash", "list"]), "", "no leftover stash");
}

/// A name with glob characters is one literal path, not a pattern (#606).
#[test]
fn a_staged_rename_of_a_name_with_glob_characters_is_literal() {
    let repo = tempfile::tempdir().unwrap();
    commit_with_unstaged_edit(repo.path(), &["a*.txt", "bz.txt"], "bz.txt", "~ true\n");
    git(repo.path(), &["mv", "--", "a*.txt", "b*.txt"]);

    run_hook(repo.path()).success();

    assert_eq!(staged(repo.path()), "R100\ta*.txt\tb*.txt");
    assert_unrelated_edit_untouched(repo.path(), "bz.txt");
}

#[test]
fn a_staged_rename_of_a_bracket_name_is_literal() {
    let repo = tempfile::tempdir().unwrap();
    commit_with_unstaged_edit(
        repo.path(),
        &["[ab].txt", "a.txt", "c.txt"],
        "a.txt",
        "~ true\n",
    );
    git(repo.path(), &["mv", "--", "[ab].txt", "[ac].txt"]);

    run_hook(repo.path()).success();

    assert_eq!(staged(repo.path()), "R100\t[ab].txt\t[ac].txt");
    assert_unrelated_edit_untouched(repo.path(), "a.txt");
}

#[test]
fn a_staged_deletion_of_a_name_with_glob_characters_is_literal() {
    let repo = tempfile::tempdir().unwrap();
    commit_with_unstaged_edit(repo.path(), &["a*.txt", "ab.txt"], "ab.txt", "~ true\n");
    git(repo.path(), &["rm", "-q", "--", "a*.txt"]);

    run_hook(repo.path()).success();

    assert_eq!(staged(repo.path()), "D\ta*.txt");
    assert_unrelated_edit_untouched(repo.path(), "ab.txt");
}

#[test]
fn restaging_a_formatted_name_with_glob_characters_is_literal() {
    let repo = tempfile::tempdir().unwrap();
    let hook_line = "~ for f in \"$@\"; do echo formatted >> \"$f\"; done   a*.txt\n";
    commit_with_unstaged_edit(repo.path(), &["a*.txt", "ab.txt"], "ab.txt", hook_line);
    std::fs::write(repo.path().join("a*.txt"), "staged edit\n").unwrap();
    git(repo.path(), &["add", "--", "a*.txt"]);

    run_hook(repo.path()).success();

    assert_eq!(staged(repo.path()), "M\ta*.txt");
    assert_eq!(
        git(repo.path(), &["show", ":a*.txt"]),
        "staged edit\nformatted"
    );
    assert_unrelated_edit_untouched(repo.path(), "ab.txt");
}

/// File names that are not valid UTF-8 are not supported by fix mode. The
/// hook stops before it changes the index, the work tree or the stash.
#[cfg(target_os = "linux")]
#[test]
fn a_staged_name_that_is_not_utf8_stops_the_hook_unchanged() {
    use std::os::unix::ffi::OsStrExt;

    let repo = tempfile::tempdir().unwrap();
    init_repo(repo.path(), IgnoreCase::Default, &["keep.txt"], "~ true\n");
    let bad = std::ffi::OsStr::from_bytes(b"bad\xff.txt");
    std::fs::write(repo.path().join(bad), "bad\n").unwrap();
    git(repo.path(), &["add", "--all"]);
    git(repo.path(), &["commit", "-q", "-m", "add bad name"]);
    std::fs::remove_file(repo.path().join(bad)).unwrap();
    git(repo.path(), &["add", "--all"]);
    let before = git(repo.path(), &["ls-files", "--stage"]);
    assert_eq!(staged(repo.path()).lines().count(), 1);

    let assert = run_hook(repo.path()).failure();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(stderr.contains("not valid UTF-8"), "{stderr}");
    assert!(stderr.contains("GIT_STD_SKIP_HOOKS=1"), "{stderr}");
    assert_eq!(git(repo.path(), &["ls-files", "--stage"]), before);
    assert_eq!(git(repo.path(), &["stash", "list"]), "");
    assert_eq!(unstaged(repo.path()), "");
}
