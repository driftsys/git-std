//! Tag-related git queries implemented via `git` CLI subprocess calls.

use std::path::Path;

use super::cmd::{GitError, git};

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI64, Ordering};

    static COMMIT_TIME: AtomicI64 = AtomicI64::new(1_800_000_000);

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
