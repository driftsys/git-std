use standard_changelog::VersionRelease;
use yansi::Paint;

use crate::config::ProjectConfig;
use crate::git;

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
    let current_version = match git::find_latest_version_tag(&repo, tag_prefix) {
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
    let raw_commits = match git::walk_commits(&repo, head_oid, tag_oid) {
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

        let cargo_toml = git::find_cargo_toml(&repo);
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
    if let Some(ref path) = git::find_cargo_toml(&repo) {
        let workdir = match repo.workdir() {
            Some(w) => w,
            None => {
                eprintln!("error: bare repository not supported");
                return 1;
            }
        };
        let full_path = workdir.join(path);
        if let Err(e) = git::update_cargo_toml_version(&full_path, &new_version.to_string()) {
            eprintln!("error: cannot update {path}: {e}");
            return 1;
        }
        updated_files.push((path.clone(), cur_ver.to_string(), new_version.to_string()));
    }

    // Step 8: Generate/update changelog.
    if !opts.skip_changelog {
        let changelog_config = config.to_changelog_config();
        let host = git::detect_host_from_repo(&repo);
        let workdir = match repo.workdir() {
            Some(w) => w,
            None => {
                eprintln!("error: bare repository not supported");
                return 1;
            }
        };
        let changelog_path = workdir.join("CHANGELOG.md");

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

        if let Err(e) = git::stage_files(&repo, workdir) {
            eprintln!("error: cannot stage files: {e}");
            return 1;
        }

        let commit_msg = format!("chore(release): {new_version}");

        if opts.sign {
            if let Err(e) = git::create_signed_commit(&commit_msg) {
                eprintln!("error: {e}");
                return 1;
            }
        } else if let Err(e) = git::create_commit(&repo, &commit_msg) {
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
            if let Err(e) = git::create_signed_tag(&tag_name, &tag_msg) {
                eprintln!("error: {e}");
                return 1;
            }
        } else if let Err(e) = git::create_annotated_tag(&repo, &tag_name, &tag_msg) {
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

/// Build a `VersionRelease` from raw commits for changelog generation.
fn build_version_release(
    commits: &[(git2::Oid, String)],
    version: &str,
    prev_tag: Option<&str>,
    config: &standard_changelog::ChangelogConfig,
) -> Option<VersionRelease> {
    let pairs: Vec<(String, &str)> = commits
        .iter()
        .map(|(oid, msg)| (format!("{oid}")[..7].to_string(), msg.as_str()))
        .collect();
    let refs: Vec<(&str, &str)> = pairs.iter().map(|(h, m)| (h.as_str(), *m)).collect();

    let mut release = standard_changelog::build_release(&refs, version, prev_tag, config)?;

    // Use today's date.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    release.date = standard_changelog::format_date(secs);

    Some(release)
}

#[cfg(test)]
mod tests {
    #[test]
    fn chrono_date_format() {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let date = standard_changelog::format_date(secs);
        // Should be YYYY-MM-DD format.
        assert_eq!(date.len(), 10);
        assert_eq!(&date[4..5], "-");
        assert_eq!(&date[7..8], "-");
    }
}
