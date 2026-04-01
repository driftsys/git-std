use anyhow::Result;
use standard_changelog::{ChangelogConfig, RepoHost, VersionRelease};

use crate::config::ProjectConfig;
use crate::git;
use crate::ui;

/// Options for the changelog subcommand.
pub struct ChangelogOptions {
    /// Regenerate the entire changelog from the first commit.
    pub full: bool,
    /// Write to file. `None` = stdout, `Some(path)` = write to file.
    pub write: Option<String>,
    /// Optional git revision range (e.g. `v1.0.0..v2.0.0`).
    pub range: Option<String>,
    /// Generate changelog for a specific package (monorepo only).
    pub package: Option<String>,
    /// Whether monorepo mode is enabled.
    pub monorepo: bool,
    /// Tag template for per-package tags.
    pub tag_template: String,
    /// Tag prefix for root tags.
    pub tag_prefix: String,
}

/// Run the changelog subcommand. Returns the exit code.
pub fn run(
    project_config: &ProjectConfig,
    config: &ChangelogConfig,
    opts: &ChangelogOptions,
) -> i32 {
    let dir = std::path::Path::new(".");
    let host = git::detect_host(dir);

    // Per-package changelog mode.
    if let Some(ref pkg_name) = opts.package {
        if !opts.monorepo {
            ui::error("--package requires monorepo = true");
            return 1;
        }
        return run_package_changelog(dir, project_config, config, &host, opts, pkg_name);
    }

    // Full mode in monorepo generates root + all per-package changelogs.
    if opts.full && opts.monorepo {
        return run_full_monorepo(dir, project_config, config, &host, opts);
    }

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

    if opts.write.is_none() {
        let section = standard_changelog::render_version(&release, config, host);
        println!("# {}\n", config.title);
        print!("{section}");
        return 0;
    }

    let path = opts.write.as_deref().unwrap();
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let output = standard_changelog::prepend_release(&existing, &release, config, host);
    write_output(&output, opts)
}

/// Write output to stdout or file.
fn write_output(content: &str, opts: &ChangelogOptions) -> i32 {
    match &opts.write {
        None => {
            print!("{content}");
        }
        Some(path) => {
            if let Err(e) = std::fs::write(path, content) {
                ui::error(&format!("cannot write {path}: {e}"));
                return 1;
            }
            ui::info(&format!("wrote {path}"));
        }
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
) -> Result<Option<VersionRelease>> {
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

    // If the range produced no commits, check if the user reversed the arguments.
    // Probe the inverse direction — if that yields commits, the range is backwards.
    if commits.is_empty() {
        let inverse_has_commits = git::walk_commits(dir, &from_oid, Some(&to_oid))
            .map(|c| !c.is_empty())
            .unwrap_or(false);
        if inverse_has_commits {
            ui::warning(&format!("range '{range}' is empty"));
            ui::hint(&format!("did you mean '{to_spec}..{from_spec}'?"));
            return 1;
        }
    }

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

    if opts.write.is_none() {
        let section = standard_changelog::render_version(&release, config, host);
        print!("{section}");
        return 0;
    }

    let path = opts.write.as_deref().unwrap();
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let output = standard_changelog::prepend_release(&existing, &release, config, host);
    write_output(&output, opts)
}

/// Build version releases from git history.
fn build_releases(dir: &std::path::Path, config: &ChangelogConfig) -> Result<Vec<VersionRelease>> {
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

// ── Per-package changelog ──────────────────────────────────────────

/// Generate changelog for a single package using path-filtered commits.
fn run_package_changelog(
    dir: &std::path::Path,
    project_config: &ProjectConfig,
    config: &ChangelogConfig,
    host: &RepoHost,
    opts: &ChangelogOptions,
    pkg_name: &str,
) -> i32 {
    let workdir = match git::workdir(dir) {
        Ok(w) => w,
        Err(_) => {
            ui::error("bare repository not supported");
            return 1;
        }
    };

    let packages = project_config.resolved_packages(&workdir);
    let pkg = match packages.iter().find(|p| p.name == pkg_name) {
        Some(p) => p,
        None => {
            ui::error(&format!("unknown package: {pkg_name}"));
            return 1;
        }
    };

    let head_oid = match git::head_oid(dir) {
        Ok(oid) => oid,
        Err(e) => {
            ui::error(&format!("cannot resolve HEAD: {e}"));
            return 1;
        }
    };

    // Find latest per-package tag.
    let tag_template = &opts.tag_template;
    let prefix = tag_template
        .replace("{name}", pkg_name)
        .replace("{version}", "");

    let tags = match git::collect_tags(dir) {
        Ok(t) => t,
        Err(e) => {
            ui::error(&e.to_string());
            return 1;
        }
    };

    let latest_tag: Option<(String, String)> = tags
        .iter()
        .filter(|(_, name)| name.starts_with(&prefix))
        .max_by_key(|(_, name)| {
            name.strip_prefix(&prefix)
                .and_then(|v| semver::Version::parse(v).ok())
        })
        .map(|(oid, name)| (oid.clone(), name.clone()));

    let tag_oid = latest_tag.as_ref().map(|(oid, _)| oid.as_str());
    let commits = match git::walk_commits_for_path(dir, &head_oid, tag_oid, &[&pkg.path]) {
        Ok(c) => c,
        Err(e) => {
            ui::error(&format!("cannot walk commits: {e}"));
            return 1;
        }
    };

    if commits.is_empty() {
        ui::print(&format!("no unreleased changes found for {pkg_name}"));
        return 0;
    }

    let prev_tag_name = latest_tag.as_ref().map(|(_, name)| name.as_str());
    let release = match build_release_from_commits(&commits, "Unreleased", prev_tag_name, config) {
        Some(mut r) => {
            if let Ok(date) = git::commit_date(dir, &head_oid) {
                r.date = date;
            }
            r
        }
        None => {
            ui::print(&format!("no conventional commits found for {pkg_name}"));
            return 0;
        }
    };

    if opts.write.is_none() {
        let section = standard_changelog::render_version(&release, config, host);
        print!("{section}");
        return 0;
    }

    let filename = opts.write.as_deref().unwrap();
    let output_path = workdir.join(&pkg.path).join(filename);
    let existing = std::fs::read_to_string(&output_path).unwrap_or_default();
    let output = standard_changelog::prepend_release(&existing, &release, config, host);

    if let Err(e) = std::fs::write(&output_path, &output) {
        ui::error(&format!("cannot write {}: {e}", output_path.display()));
        return 1;
    }
    ui::info(&format!("wrote {}", output_path.display()));
    0
}

/// Full monorepo changelog: regenerate root + all per-package changelogs.
fn run_full_monorepo(
    dir: &std::path::Path,
    project_config: &ProjectConfig,
    config: &ChangelogConfig,
    host: &RepoHost,
    opts: &ChangelogOptions,
) -> i32 {
    // Root changelog (all commits).
    let code = run_full(dir, config, host, opts);
    if code != 0 {
        return code;
    }

    let workdir = match git::workdir(dir) {
        Ok(w) => w,
        Err(_) => {
            ui::error("bare repository not supported");
            return 1;
        }
    };

    let packages = project_config.resolved_packages(&workdir);
    for pkg in &packages {
        let pkg_opts = ChangelogOptions {
            full: false,
            write: opts.write.as_ref().map(|_| "CHANGELOG.md".to_string()),
            range: None,
            package: Some(pkg.name.clone()),
            monorepo: true,
            tag_template: opts.tag_template.clone(),
            tag_prefix: opts.tag_prefix.clone(),
        };
        let code = run_package_changelog(dir, project_config, config, host, &pkg_opts, &pkg.name);
        if code != 0 {
            return code;
        }
    }

    0
}
