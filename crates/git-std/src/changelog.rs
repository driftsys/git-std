use std::collections::HashMap;

use standard_changelog::{ChangelogConfig, ChangelogEntry, RepoHost, VersionRelease};

/// Options for the changelog subcommand.
pub struct ChangelogOptions {
    /// Regenerate the entire changelog from the first commit.
    pub full: bool,
    /// Print to stdout instead of writing to a file.
    pub stdout: bool,
    /// Output file path.
    pub output: String,
}

/// Run the changelog subcommand. Returns the exit code.
pub fn run(config: &ChangelogConfig, opts: &ChangelogOptions) -> i32 {
    let repo = match git2::Repository::discover(".") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: cannot open repository: {e}");
            return 1;
        }
    };

    let host = detect_host_from_repo(&repo);

    if opts.full {
        run_full(&repo, config, &host, opts)
    } else {
        run_incremental(&repo, config, &host, opts)
    }
}

/// Full regeneration: render all releases from git history.
fn run_full(
    repo: &git2::Repository,
    config: &ChangelogConfig,
    host: &RepoHost,
    opts: &ChangelogOptions,
) -> i32 {
    let releases = match build_releases(repo, config) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    if releases.is_empty() {
        eprintln!("error: no releases found");
        return 1;
    }

    let output = standard_changelog::render(&releases, config, host);
    write_output(&output, opts)
}

/// Incremental: render only unreleased commits and prepend to existing file.
fn run_incremental(
    repo: &git2::Repository,
    config: &ChangelogConfig,
    host: &RepoHost,
    opts: &ChangelogOptions,
) -> i32 {
    let release = match build_unreleased(repo, config) {
        Ok(Some(r)) => r,
        Ok(None) => {
            eprintln!("no unreleased changes found");
            return 0;
        }
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    if opts.stdout {
        let section = standard_changelog::render_version(&release, config, host);
        println!("# {}\n", config.title);
        print!("{section}");
        return 0;
    }

    let existing = std::fs::read_to_string(&opts.output).unwrap_or_default();
    let output = standard_changelog::prepend_release(&existing, &release, config, host);
    write_output(&output, opts)
}

/// Write output to stdout or file.
fn write_output(content: &str, opts: &ChangelogOptions) -> i32 {
    if opts.stdout {
        print!("{content}");
    } else {
        if let Err(e) = std::fs::write(&opts.output, content) {
            eprintln!("error: cannot write {}: {e}", opts.output);
            return 1;
        }
        eprintln!("wrote {}", opts.output);
    }
    0
}

/// Detect the repo host from the `origin` remote URL.
fn detect_host_from_repo(repo: &git2::Repository) -> RepoHost {
    repo.find_remote("origin")
        .ok()
        .and_then(|remote| remote.url().map(standard_changelog::detect_host))
        .unwrap_or(RepoHost::Unknown)
}

/// Collect all tags pointing at commits, sorted by commit time (newest first).
fn collect_tags(repo: &git2::Repository) -> Result<Vec<(git2::Oid, String)>, git2::Error> {
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

/// Build only the unreleased version (commits since the last tag).
fn build_unreleased(
    repo: &git2::Repository,
    config: &ChangelogConfig,
) -> Result<Option<VersionRelease>, Box<dyn std::error::Error>> {
    let tags = collect_tags(repo)?;

    let section_map: HashMap<&str, &str> = config
        .sections
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let hidden_set: std::collections::HashSet<&str> =
        config.hidden.iter().map(|s| s.as_str()).collect();

    let head_oid = repo.head()?.peel_to_commit()?.id();
    let newest_tag_oid = tags.first().map(|(oid, _)| *oid);

    if newest_tag_oid == Some(head_oid) {
        return Ok(None);
    }

    let commits = walk_commits(repo, head_oid, newest_tag_oid)?;
    let newest_tag_name = tags.first().map(|(_, name)| name.as_str());

    let mut release = match build_release(
        &commits,
        "Unreleased",
        newest_tag_name,
        &section_map,
        &hidden_set,
    ) {
        Some(r) => r,
        None => return Ok(None),
    };

    let head_commit = repo.find_commit(head_oid)?;
    release.date = format_commit_date(&head_commit);
    Ok(Some(release))
}

/// Build version releases from git history.
fn build_releases(
    repo: &git2::Repository,
    config: &ChangelogConfig,
) -> Result<Vec<VersionRelease>, Box<dyn std::error::Error>> {
    let tags = collect_tags(repo)?;

    // Build section lookup from config.
    let section_map: HashMap<&str, &str> = config
        .sections
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let hidden_set: std::collections::HashSet<&str> =
        config.hidden.iter().map(|s| s.as_str()).collect();

    let mut releases = Vec::new();

    // Collect commits between HEAD and the newest tag as "Unreleased".
    let head_oid = repo.head()?.peel_to_commit()?.id();
    let newest_tag_oid = tags.first().map(|(oid, _)| *oid);
    if newest_tag_oid != Some(head_oid) {
        let unreleased_commits = walk_commits(repo, head_oid, newest_tag_oid)?;
        if let Some(release) = build_release(
            &unreleased_commits,
            "Unreleased",
            None,
            &section_map,
            &hidden_set,
        ) {
            let head_commit = repo.find_commit(head_oid)?;
            let mut release = release;
            release.date = format_commit_date(&head_commit);
            releases.push(release);
        }
    }

    for (i, (tag_oid, tag_name)) in tags.iter().enumerate() {
        let prev_tag = tags.get(i + 1).map(|(_, name)| name.clone());
        let prev_oid = tags.get(i + 1).map(|(oid, _)| *oid);

        let version = tag_name.strip_prefix('v').unwrap_or(tag_name);

        let tag_commit = repo.find_commit(*tag_oid)?;
        let date = format_commit_date(&tag_commit);

        let commits = walk_commits(repo, *tag_oid, prev_oid)?;

        if let Some(mut release) = build_release(
            &commits,
            version,
            prev_tag.as_deref(),
            &section_map,
            &hidden_set,
        ) {
            release.date = date;
            releases.push(release);
        }
    }

    Ok(releases)
}

/// Group parsed commits into a single `VersionRelease`.
fn build_release(
    commits: &[(git2::Oid, String)],
    version: &str,
    prev_tag: Option<&str>,
    section_map: &HashMap<&str, &str>,
    hidden_set: &std::collections::HashSet<&str>,
) -> Option<VersionRelease> {
    let mut groups_map: HashMap<String, Vec<ChangelogEntry>> = HashMap::new();
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
                    // Split comma-separated refs (e.g. "Closes: #1, #2").
                    for r in footer.value.split(',') {
                        let r = r.trim();
                        if !r.is_empty() {
                            // Normalize bare numbers to #N.
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

        let entry = ChangelogEntry {
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
    let groups: Vec<(String, Vec<ChangelogEntry>)> = sections
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

    Some(VersionRelease {
        tag: version.to_string(),
        date: String::new(),
        prev_tag: prev_tag.map(|t| t.strip_prefix('v').unwrap_or(t).to_string()),
        groups,
        breaking_changes,
    })
}

/// Walk commits from `from_oid` (inclusive) back to `until_oid` (exclusive), or to root.
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

/// Format a commit's time as YYYY-MM-DD.
fn format_commit_date(commit: &git2::Commit) -> String {
    let time = commit.time();
    let secs = time.seconds() + (time.offset_minutes() as i64) * 60;

    // Simple date calculation from unix timestamp.
    let days = secs / 86400;
    let (year, month, day) = days_to_date(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_date(mut days: i64) -> (i64, i64, i64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
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
