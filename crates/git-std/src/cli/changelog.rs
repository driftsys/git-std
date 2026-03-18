use standard_changelog::{ChangelogConfig, RepoHost, VersionRelease};

use crate::git;
use crate::ui;

/// Options for the changelog subcommand.
pub struct ChangelogOptions {
    /// Regenerate the entire changelog from the first commit.
    pub full: bool,
    /// Print to stdout instead of writing to a file.
    pub stdout: bool,
    /// Output file path.
    pub output: String,
    /// Optional git revision range (e.g. `v1.0.0..v2.0.0`).
    pub range: Option<String>,
}

/// Run the changelog subcommand. Returns the exit code.
pub fn run(config: &ChangelogConfig, opts: &ChangelogOptions) -> i32 {
    let dir = std::path::Path::new(".");

    let host = git::detect_host(dir);

    if let Some(ref range) = opts.range {
        run_range(dir, config, &host, opts, range)
    } else if opts.full {
        run_full(dir, config, &host, opts)
    } else {
        run_incremental(dir, config, &host, opts)
    }
}

/// Full regeneration: render all releases from git history.
fn run_full(
    dir: &std::path::Path,
    config: &ChangelogConfig,
    host: &RepoHost,
    opts: &ChangelogOptions,
) -> i32 {
    let releases = match build_releases(dir, config) {
        Ok(r) => r,
        Err(e) => {
            ui::error(&e.to_string());
            return 1;
        }
    };

    if releases.is_empty() {
        ui::error("no releases found");
        return 1;
    }

    let output = standard_changelog::render(&releases, config, host);
    write_output(&output, opts)
}

/// Incremental: render only unreleased commits and prepend to existing file.
fn run_incremental(
    dir: &std::path::Path,
    config: &ChangelogConfig,
    host: &RepoHost,
    opts: &ChangelogOptions,
) -> i32 {
    let release = match build_unreleased(dir, config) {
        Ok(Some(r)) => r,
        Ok(None) => {
            ui::print("no unreleased changes found");
            return 0;
        }
        Err(e) => {
            ui::error(&e.to_string());
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
            ui::error(&format!("cannot write {}: {e}", opts.output));
            return 1;
        }
        ui::info(&format!("wrote {}", opts.output));
    }
    0
}

/// Convert raw `(String, String)` commits to `(&str, &str)` pairs and call
/// `standard_changelog::build_release`.
pub(crate) fn build_release_from_commits(
    commits: &[(String, String)],
    version: &str,
    prev_tag: Option<&str>,
    config: &ChangelogConfig,
) -> Option<VersionRelease> {
    let pairs: Vec<(String, &str)> = commits
        .iter()
        .map(|(oid, msg)| (oid[..7].to_string(), msg.as_str()))
        .collect();
    let refs: Vec<(&str, &str)> = pairs.iter().map(|(h, m)| (h.as_str(), *m)).collect();
    standard_changelog::build_release(&refs, version, prev_tag, config)
}

/// Build only the unreleased version (commits since the last tag).
fn build_unreleased(
    dir: &std::path::Path,
    config: &ChangelogConfig,
) -> Result<Option<VersionRelease>, Box<dyn std::error::Error>> {
    let tags = git::collect_tags(dir)?;

    let head_oid = git::head_oid(dir)?;
    let newest_tag_oid = tags.first().map(|(oid, _)| oid.as_str());

    if newest_tag_oid == Some(head_oid.as_str()) {
        return Ok(None);
    }

    let commits = git::walk_commits(dir, &head_oid, newest_tag_oid)?;
    let newest_tag_name = tags.first().map(|(_, name)| name.as_str());

    let mut release =
        match build_release_from_commits(&commits, "Unreleased", newest_tag_name, config) {
            Some(r) => r,
            None => return Ok(None),
        };

    release.date = git::commit_date(dir, &head_oid)?;
    Ok(Some(release))
}

/// Render a changelog for a specific git revision range (e.g. `v1.0.0..v2.0.0`).
fn run_range(
    dir: &std::path::Path,
    config: &ChangelogConfig,
    host: &RepoHost,
    opts: &ChangelogOptions,
    range: &str,
) -> i32 {
    let (from_spec, to_spec) = match range.split_once("..") {
        Some(pair) => pair,
        None => {
            ui::error("range must contain '..' (e.g. v1.0.0..v2.0.0)");
            return 1;
        }
    };

    let from_oid = match git::resolve_rev(dir, from_spec) {
        Ok(oid) => oid,
        Err(e) => {
            ui::error(&format!("cannot resolve '{from_spec}': {e}"));
            return 1;
        }
    };

    let to_oid = match git::resolve_rev(dir, to_spec) {
        Ok(oid) => oid,
        Err(e) => {
            ui::error(&format!("cannot resolve '{to_spec}': {e}"));
            return 1;
        }
    };

    let commits = match git::walk_commits(dir, &to_oid, Some(&from_oid)) {
        Ok(c) => c,
        Err(e) => {
            ui::error(&e.to_string());
            return 1;
        }
    };

    // Use the "to" ref as the version label, stripping a leading 'v' if present.
    let version = to_spec.strip_prefix('v').unwrap_or(to_spec);

    let release = match build_release_from_commits(&commits, version, Some(from_spec), config) {
        Some(mut r) => {
            // Use the commit date of the "to" ref.
            if let Ok(date) = git::commit_date(dir, &to_oid) {
                r.date = date;
            }
            r
        }
        None => {
            ui::info(&format!("no conventional commits found in range {range}"));
            return 0;
        }
    };

    if opts.stdout {
        let section = standard_changelog::render_version(&release, config, host);
        print!("{section}");
        return 0;
    }

    let existing = std::fs::read_to_string(&opts.output).unwrap_or_default();
    let output = standard_changelog::prepend_release(&existing, &release, config, host);
    write_output(&output, opts)
}

/// Build version releases from git history.
fn build_releases(
    dir: &std::path::Path,
    config: &ChangelogConfig,
) -> Result<Vec<VersionRelease>, Box<dyn std::error::Error>> {
    let tags = git::collect_tags(dir)?;

    let mut releases = Vec::new();

    // Collect commits between HEAD and the newest tag as "Unreleased".
    let head_oid = git::head_oid(dir)?;
    let newest_tag_oid = tags.first().map(|(oid, _)| oid.as_str());
    if newest_tag_oid != Some(head_oid.as_str()) {
        let unreleased_commits = git::walk_commits(dir, &head_oid, newest_tag_oid)?;
        if let Some(release) =
            build_release_from_commits(&unreleased_commits, "Unreleased", None, config)
        {
            let mut release = release;
            release.date = git::commit_date(dir, &head_oid)?;
            releases.push(release);
        }
    }

    for (i, (tag_oid, tag_name)) in tags.iter().enumerate() {
        let prev_tag = tags.get(i + 1).map(|(_, name)| name.clone());
        let prev_oid = tags.get(i + 1).map(|(oid, _)| oid.as_str());

        let version = tag_name.strip_prefix('v').unwrap_or(tag_name);

        let date = git::commit_date(dir, tag_oid)?;

        let commits = git::walk_commits(dir, tag_oid, prev_oid)?;

        if let Some(mut release) =
            build_release_from_commits(&commits, version, prev_tag.as_deref(), config)
        {
            release.date = date;
            releases.push(release);
        }
    }

    Ok(releases)
}
