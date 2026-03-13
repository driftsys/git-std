use std::collections::HashMap;

use standard_changelog::{RepoHost, VersionRelease};
use yansi::Paint;

use crate::config::ProjectConfig;

/// Options for the bump subcommand.
pub struct BumpOptions {
    /// Print the plan without writing anything.
    pub dry_run: bool,
    /// Bump as pre-release (e.g. `2.0.0-rc.1`).
    pub prerelease: Option<String>,
    /// Force a specific version, skip calculation.
    pub release_as: Option<String>,
    /// Use current version for initial changelog (no bump).
    pub first_release: bool,
    /// Skip tag creation.
    pub no_tag: bool,
    /// Skip commit and tag (update files only).
    pub no_commit: bool,
    /// Skip changelog generation.
    pub skip_changelog: bool,
    /// GPG-sign the commit and tag.
    pub sign: bool,
}

/// Run the bump subcommand. Returns the exit code.
pub fn run(config: &ProjectConfig, opts: &BumpOptions) -> i32 {
    let repo = match git2::Repository::discover(".") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: cannot open repository: {e}");
            return 1;
        }
    };

    let tag_prefix = &config.versioning.tag_prefix;

    // Step 1: Find latest version tag.
    let current_version = match find_latest_version_tag(&repo, tag_prefix) {
        Ok(Some((oid, ver))) => Some((oid, ver)),
        Ok(None) => None,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    // Step 2: Collect commits since that tag.
    let head_oid = match repo.head().and_then(|h| h.peel_to_commit().map(|c| c.id())) {
        Ok(oid) => oid,
        Err(e) => {
            eprintln!("error: cannot resolve HEAD: {e}");
            return 1;
        }
    };

    let tag_oid = current_version.as_ref().map(|(oid, _)| *oid);
    let raw_commits = match walk_commits(&repo, head_oid, tag_oid) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    // Step 3: Parse as conventional commits.
    let parsed: Vec<standard_commit::ConventionalCommit> = raw_commits
        .iter()
        .filter_map(|(_, msg)| standard_commit::parse(msg).ok())
        .collect();

    let cur_ver = current_version
        .as_ref()
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| semver::Version::new(0, 0, 0));

    let cur_ver_str = current_version
        .as_ref()
        .map(|(_, v)| format!("{tag_prefix}{v}"))
        .unwrap_or_else(|| "none".to_string());

    // Step 4: Determine new version.
    let new_version = if let Some(ref forced) = opts.release_as {
        match semver::Version::parse(forced) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("error: invalid --release-as version '{forced}': {e}");
                return 1;
            }
        }
    } else if opts.first_release {
        cur_ver.clone()
    } else {
        let summary = standard_version::summarise(&parsed);
        let bump_level = match standard_version::determine_bump(&parsed) {
            Some(level) => level,
            None => {
                eprintln!();
                eprintln!("  Analysing commits since {}...", cur_ver_str.bold());
                eprintln!("    no bump-worthy commits found");
                eprintln!();
                return 0;
            }
        };

        eprintln!();
        eprintln!("  Analysing commits since {}...", cur_ver_str.bold());
        print_summary(&summary);

        if let Some(ref pre_tag) = opts.prerelease {
            let tag = if pre_tag.is_empty() {
                &config.versioning.prerelease_tag
            } else {
                pre_tag
            };
            standard_version::apply_prerelease(&cur_ver, bump_level, tag)
        } else {
            standard_version::apply_bump(&cur_ver, bump_level)
        }
    };

    let bump_reason = if opts.first_release {
        "first release".to_string()
    } else if let Some(ref forced) = opts.release_as {
        format!("forced as {forced}")
    } else {
        let level = standard_version::determine_bump(&parsed).unwrap();
        let reason = match level {
            standard_version::BumpLevel::Major => "major — breaking change detected",
            standard_version::BumpLevel::Minor => "minor — new feature",
            standard_version::BumpLevel::Patch => "patch — bug fix",
        };
        reason.to_string()
    };

    eprintln!();
    eprintln!(
        "  {} ({bump_reason})",
        format!("{cur_ver} → {new_version}").bold()
    );

    // --- Dry run: print plan and exit ---
    if opts.dry_run {
        eprintln!();

        // Show which files would be updated.
        let cargo_toml = find_cargo_toml(&repo);
        if let Some(ref path) = cargo_toml {
            eprintln!("  Would update: {:<20} {} → {}", path, cur_ver, new_version);
        }

        if !opts.skip_changelog {
            eprintln!(
                "  Would update: CHANGELOG.md         prepend {tag_prefix}{new_version} section"
            );
        }

        if !opts.no_commit {
            eprintln!("  Would commit: chore(release): {new_version}");
        }

        if !opts.no_commit && !opts.no_tag {
            eprintln!("  Would tag:    {tag_prefix}{new_version}");
        }

        eprintln!();
        return 0;
    }

    // --- Actual execution ---
    let mut updated_files: Vec<(String, String, String)> = Vec::new();

    // Step 7: Update Cargo.toml version.
    if let Some(ref path) = find_cargo_toml(&repo) {
        let workdir = match repo.workdir() {
            Some(w) => w,
            None => {
                eprintln!("error: bare repository not supported");
                return 1;
            }
        };
        let full_path = workdir.join(path);
        if let Err(e) = update_cargo_toml_version(&full_path, &new_version.to_string()) {
            eprintln!("error: cannot update {path}: {e}");
            return 1;
        }
        updated_files.push((path.clone(), cur_ver.to_string(), new_version.to_string()));
    }

    // Step 8: Generate/update changelog.
    if !opts.skip_changelog {
        let changelog_config = config.to_changelog_config();
        let host = detect_host_from_repo(&repo);
        let workdir = match repo.workdir() {
            Some(w) => w,
            None => {
                eprintln!("error: bare repository not supported");
                return 1;
            }
        };
        let changelog_path = workdir.join("CHANGELOG.md");

        // Build a release for the new version.
        let release = build_version_release(
            &raw_commits,
            &new_version.to_string(),
            current_version
                .as_ref()
                .map(|(_, v)| v.to_string())
                .as_deref(),
            &changelog_config,
        );

        if let Some(release) = release {
            let existing = std::fs::read_to_string(&changelog_path).unwrap_or_default();
            let output =
                standard_changelog::prepend_release(&existing, &release, &changelog_config, &host);
            if let Err(e) = std::fs::write(&changelog_path, &output) {
                eprintln!("error: cannot write CHANGELOG.md: {e}");
                return 1;
            }
        }
    }

    // Print updated files.
    if !updated_files.is_empty() {
        eprintln!();
        eprintln!("  Updated:");
        for (file, old, new) in &updated_files {
            eprintln!("    {:<20} {} → {}", file, old, new);
        }
    }

    if !opts.skip_changelog {
        eprintln!();
        eprintln!("  Changelog:");
        eprintln!(
            "    {:<20} prepended {tag_prefix}{new_version} section",
            "CHANGELOG.md"
        );
    }

    // Step 9: Create commit.
    if !opts.no_commit {
        let workdir = match repo.workdir() {
            Some(w) => w,
            None => {
                eprintln!("error: bare repository not supported");
                return 1;
            }
        };

        // Stage all modified files.
        if let Err(e) = stage_files(&repo, workdir) {
            eprintln!("error: cannot stage files: {e}");
            return 1;
        }

        let commit_msg = format!("chore(release): {new_version}");

        if opts.sign {
            if let Err(e) = create_signed_commit(&commit_msg) {
                eprintln!("error: {e}");
                return 1;
            }
        } else if let Err(e) = create_commit(&repo, &commit_msg) {
            eprintln!("error: cannot create commit: {e}");
            return 1;
        }

        eprintln!();
        eprintln!("  Committed: {}", commit_msg.green());
    }

    // Step 10: Create annotated tag.
    if !opts.no_commit && !opts.no_tag {
        let tag_name = format!("{tag_prefix}{new_version}");
        let tag_msg = format!("{new_version}");

        if opts.sign {
            if let Err(e) = create_signed_tag(&tag_name, &tag_msg) {
                eprintln!("error: {e}");
                return 1;
            }
        } else if let Err(e) = create_annotated_tag(&repo, &tag_name, &tag_msg) {
            eprintln!("error: cannot create tag: {e}");
            return 1;
        }

        eprintln!("  Tagged:    {}", tag_name.green());
    }

    eprintln!();
    eprintln!("  Push with: git push --follow-tags");
    eprintln!();

    0
}

/// Print a summary of analysed commits.
fn print_summary(summary: &standard_version::BumpSummary) {
    let mut parts = Vec::new();
    if summary.feat_count > 0 {
        parts.push(format!("{} feat", summary.feat_count));
    }
    if summary.fix_count > 0 {
        parts.push(format!("{} fix", summary.fix_count));
    }
    if summary.breaking_count > 0 {
        parts.push(format!("{} BREAKING CHANGE", summary.breaking_count));
    }
    if summary.other_count > 0 {
        parts.push(format!("{} other", summary.other_count));
    }
    if !parts.is_empty() {
        eprintln!("    {}", parts.join(", "));
    }
}

/// Find the latest version tag matching `<prefix><semver>`.
fn find_latest_version_tag(
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

/// Walk commits from `from_oid` (inclusive) back to `until_oid` (exclusive).
fn walk_commits(
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
fn detect_host_from_repo(repo: &git2::Repository) -> RepoHost {
    repo.find_remote("origin")
        .ok()
        .and_then(|remote| remote.url().map(standard_changelog::detect_host))
        .unwrap_or(RepoHost::Unknown)
}

/// Build a `VersionRelease` from raw commits for changelog generation.
fn build_version_release(
    commits: &[(git2::Oid, String)],
    version: &str,
    prev_tag: Option<&str>,
    config: &standard_changelog::ChangelogConfig,
) -> Option<VersionRelease> {
    let section_map: HashMap<&str, &str> = config
        .sections
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let hidden_set: std::collections::HashSet<&str> =
        config.hidden.iter().map(|s| s.as_str()).collect();

    let mut groups_map: HashMap<String, Vec<standard_changelog::ChangelogEntry>> = HashMap::new();
    let mut breaking_changes = Vec::new();

    for (oid, message) in commits {
        let parsed = match standard_commit::parse(message) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if hidden_set.contains(parsed.r#type.as_str()) {
            continue;
        }

        let section_title = match section_map.get(parsed.r#type.as_str()) {
            Some(title) => (*title).to_string(),
            None => continue,
        };

        let short_hash = format!("{oid}")[..7].to_string();

        let mut refs = Vec::new();
        for footer in &parsed.footers {
            match footer.token.as_str() {
                "BREAKING CHANGE" | "BREAKING-CHANGE" => {
                    breaking_changes.push(footer.value.clone());
                }
                "Refs" | "Closes" | "Fixes" | "Resolves" => {
                    let token = footer.token.to_lowercase();
                    for r in footer.value.split(',') {
                        let r = r.trim();
                        if !r.is_empty() {
                            let value = if r.chars().all(|c| c.is_ascii_digit()) {
                                format!("#{r}")
                            } else {
                                r.to_string()
                            };
                            refs.push((token.clone(), value));
                        }
                    }
                }
                _ => {}
            }
        }

        let entry = standard_changelog::ChangelogEntry {
            scope: parsed.scope,
            description: parsed.description,
            hash: short_hash,
            is_breaking: parsed.is_breaking,
            refs,
        };

        groups_map.entry(section_title).or_default().push(entry);
    }

    // Order groups by section config order.
    let sections: Vec<(&str, &str)> = section_map.iter().map(|(k, v)| (*k, *v)).collect();
    let groups: Vec<(String, Vec<standard_changelog::ChangelogEntry>)> = sections
        .iter()
        .filter_map(|(_, title)| {
            groups_map
                .remove(*title)
                .map(|entries| (title.to_string(), entries))
        })
        .collect();

    if groups.is_empty() && breaking_changes.is_empty() {
        return None;
    }

    // Use today's date.
    let now = chrono_date();

    Some(VersionRelease {
        tag: version.to_string(),
        date: now,
        prev_tag: prev_tag.map(|t| t.strip_prefix('v').unwrap_or(t).to_string()),
        groups,
        breaking_changes,
    })
}

/// Get today's date as YYYY-MM-DD without pulling in a datetime crate.
fn chrono_date() -> String {
    // Use the same algorithm as changelog.rs to get date from system time.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = secs / 86400;
    let (year, month, day) = days_to_date(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_date(mut days: i64) -> (i64, i64, i64) {
    days += 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Find the relative path to the project's Cargo.toml.
fn find_cargo_toml(repo: &git2::Repository) -> Option<String> {
    let workdir = repo.workdir()?;
    let path = workdir.join("Cargo.toml");
    if path.exists() {
        Some("Cargo.toml".to_string())
    } else {
        None
    }
}

/// Update the `version` field in a Cargo.toml file.
fn update_cargo_toml_version(
    path: &std::path::Path,
    new_version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let mut doc: toml::Table = content.parse()?;

    if let Some(pkg) = doc.get_mut("package").and_then(|v| v.as_table_mut()) {
        pkg.insert(
            "version".to_string(),
            toml::Value::String(new_version.to_string()),
        );
    } else {
        return Err("no [package] section found in Cargo.toml".into());
    }

    // Preserve formatting by doing a targeted replacement instead of
    // rewriting the entire file through toml serialisation.
    let updated = replace_version_in_toml(&content, new_version)?;
    std::fs::write(path, updated)?;
    Ok(())
}

/// Replace the version value in a TOML string while preserving formatting.
fn replace_version_in_toml(
    content: &str,
    new_version: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Find `version = "..."` in the [package] section.
    let mut in_package = false;
    let mut result = String::new();
    let mut replaced = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[package]" {
            in_package = true;
        } else if trimmed.starts_with('[') {
            in_package = false;
        }

        if in_package && !replaced && trimmed.starts_with("version") {
            // Replace the version value.
            if let Some(eq_pos) = line.find('=') {
                let prefix = &line[..=eq_pos];
                result.push_str(prefix);
                result.push_str(&format!(" \"{new_version}\""));
                result.push('\n');
                replaced = true;
                continue;
            }
        }

        result.push_str(line);
        result.push('\n');
    }

    if !replaced {
        return Err("could not find version field in [package] section".into());
    }

    // Remove trailing extra newline if the original didn't end with one.
    if !content.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    Ok(result)
}

/// Stage all modified and new files in the working directory.
fn stage_files(
    repo: &git2::Repository,
    workdir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut index = repo.index()?;

    // Stage known modified files.
    let cargo_toml = workdir.join("Cargo.toml");
    if cargo_toml.exists() {
        index.add_path(std::path::Path::new("Cargo.toml"))?;
    }

    let changelog = workdir.join("CHANGELOG.md");
    if changelog.exists() {
        index.add_path(std::path::Path::new("CHANGELOG.md"))?;
    }

    // Also stage Cargo.lock if it was updated.
    let cargo_lock = workdir.join("Cargo.lock");
    if cargo_lock.exists() {
        index.add_path(std::path::Path::new("Cargo.lock"))?;
    }

    index.write()?;
    Ok(())
}

/// Create a commit using git2.
fn create_commit(repo: &git2::Repository, message: &str) -> Result<(), Box<dyn std::error::Error>> {
    let sig = repo.signature()?;
    let mut index = repo.index()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let parent = repo.head()?.peel_to_commit()?;
    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])?;

    Ok(())
}

/// Create a signed commit by shelling out to `git`.
fn create_signed_commit(message: &str) -> Result<(), Box<dyn std::error::Error>> {
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
fn create_annotated_tag(
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
fn create_signed_tag(name: &str, message: &str) -> Result<(), Box<dyn std::error::Error>> {
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
    fn replace_version_in_toml_basic() {
        let input = r#"[package]
name = "my-crate"
version = "0.1.0"
edition = "2021"
"#;
        let result = replace_version_in_toml(input, "1.0.0").unwrap();
        assert!(result.contains("version = \"1.0.0\""));
        assert!(result.contains("name = \"my-crate\""));
        assert!(result.contains("edition = \"2021\""));
    }

    #[test]
    fn replace_version_only_in_package_section() {
        let input = r#"[package]
name = "my-crate"
version = "0.1.0"

[dependencies]
foo = { version = "1.0" }
"#;
        let result = replace_version_in_toml(input, "2.0.0").unwrap();
        assert!(result.contains("[package]"));
        assert!(result.contains("version = \"2.0.0\""));
        // Dependency version should be unchanged.
        assert!(result.contains("foo = { version = \"1.0\" }"));
    }

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

    #[test]
    fn chrono_date_format() {
        let date = chrono_date();
        // Should be YYYY-MM-DD format.
        assert_eq!(date.len(), 10);
        assert_eq!(&date[4..5], "-");
        assert_eq!(&date[7..8], "-");
    }
}
