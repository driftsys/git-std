//! Read-only git queries implemented via `git` CLI subprocess calls.

use std::path::Path;

use super::cmd::{GitError, git};

/// Read a git config value.
pub fn config_value(dir: &Path, key: &str) -> Result<String, GitError> {
    git(dir, &["config", key])
}

/// Return the full SHA of HEAD.
pub fn head_oid(dir: &Path) -> Result<String, GitError> {
    git(dir, &["rev-parse", "HEAD"])
}

/// Return the current branch name (e.g. `main`).
pub fn current_branch(dir: &Path) -> Result<String, GitError> {
    git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
}

/// Resolve an arbitrary revision spec to a full SHA.
pub fn resolve_rev(dir: &Path, rev: &str) -> Result<String, GitError> {
    git(dir, &["rev-parse", "--verify", rev])
}

/// Detect the repository host (GitHub, GitLab, etc.) from the `origin` remote URL.
pub fn detect_host(dir: &Path) -> standard_changelog::RepoHost {
    match git(dir, &["remote", "get-url", "origin"]) {
        Ok(url) => standard_changelog::detect_host(&url),
        Err(_) => standard_changelog::RepoHost::Unknown,
    }
}

/// Return the staged diff (`git diff --staged`).
///
/// Returns an empty string when there is nothing staged.
pub fn staged_diff(dir: &Path) -> Result<String, GitError> {
    git(dir, &["diff", "--staged"])
}

/// Return `git status --short` output.
///
/// Returns an empty string when the working tree is clean.
pub fn short_status(dir: &Path) -> Result<String, GitError> {
    git(dir, &["status", "--short"])
}

/// Walk commits from `from` (inclusive) back to `until` (exclusive).
///
/// Returns `(full_sha, commit_message)` pairs in topological order.
pub fn walk_commits(
    dir: &Path,
    from: &str,
    until: Option<&str>,
) -> Result<Vec<(String, String)>, GitError> {
    let range = match until {
        Some(u) => format!("{u}..{from}"),
        None => from.to_string(),
    };

    let output = git(
        dir,
        &["log", "--format=%H%x00%B%x00", "--topo-order", &range, "--"],
    )?;
    Ok(parse_nul_delimited_log(&output))
}

/// Walk commits in a revision range string (e.g. `v1.0.0..v2.0.0`).
pub fn walk_range(dir: &Path, range: &str) -> Result<Vec<(String, String)>, GitError> {
    let output = git(
        dir,
        &["log", "--format=%H%x00%B%x00", "--topo-order", range, "--"],
    )?;
    Ok(parse_nul_delimited_log(&output))
}

/// Walk commits from `from` (inclusive) back to `until` (exclusive),
/// filtered to only those touching the given paths.
///
/// Returns `(full_sha, commit_message)` pairs in topological order.
/// Branch commits merged via pull requests are included — the path filter
/// already limits results to commits that actually touched the given paths.
pub fn walk_commits_for_path(
    dir: &Path,
    from: &str,
    until: Option<&str>,
    paths: &[&str],
) -> Result<Vec<(String, String)>, GitError> {
    let range = match until {
        Some(u) => format!("{u}..{from}"),
        None => from.to_string(),
    };

    let mut args = vec!["log", "--format=%H%x00%B%x00", "--topo-order", &range, "--"];
    args.extend(paths);

    let output = git(dir, &args)?;
    Ok(parse_nul_delimited_log(&output))
}

/// Parse NUL-delimited `git log` output into `(sha, message)` pairs.
fn parse_nul_delimited_log(output: &str) -> Vec<(String, String)> {
    let mut commits = Vec::new();
    let parts: Vec<&str> = output.split('\0').collect();
    // The format produces: SHA\0BODY\0 SHA\0BODY\0 ...
    // After split we get pairs with possible leading newlines.
    let mut i = 0;
    while i + 1 < parts.len() {
        let sha_part = parts[i].trim();
        let msg_part = parts[i + 1].trim();
        if !sha_part.is_empty() && sha_part.len() >= 40 {
            // The SHA is the last 40+ hex chars of sha_part (could have leading newline from previous record).
            let sha = if let Some(pos) = sha_part.rfind('\n') {
                &sha_part[pos + 1..]
            } else {
                sha_part
            };
            if sha.len() >= 40 {
                commits.push((sha.to_string(), msg_part.to_string()));
            }
        }
        i += 2;
    }
    commits
}

/// Return the commit date of a revision as `YYYY-MM-DD`.
pub fn commit_date(dir: &Path, rev: &str) -> Result<String, GitError> {
    let output = git(dir, &["log", "-1", "--format=%ai", rev, "--"])?;
    // %ai produces "2024-03-16 12:34:56 +0000", take first 10 chars.
    if output.len() < 10 {
        return Err(GitError {
            message: format!("unexpected date format: '{output}'"),
        });
    }
    Ok(output[..10].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI64, Ordering};

    static COMMIT_TIME: AtomicI64 = AtomicI64::new(1_700_000_000);

    fn next_timestamp() -> String {
        let ts = COMMIT_TIME.fetch_add(1, Ordering::SeqCst);
        format!("{ts} +0000")
    }

    fn init_repo(dir: &Path) {
        std::process::Command::new("git")
            .current_dir(dir)
            .args(["init"])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .current_dir(dir)
            .args(["config", "user.name", "Test"])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .current_dir(dir)
            .args(["config", "user.email", "test@test.com"])
            .output()
            .unwrap();
    }

    fn commit(dir: &Path, message: &str) -> String {
        let ts = next_timestamp();
        let filename = format!("file-{}.txt", &ts[..10]);
        std::fs::write(dir.join(&filename), message).unwrap();
        std::process::Command::new("git")
            .current_dir(dir)
            .args(["add", &filename])
            .output()
            .unwrap();
        let output = std::process::Command::new("git")
            .current_dir(dir)
            .args(["commit", "-m", message])
            .env("GIT_COMMITTER_DATE", &ts)
            .env("GIT_AUTHOR_DATE", &ts)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git commit failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        git(dir, &["rev-parse", "HEAD"]).unwrap()
    }

    /// Create a file in a specific subdirectory and commit it.
    fn commit_in_path(dir: &Path, subdir: &str, message: &str) -> String {
        let ts = next_timestamp();
        let full_dir = dir.join(subdir);
        std::fs::create_dir_all(&full_dir).unwrap();
        let filename = format!("{subdir}/file-{}.txt", &ts[..10]);
        std::fs::write(dir.join(&filename), message).unwrap();
        std::process::Command::new("git")
            .current_dir(dir)
            .args(["add", &filename])
            .output()
            .unwrap();
        let output = std::process::Command::new("git")
            .current_dir(dir)
            .args(["commit", "-m", message])
            .env("GIT_COMMITTER_DATE", &ts)
            .env("GIT_AUTHOR_DATE", &ts)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git commit failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        git(dir, &["rev-parse", "HEAD"]).unwrap()
    }

    #[test]
    fn walk_commits_returns_topological_order() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        commit(dir.path(), "chore: init");
        let start = git(dir.path(), &["rev-parse", "HEAD"]).unwrap();
        commit(dir.path(), "feat: A");
        commit(dir.path(), "feat: B");
        let head = git(dir.path(), &["rev-parse", "HEAD"]).unwrap();

        let commits = walk_commits(dir.path(), &head, Some(&start)).unwrap();
        assert_eq!(commits.len(), 2);
        // Topological order: newest first.
        assert_eq!(commits[0].1, "feat: B");
        assert_eq!(commits[1].1, "feat: A");
    }

    #[test]
    fn walk_commits_for_path_filters_by_directory() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        let base = commit(dir.path(), "chore: init");
        commit_in_path(dir.path(), "crates/core", "feat: core feature");
        commit_in_path(dir.path(), "crates/cli", "feat: cli feature");
        commit_in_path(dir.path(), "crates/core", "fix: core fix");
        let head = head_oid(dir.path()).unwrap();

        // Only core commits
        let core_commits =
            walk_commits_for_path(dir.path(), &head, Some(&base), &["crates/core"]).unwrap();
        assert_eq!(core_commits.len(), 2);
        assert_eq!(core_commits[0].1, "fix: core fix");
        assert_eq!(core_commits[1].1, "feat: core feature");

        // Only cli commits
        let cli_commits =
            walk_commits_for_path(dir.path(), &head, Some(&base), &["crates/cli"]).unwrap();
        assert_eq!(cli_commits.len(), 1);
        assert_eq!(cli_commits[0].1, "feat: cli feature");
    }

    #[test]
    fn walk_commits_for_path_multi_package_commit_appears_in_both() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        let base = commit(dir.path(), "chore: init");

        // Create a commit that touches both core and cli
        let ts = next_timestamp();
        std::fs::create_dir_all(dir.path().join("crates/core")).unwrap();
        std::fs::create_dir_all(dir.path().join("crates/cli")).unwrap();
        std::fs::write(dir.path().join("crates/core/shared.txt"), "shared").unwrap();
        std::fs::write(dir.path().join("crates/cli/shared.txt"), "shared").unwrap();
        std::process::Command::new("git")
            .current_dir(dir.path())
            .args(["add", "."])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .current_dir(dir.path())
            .args(["commit", "-m", "feat: shared change"])
            .env("GIT_COMMITTER_DATE", &ts)
            .env("GIT_AUTHOR_DATE", &ts)
            .output()
            .unwrap();
        let head = head_oid(dir.path()).unwrap();

        let core_commits =
            walk_commits_for_path(dir.path(), &head, Some(&base), &["crates/core"]).unwrap();
        let cli_commits =
            walk_commits_for_path(dir.path(), &head, Some(&base), &["crates/cli"]).unwrap();

        assert_eq!(core_commits.len(), 1);
        assert_eq!(cli_commits.len(), 1);
        assert_eq!(core_commits[0].1, "feat: shared change");
        assert_eq!(cli_commits[0].1, "feat: shared change");
    }

    #[test]
    fn walk_commits_for_path_empty_when_no_matching_commits() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        let base = commit(dir.path(), "chore: init");
        commit_in_path(dir.path(), "crates/core", "feat: core only");
        let head = head_oid(dir.path()).unwrap();

        let commits =
            walk_commits_for_path(dir.path(), &head, Some(&base), &["crates/cli"]).unwrap();
        assert!(commits.is_empty());
    }

    #[test]
    fn walk_commits_for_path_without_until() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        commit_in_path(dir.path(), "crates/core", "feat: core feature");
        commit_in_path(dir.path(), "crates/cli", "feat: cli feature");
        let head = head_oid(dir.path()).unwrap();

        // Without until, returns all matching commits from HEAD back
        let core_commits =
            walk_commits_for_path(dir.path(), &head, None, &["crates/core"]).unwrap();
        assert_eq!(core_commits.len(), 1);
        assert_eq!(core_commits[0].1, "feat: core feature");
    }

    /// Regression: branch commits merged via a PR must be visible to
    /// `walk_commits_for_path`. Previously `--first-parent` caused git to skip
    /// the feature-branch commits entirely, so the path-filtered query returned
    /// an empty result even though conventional commits existed for the path.
    #[test]
    fn walk_commits_for_path_includes_merged_branch_commits() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        let base = commit(dir.path(), "chore: init");

        // Simulate a feature branch: create a detached branch from base, add a
        // commit touching crates/core, then merge it into main.
        std::process::Command::new("git")
            .current_dir(dir.path())
            .args(["checkout", "-b", "feature"])
            .output()
            .unwrap();
        let ts = next_timestamp();
        std::fs::create_dir_all(dir.path().join("crates/core")).unwrap();
        std::fs::write(dir.path().join("crates/core/f.txt"), "x").unwrap();
        std::process::Command::new("git")
            .current_dir(dir.path())
            .args(["add", "crates/core/f.txt"])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .current_dir(dir.path())
            .args(["commit", "-m", "feat: core feature on branch"])
            .env("GIT_COMMITTER_DATE", &ts)
            .env("GIT_AUTHOR_DATE", &ts)
            .output()
            .unwrap();

        // Switch back to main and merge (creates a merge commit).
        std::process::Command::new("git")
            .current_dir(dir.path())
            .args(["checkout", "-"])
            .output()
            .unwrap();
        let ts2 = next_timestamp();
        std::process::Command::new("git")
            .current_dir(dir.path())
            .args([
                "merge",
                "--no-ff",
                "feature",
                "-m",
                "Merge branch 'feature'",
            ])
            .env("GIT_COMMITTER_DATE", &ts2)
            .env("GIT_AUTHOR_DATE", &ts2)
            .output()
            .unwrap();

        let head = head_oid(dir.path()).unwrap();
        let commits =
            walk_commits_for_path(dir.path(), &head, Some(&base), &["crates/core"]).unwrap();

        assert_eq!(
            commits.len(),
            1,
            "branch commit must be visible after merge"
        );
        assert_eq!(commits[0].1, "feat: core feature on branch");
    }
}
