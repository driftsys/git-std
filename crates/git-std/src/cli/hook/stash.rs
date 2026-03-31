use std::process::Command;

use crate::ui;

/// Fetch all tracked files for glob filtering in non-pre-commit hooks.
///
/// Returns file paths from `git ls-files`. Only called when at least one
/// command has a glob pattern and the hook is not `pre-commit` (pre-commit
/// reuses the already-fetched staged files instead).
pub(super) fn fetch_tracked_files() -> Option<Vec<String>> {
    match Command::new("git").args(["ls-files"]).output() {
        Ok(o) => Some(
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(String::from)
                .collect(),
        ),
        Err(_) => None,
    }
}

/// Fetch staged file paths matching the given `--diff-filter`.
///
/// Returns file paths from `git diff --cached --name-only --diff-filter=<filter>`
/// relative to the working tree root. Returns an empty vec on failure.
pub(super) fn fetch_staged(filter: &str) -> Vec<String> {
    match Command::new("git")
        .args([
            "diff",
            "--cached",
            "--name-only",
            &format!("--diff-filter={filter}"),
        ])
        .output()
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(String::from)
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Re-apply staged deletions after the stash dance.
///
/// Runs `git rm --cached --quiet -- <files>` to restore the deletion state
/// in the index without touching the working tree. This undoes the effect
/// of `stash apply` which restores deleted files.
///
/// Returns `true` on success, `false` if the command fails. A failure means
/// the user's `git rm` intent would be silently lost — callers must treat
/// this as a fatal error.
pub(super) fn restage_deletions(files: &[String]) -> bool {
    if files.is_empty() {
        return true;
    }
    let mut cmd = Command::new("git");
    cmd.args(["rm", "--cached", "--quiet", "--force", "--"]);
    for f in files {
        cmd.arg(f);
    }
    match cmd.status() {
        Ok(s) if s.success() => true,
        Ok(s) => {
            let code = s.code().unwrap_or(-1);
            ui::error(&format!(
                "git rm --cached failed (exit {code}) — staged deletions may be lost"
            ));
            false
        }
        Err(e) => {
            ui::error(&format!(
                "git rm --cached failed after fix-mode stash dance: {e}"
            ));
            false
        }
    }
}

/// Fetch the list of unstaged (working-tree-modified) file paths.
///
/// Returns file paths that differ between index and working tree.
pub(super) fn fetch_unstaged_files() -> Vec<String> {
    match Command::new("git").args(["diff", "--name-only"]).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(String::from)
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Check whether any staged entries are submodules (mode `160000`).
///
/// Parses `git diff --cached --diff-filter=ACMR --raw` and looks for the
/// submodule file mode. Returns `true` if at least one submodule entry is
/// staged.
pub(super) fn has_staged_submodules() -> bool {
    let output = Command::new("git")
        .args(["diff", "--cached", "--diff-filter=ACMR", "--raw"])
        .output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).contains(" 160000 "),
        Err(_) => false,
    }
}

/// Run `git stash push --quiet --include-untracked`. Returns `true` if the
/// stash was created successfully (something was stashed), `false` otherwise
/// (nothing to stash or git error).
///
/// `--include-untracked` ensures formatter-generated new files are captured
/// in the stash backup so they can be detected by the post-run diff check.
pub(super) fn stash_push() -> bool {
    Command::new("git")
        .args(["stash", "push", "--quiet", "--include-untracked"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Get the new names of any files staged as renames.
///
/// Returns the new file paths from renames. Used to temporarily unstage
/// renames before the stash dance to prevent corruption (#387).
pub(super) fn fetch_staged_rename_targets() -> Vec<String> {
    match Command::new("git")
        .args(["diff", "--cached", "--diff-filter=R", "--name-only"])
        .output()
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(String::from)
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Temporarily unstage renamed files by restoring only the new names from
/// the HEAD. This prevents git stash from corrupting the rename.
///
/// Returns `true` on success, `false` on failure.
pub(super) fn unstage_renames(rename_targets: &[String]) -> bool {
    if rename_targets.is_empty() {
        return true;
    }
    let mut cmd = Command::new("git");
    cmd.args(["restore", "--staged", "--"]);
    for f in rename_targets {
        cmd.arg(f);
    }
    matches!(cmd.status(), Ok(s) if s.success())
}

/// Run `git stash apply --quiet`. Returns `true` on success, `false` on
/// failure (e.g. merge conflicts).
pub(super) fn stash_apply() -> bool {
    Command::new("git")
        .args(["stash", "apply", "--quiet"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run `git stash drop --quiet`. Warns on failure.
pub(super) fn stash_drop() {
    let ok = Command::new("git")
        .args(["stash", "drop", "--quiet"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        ui::warning("git stash drop failed — stash entry may remain");
    }
}

/// Re-stage the given files after a formatter has run.
///
/// Runs `git add -- <files>` to pick up any formatting changes.
/// Files that no longer exist on disk are skipped with a warning to
/// prevent a formatter-caused deletion from being silently staged (#279).
///
/// Returns `true` on success, `false` if the command fails. A failure means
/// formatted changes would be silently lost — callers must treat this as a
/// fatal error.
pub(super) fn restage_files(files: &[String]) -> bool {
    if files.is_empty() {
        return true;
    }
    let mut existing: Vec<&String> = Vec::new();
    for f in files {
        if std::path::Path::new(f).exists() {
            existing.push(f);
        } else {
            ui::warning(&format!("{f} was deleted by formatter — skipping restage"));
        }
    }
    if existing.is_empty() {
        return true;
    }
    let mut cmd = Command::new("git");
    cmd.arg("add").arg("--");
    for f in &existing {
        cmd.arg(f);
    }
    match cmd.status() {
        Ok(s) if s.success() => true,
        Ok(s) => {
            let code = s.code().unwrap_or(-1);
            ui::error(&format!(
                "git add failed (exit {code}) — formatted changes may be lost"
            ));
            false
        }
        Err(e) => {
            ui::error(&format!("git add failed after fix-mode formatting: {e}"));
            false
        }
    }
}
