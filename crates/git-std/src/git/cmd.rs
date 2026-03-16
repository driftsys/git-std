//! Low-level helpers for running `git` as a subprocess.

use std::path::Path;
use std::process::Command;

/// Error returned by git CLI operations.
#[derive(Debug)]
pub struct GitError {
    pub message: String,
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for GitError {}

/// Run a git command in `dir`, returning trimmed stdout on success.
pub fn git(dir: &Path, args: &[&str]) -> Result<String, GitError> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| GitError {
            message: format!("failed to run git: {e}"),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GitError {
            message: stderr.trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string())
}

/// Run a git command in `dir`, returning only success/failure.
pub fn git_ok(dir: &Path, args: &[&str]) -> Result<(), GitError> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| GitError {
            message: format!("failed to run git: {e}"),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GitError {
            message: stderr.trim().to_string(),
        });
    }
    Ok(())
}

/// Run a git command in `dir`, returning `true` if it exits successfully.
pub fn git_success(dir: &Path, args: &[&str]) -> Result<bool, GitError> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| GitError {
            message: format!("failed to run git: {e}"),
        })?;
    Ok(output.status.success())
}
