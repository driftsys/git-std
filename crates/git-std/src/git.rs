//! Shared git2 helpers used by multiple CLI subcommands.

use std::collections::HashMap;

use standard_changelog::RepoHost;

/// Walk commits from `from_oid` (inclusive) back to `until_oid` (exclusive), or
/// to the root when `until_oid` is `None`.
pub fn walk_commits(
    repo: &git2::Repository,
    from_oid: git2::Oid,
    until_oid: Option<git2::Oid>,
) -> Result<Vec<(git2::Oid, String)>, git2::Error> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push(from_oid)?;
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL)?;

    if let Some(until) = until_oid {
        revwalk.hide(until)?;
    }

    let mut commits = Vec::new();
    for oid in revwalk {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let message = commit.message().unwrap_or("").to_string();
        commits.push((oid, message));
    }

    Ok(commits)
}

/// Detect the repo host from the `origin` remote URL.
pub fn detect_host_from_repo(repo: &git2::Repository) -> RepoHost {
    repo.find_remote("origin")
        .ok()
        .and_then(|remote| remote.url().map(standard_changelog::detect_host))
        .unwrap_or(RepoHost::Unknown)
}

/// Collect all tags pointing at commits, sorted by commit time (newest first).
pub fn collect_tags(repo: &git2::Repository) -> Result<Vec<(git2::Oid, String)>, git2::Error> {
    let mut tag_map: HashMap<git2::Oid, String> = HashMap::new();

    repo.tag_foreach(|oid, name_bytes| {
        let name = String::from_utf8_lossy(name_bytes).to_string();
        let name = name.strip_prefix("refs/tags/").unwrap_or(&name).to_string();

        // Peel annotated tags to their target commit.
        let target_oid = repo.find_tag(oid).map(|tag| tag.target_id()).unwrap_or(oid);

        tag_map.insert(target_oid, name);
        true
    })?;

    // Sort by commit time (newest first).
    let mut tags: Vec<(git2::Oid, String)> = tag_map.into_iter().collect();
    tags.sort_by(|a, b| {
        let time_a = repo
            .find_commit(a.0)
            .map(|c| c.time().seconds())
            .unwrap_or(0);
        let time_b = repo
            .find_commit(b.0)
            .map(|c| c.time().seconds())
            .unwrap_or(0);
        time_b.cmp(&time_a)
    });

    Ok(tags)
}

/// Find the latest version tag matching `<prefix><semver>`.
pub fn find_latest_version_tag(
    repo: &git2::Repository,
    prefix: &str,
) -> Result<Option<(git2::Oid, semver::Version)>, Box<dyn std::error::Error>> {
    let mut tags: Vec<(git2::Oid, semver::Version, i64)> = Vec::new();

    repo.tag_foreach(|oid, name_bytes| {
        let name = String::from_utf8_lossy(name_bytes).to_string();
        let name = name.strip_prefix("refs/tags/").unwrap_or(&name);

        if let Some(ver_str) = name.strip_prefix(prefix)
            && let Ok(ver) = semver::Version::parse(ver_str)
        {
            // Peel annotated tags to their target commit.
            let target_oid = repo.find_tag(oid).map(|t| t.target_id()).unwrap_or(oid);
            let time = repo
                .find_commit(target_oid)
                .map(|c| c.time().seconds())
                .unwrap_or(0);
            tags.push((target_oid, ver, time));
        }
        true
    })?;

    // Sort by semver (highest first).
    tags.sort_by(|a, b| b.1.cmp(&a.1));

    Ok(tags.into_iter().next().map(|(oid, ver, _)| (oid, ver)))
}

/// Find the latest calver-style tag matching `<prefix><version>`.
///
/// Unlike semver tags, calver tags are sorted by commit time (newest first)
/// since the version string itself may not sort correctly as a semver value.
pub fn find_latest_calver_tag(
    repo: &git2::Repository,
    prefix: &str,
) -> Result<Option<(git2::Oid, String)>, Box<dyn std::error::Error>> {
    let mut tags: Vec<(git2::Oid, String, i64)> = Vec::new();

    repo.tag_foreach(|oid, name_bytes| {
        let name = String::from_utf8_lossy(name_bytes).to_string();
        let name = name.strip_prefix("refs/tags/").unwrap_or(&name).to_string();

        if let Some(ver_str) = name.strip_prefix(prefix) {
            // Accept any tag that starts with the prefix and has a digit after it.
            if ver_str.starts_with(|c: char| c.is_ascii_digit()) {
                let target_oid = repo.find_tag(oid).map(|t| t.target_id()).unwrap_or(oid);
                let time = repo
                    .find_commit(target_oid)
                    .map(|c| c.time().seconds())
                    .unwrap_or(0);
                tags.push((target_oid, ver_str.to_string(), time));
            }
        }
        true
    })?;

    // Sort by commit time (newest first).
    tags.sort_by(|a, b| b.2.cmp(&a.2));

    Ok(tags.into_iter().next().map(|(oid, ver, _)| (oid, ver)))
}

/// Format a commit's time as `YYYY-MM-DD`.
pub fn format_commit_date(commit: &git2::Commit) -> String {
    let time = commit.time();
    let secs = time.seconds() + (time.offset_minutes() as i64) * 60;
    standard_changelog::format_date(secs)
}

/// Stage the given relative paths in the repository index.
pub fn stage_files(
    repo: &git2::Repository,
    paths: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let workdir = repo.workdir().ok_or("bare repository not supported")?;
    let mut index = repo.index()?;

    for rel_path in paths {
        let full = workdir.join(rel_path);
        if full.exists() {
            index.add_path(std::path::Path::new(rel_path))?;
        }
    }

    index.write()?;
    Ok(())
}

/// Create a commit using git2.
pub fn create_commit(
    repo: &git2::Repository,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let sig = repo.signature()?;
    let mut index = repo.index()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let parent = repo.head()?.peel_to_commit()?;
    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])?;

    Ok(())
}

/// Create a signed commit by shelling out to `git`.
pub fn create_signed_commit(message: &str) -> Result<(), Box<dyn std::error::Error>> {
    let status = std::process::Command::new("git")
        .args(["commit", "-S", "-m", message])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("git commit exited with status {status}").into())
    }
}

/// Create an annotated tag using git2.
pub fn create_annotated_tag(
    repo: &git2::Repository,
    name: &str,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let sig = repo.signature()?;
    let head = repo.head()?.peel_to_commit()?;
    let obj = head.as_object();
    repo.tag(name, obj, &sig, message, false)?;
    Ok(())
}

/// Create a signed tag by shelling out to `git`.
pub fn create_signed_tag(name: &str, message: &str) -> Result<(), Box<dyn std::error::Error>> {
    let status = std::process::Command::new("git")
        .args(["tag", "-s", "-a", name, "-m", message])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("git tag exited with status {status}").into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_latest_version_tag_empty_repo() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();

        // Create an initial commit.
        let file_path = dir.path().join("hello.txt");
        std::fs::write(&file_path, "hello").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("hello.txt")).unwrap();
        index.write().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();

        let result = find_latest_version_tag(&repo, "v").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn find_latest_version_tag_with_tags() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();

        // Create commits and tags.
        let file_path = dir.path().join("hello.txt");
        std::fs::write(&file_path, "v1").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("hello.txt")).unwrap();
        index.write().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        let c1 = repo
            .commit(Some("HEAD"), &sig, &sig, "feat: first", &tree, &[])
            .unwrap();

        let obj1 = repo.find_object(c1, None).unwrap();
        repo.tag("v1.0.0", &obj1, &sig, "v1.0.0", false).unwrap();

        std::fs::write(&file_path, "v2").unwrap();
        index.add_path(std::path::Path::new("hello.txt")).unwrap();
        index.write().unwrap();
        let tree_oid2 = index.write_tree().unwrap();
        let tree2 = repo.find_tree(tree_oid2).unwrap();
        let parent = repo.find_commit(c1).unwrap();
        let c2 = repo
            .commit(Some("HEAD"), &sig, &sig, "feat: second", &tree2, &[&parent])
            .unwrap();

        let obj2 = repo.find_object(c2, None).unwrap();
        repo.tag("v2.0.0", &obj2, &sig, "v2.0.0", false).unwrap();

        let result = find_latest_version_tag(&repo, "v").unwrap();
        assert!(result.is_some());
        let (_, ver) = result.unwrap();
        assert_eq!(ver, semver::Version::new(2, 0, 0));
    }
}
