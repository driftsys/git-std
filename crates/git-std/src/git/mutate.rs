//! Mutable git operations implemented via `git` CLI subprocess calls.

use std::path::{Path, PathBuf};

use super::cmd::{GitError, git, git_ok, git_success};

/// Return the working directory (top-level) of the repository.
pub fn workdir(dir: &Path) -> Result<PathBuf, GitError> {
    let output = git(dir, &["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(output))
}

/// Stage the given files. Files that do not exist on disk are silently
/// skipped (matching the previous git2 behavior).
pub fn stage_files(dir: &Path, paths: &[&str]) -> Result<(), GitError> {
    let existing: Vec<&str> = paths
        .iter()
        .filter(|p| dir.join(p).exists())
        .copied()
        .collect();
    if existing.is_empty() {
        return Ok(());
    }
    let mut args = vec!["add", "--"];
    args.extend(existing);
    git_ok(dir, &args)
}

/// Stage all tracked modified files (`git add -u`).
pub fn stage_tracked_modified(dir: &Path) -> Result<(), GitError> {
    git_ok(dir, &["add", "-u"])
}

/// Create a new commit with the given message.
pub fn create_commit(dir: &Path, message: &str) -> Result<(), GitError> {
    git_ok(dir, &["commit", "-m", message])
}

/// Create a GPG-signed commit.
pub fn create_signed_commit(dir: &Path, message: &str) -> Result<(), GitError> {
    git_ok(dir, &["commit", "-S", "-m", message])
}

/// Create a GPG-signed commit, optionally amending the previous one.
pub fn create_signed_commit_amend(dir: &Path, message: &str, amend: bool) -> Result<(), GitError> {
    if amend {
        git_ok(dir, &["commit", "-S", "--amend", "-m", message])
    } else {
        git_ok(dir, &["commit", "-S", "-m", message])
    }
}

/// Amend the previous commit with a new message.
pub fn amend_commit(dir: &Path, message: &str) -> Result<(), GitError> {
    git_ok(dir, &["commit", "--amend", "-m", message])
}

/// Create an annotated tag.
pub fn create_annotated_tag(dir: &Path, name: &str, message: &str) -> Result<(), GitError> {
    git_ok(dir, &["tag", "-a", name, "-m", message])
}

/// Create a GPG-signed annotated tag.
pub fn create_signed_tag(dir: &Path, name: &str, message: &str) -> Result<(), GitError> {
    git_ok(dir, &["tag", "-s", "-a", name, "-m", message])
}

/// Return `true` if the working tree has uncommitted changes.
pub fn is_working_tree_dirty(dir: &Path) -> Result<bool, GitError> {
    let output = git(dir, &["status", "--porcelain"])?;
    Ok(!output.is_empty())
}

/// Create a new branch at HEAD.
pub fn create_branch(dir: &Path, name: &str) -> Result<(), GitError> {
    git_ok(dir, &["branch", name])
}

/// Return `true` if the given branch exists.
pub fn branch_exists(dir: &Path, name: &str) -> Result<bool, GitError> {
    let refspec = format!("refs/heads/{name}");
    git_success(dir, &["rev-parse", "--verify", &refspec])
}

/// Check out an existing branch.
pub fn checkout_branch(dir: &Path, name: &str) -> Result<(), GitError> {
    git_ok(dir, &["checkout", name])
}

/// Push the current branch and all tags to the given remote using `--follow-tags`.
pub fn push_follow_tags(dir: &Path, remote: &str) -> Result<(), GitError> {
    git_ok(dir, &["push", "--follow-tags", remote, "HEAD"])
}
