//! Read-only git queries implemented via `git` CLI subprocess calls.

use std::path::Path;

use super::cmd::{GitError, git};

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

/// Collect all tags as `(commit_sha, tag_name)` pairs sorted by creator date (newest first).
///
/// For annotated tags the dereferenced commit OID is used.
pub fn collect_tags(dir: &Path) -> Result<Vec<(String, String)>, GitError> {
    let output = git(
        dir,
        &[
            "for-each-ref",
            "--sort=-creatordate",
            "--format=%(objectname) %(*objectname) %(refname:strip=2)",
            "refs/tags/",
        ],
    )?;

    let mut tags = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.splitn(3, ' ').collect();
        if parts.len() < 3 {
            continue;
        }
        // Use dereferenced OID for annotated tags, otherwise the tag OID itself.
        let oid = if parts[1].is_empty() {
            parts[0]
        } else {
            parts[1]
        };
        let name = parts[2];
        tags.push((oid.to_string(), name.to_string()));
    }
    Ok(tags)
}

/// Find the latest semver version tag matching the given prefix.
///
/// Returns `(commit_sha, parsed_version)`.
pub fn find_latest_version_tag(
    dir: &Path,
    prefix: &str,
) -> Result<Option<(String, semver::Version)>, GitError> {
    let tags = collect_tags(dir)?;

    let mut best: Option<(String, semver::Version)> = None;
    for (oid, name) in &tags {
        let ver_str = match name.strip_prefix(prefix) {
            Some(s) => s,
            None => continue,
        };
        let ver = match semver::Version::parse(ver_str) {
            Ok(v) => v,
            Err(_) => continue,
        };
        match &best {
            Some((_, current_best)) if ver <= *current_best => {}
            _ => {
                best = Some((oid.clone(), ver));
            }
        }
    }
    Ok(best)
}

/// Find the latest calver tag matching the given prefix.
///
/// Returns `(commit_sha, version_string)`. Tags are already sorted by
/// creator date (newest first) via [`collect_tags`], so the first match wins.
pub fn find_latest_calver_tag(
    dir: &Path,
    prefix: &str,
) -> Result<Option<(String, String)>, GitError> {
    let tags = collect_tags(dir)?;
    for (oid, name) in tags {
        let ver_str = match name.strip_prefix(prefix) {
            Some(s) => s,
            None => continue,
        };
        // Calver tags start with a digit (e.g. 2024.3.0).
        if ver_str.starts_with(|c: char| c.is_ascii_digit()) {
            return Ok(Some((oid, ver_str.to_string())));
        }
    }
    Ok(None)
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

    fn tag(dir: &Path, name: &str) {
        let ts = next_timestamp();
        let output = std::process::Command::new("git")
            .current_dir(dir)
            .args(["tag", "-a", name, "-m", name])
            .env("GIT_COMMITTER_DATE", &ts)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git tag failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn find_latest_version_tag_empty_repo() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        commit(dir.path(), "chore: init");

        let result = find_latest_version_tag(dir.path(), "v").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn find_latest_version_tag_with_tags() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        commit(dir.path(), "chore: init");
        tag(dir.path(), "v1.0.0");
        commit(dir.path(), "feat: feature");
        tag(dir.path(), "v1.1.0");
        commit(dir.path(), "feat: another");
        tag(dir.path(), "v2.0.0");

        let (_, ver) = find_latest_version_tag(dir.path(), "v").unwrap().unwrap();
        assert_eq!(ver, semver::Version::new(2, 0, 0));
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
    fn collect_tags_sorted_by_date() {
        let dir = tempfile::tempdir().unwrap();
        init_repo(dir.path());
        commit(dir.path(), "chore: init");
        tag(dir.path(), "v0.1.0");
        commit(dir.path(), "feat: feature");
        tag(dir.path(), "v0.2.0");

        let tags = collect_tags(dir.path()).unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].1, "v0.2.0");
        assert_eq!(tags[1].1, "v0.1.0");
    }
}
