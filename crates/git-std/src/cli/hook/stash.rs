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
/// Runs `git update-index --force-remove -- <files>` to restore the deletion
/// state in the index without touching the working tree. This undoes the
/// effect of `stash apply` which restores deleted files.
///
/// `update-index --force-remove` is used instead of `git rm --cached`
/// because it operates on exact index paths rather than resolving each path
/// against the working tree. `git rm --cached` refuses a path that used to
/// be a file but is now a directory on disk (e.g. a deleted file replaced
/// by a same-named directory in the same change) unless given `-r` — and
/// `-r` would also recursively strip any unrelated staged additions nested
/// under that directory (#533). `update-index --force-remove` has neither
/// problem: it is a no-op when the path is already gone from the index.
///
/// Returns `true` on success, `false` if the command fails. A failure means
/// the user's `git rm` intent would be silently lost — callers must treat
/// this as a fatal error.
pub(super) fn restage_deletions(files: &[String]) -> bool {
    if files.is_empty() {
        return true;
    }
    let mut cmd = Command::new("git");
    cmd.args(["update-index", "--force-remove", "--"]);
    for f in files {
        cmd.arg(f);
    }
    match cmd.status() {
        Ok(s) if s.success() => true,
        Ok(s) => {
            let code = s.code().unwrap_or(-1);
            ui::error(&format!(
                "git update-index --force-remove failed (exit {code}) — staged deletions may be lost"
            ));
            false
        }
        Err(e) => {
            ui::error(&format!(
                "git update-index --force-remove failed after fix-mode stash dance: {e}"
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

/// Read the commit SHA at the top of the shared stash stack, or `None`
/// when the stack is empty.
///
/// Git worktrees share a single `refs/stash`, so the top may be a stash
/// created by another worktree. Callers use this to identify the exact
/// stash this hook created rather than trusting the positional
/// `stash@{0}`.
fn stash_top() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", "refs/stash"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if sha.is_empty() { None } else { Some(sha) }
}

/// Run `git stash push --quiet --include-untracked` and return the commit
/// SHA of the stash it created, or `None` when nothing was stashed.
///
/// `git stash push` exits `0` even when there is nothing to stash, so its
/// exit code cannot be used to detect stash creation. Because the stash
/// stack is shared across all worktrees of a repository (#511), trusting
/// the exit code would let the hook mistake an orphan stash left by
/// another worktree for one it created — and later apply or drop it. We
/// instead compare the top of `refs/stash` before and after: a new stash
/// exists iff the top SHA changed.
///
/// `--include-untracked` ensures formatter-generated new files are captured
/// in the stash backup so they can be detected by the post-run diff check.
pub(super) fn stash_push() -> Option<String> {
    let before = stash_top();
    let pushed = Command::new("git")
        .args(["stash", "push", "--quiet", "--include-untracked"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !pushed {
        return None;
    }
    let after = stash_top();
    if after.is_some() && after != before {
        after
    } else {
        None
    }
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

/// Apply the stash identified by `stash_sha` to the working tree.
///
/// Applies the exact commit this hook created, never the positional
/// `stash@{0}`, so a stash left on the shared worktree stash stack by
/// another worktree is never applied (#511). Returns `true` on success,
/// `false` on failure (e.g. merge conflicts).
pub(super) fn stash_apply(stash_sha: &str) -> bool {
    Command::new("git")
        .args(["stash", "apply", "--quiet", stash_sha])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Drop the hook's own stash entry, identified by `stash_sha`.
///
/// Only drops when that commit is still the top of the shared stash stack,
/// so a stash created by another worktree is never dropped (#511). Warns if
/// the entry is no longer on top or the drop fails.
pub(super) fn stash_drop(stash_sha: &str) {
    if stash_top().as_deref() != Some(stash_sha) {
        ui::warning("hook stash is no longer at the top of the stash stack — leaving it in place");
        return;
    }
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
