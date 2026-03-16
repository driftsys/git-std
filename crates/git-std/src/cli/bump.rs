use std::path::PathBuf;

use standard_changelog::VersionRelease;
use standard_version::{CustomVersionFile, UpdateResult};
use yansi::Paint;

use crate::config::{ProjectConfig, Scheme};
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
    if config.scheme == Scheme::Calver {
        return run_calver(config, opts);
    }

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
            standard_version::BumpLevel::Major => "major \u{2014} breaking change detected",
            standard_version::BumpLevel::Minor => "minor \u{2014} new feature",
            standard_version::BumpLevel::Patch => "patch \u{2014} bug fix",
        };
        reason.to_string()
    };

    eprintln!();
    eprintln!(
        "  {} ({bump_reason})",
        format!("{cur_ver} \u{2192} {new_version}").bold()
    );

    let workdir = match repo.workdir() {
        Some(w) => w,
        None => {
            eprintln!("error: bare repository not supported");
            return 1;
        }
    };

    let custom_files: Vec<CustomVersionFile> = config
        .version_files
        .iter()
        .map(|vf| CustomVersionFile {
            path: PathBuf::from(&vf.path),
            pattern: vf.regex.clone(),
        })
        .collect();

    // --- Dry run: print plan and exit ---
    if opts.dry_run {
        eprintln!();

        match standard_version::detect_version_files(workdir, &custom_files) {
            Ok(detected) if detected.is_empty() => {
                eprintln!("  No version files detected");
            }
            Ok(detected) => {
                eprintln!("  Would update:");
                for f in &detected {
                    let rel = f.path.strip_prefix(workdir).unwrap_or(&f.path).display();
                    eprintln!("    {:<20} {} \u{2192} {new_version}", rel, f.old_version);
                }
            }
            Err(e) => {
                eprintln!("  warning: cannot detect version files: {e}");
            }
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

    // Step 7: Update all detected version files.
    let version_results: Vec<UpdateResult> = match standard_version::update_version_files(
        workdir,
        &new_version.to_string(),
        &custom_files,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: cannot update version files: {e}");
            return 1;
        }
    };

    // Sync Cargo.lock only when a Cargo.toml was actually updated.
    let cargo_updated = version_results.iter().any(|r| r.name == "Cargo.toml");
    if cargo_updated {
        let status = std::process::Command::new("cargo")
            .args(["update", "--workspace"])
            .status();
        if let Err(e) = status {
            eprintln!("warning: failed to update Cargo.lock: {e}");
        }
    }

    // Step 8: Generate/update changelog.
    if !opts.skip_changelog {
        let changelog_config = config.to_changelog_config();
        let host = git::detect_host_from_repo(&repo);
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
    if !version_results.is_empty() {
        eprintln!();
        eprintln!("  Updated:");
        for r in &version_results {
            let rel = r.path.strip_prefix(workdir).unwrap_or(&r.path).display();
            eprintln!(
                "    {:<20} {} \u{2192} {}",
                rel, r.old_version, r.new_version
            );
            if let Some(ref extra) = r.extra {
                eprintln!("    {:<20} {extra}", "");
            }
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
        // Collect paths to stage: all updated version files + changelog + Cargo.lock.
        let rel_paths: Vec<String> = version_results
            .iter()
            .filter_map(|r| {
                r.path
                    .strip_prefix(workdir)
                    .ok()
                    .map(|p| p.to_string_lossy().into_owned())
            })
            .collect();
        let mut paths_to_stage: Vec<&str> = rel_paths.iter().map(|s| s.as_str()).collect();
        if !opts.skip_changelog {
            paths_to_stage.push("CHANGELOG.md");
        }
        if cargo_updated {
            paths_to_stage.push("Cargo.lock");
        }

        if let Err(e) = git::stage_files(&repo, &paths_to_stage) {
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

/// Compute today's [`standard_version::calver::CalverDate`] using the Howard
/// Hinnant civil_from_days algorithm (no external date crate needed).
fn today_calver_date() -> standard_version::calver::CalverDate {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    calver_date_from_epoch_days(secs.div_euclid(86400) as i32)
}

/// Compute a [`CalverDate`] from days since the Unix epoch.
fn calver_date_from_epoch_days(days: i32) -> standard_version::calver::CalverDate {
    // Howard Hinnant's civil_from_days algorithm.
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097) as u32; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
    let y = yoe as i32 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month pseudo [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let y = if m <= 2 { y + 1 } else { y };

    // ISO week number: ISO weeks start on Monday.
    // day_of_week: Monday=1 .. Sunday=7
    let dow = ((days + 3).rem_euclid(7) + 1) as u32; // Unix epoch was Thursday (4), so +3 maps to Mon=1
    // ISO week: the week containing January 4th is week 1.
    // ordinal day of year (1-based, with year starting in January)
    let jan1_days = {
        // days_from_civil for (y, 1, 1) using Howard Hinnant algorithm.
        // January is month 1 which is <= 2, so shift year by -1.
        let ys = y - 1;
        let eras = ys.div_euclid(400);
        let yoes = ys.rem_euclid(400) as u32;
        let ms: u32 = 10; // m=1 → mp = m + 9 = 10 (March-based)
        let ds: u32 = 1;
        let doys = (153 * ms + 2) / 5 + ds - 1;
        let does = yoes * 365 + yoes / 4 - yoes / 100 + doys;
        eras * 146097 + does as i32 - 719468
    };
    let ordinal = days - jan1_days + 1;
    let jan1_dow = (jan1_days + 3).rem_euclid(7) + 1;

    // ISO week calculation
    let iso_week = {
        let w = (ordinal - dow as i32 + 10) / 7;
        if w < 1 {
            // Last week of previous year — compute that year's week count.
            // Simplified: just return 52 or 53.
            let prev_jan1_dow = (jan1_days - 1 + 3).rem_euclid(7) + 1;
            if prev_jan1_dow == 4
                || (prev_jan1_dow == 3 && {
                    // Check if previous year is leap
                    let py = y - 1;
                    py % 4 == 0 && (py % 100 != 0 || py % 400 == 0)
                })
            {
                53
            } else {
                52
            }
        } else if w > 52 {
            // Check if it belongs to week 1 of next year.
            let is_leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
            let days_in_year = if is_leap { 366 } else { 365 };
            if ordinal > days_in_year - 3 && jan1_dow != 4 {
                1
            } else {
                w
            }
        } else {
            w
        }
    };

    standard_version::calver::CalverDate {
        year: y as u32,
        month: m,
        day: d,
        iso_week: iso_week as u32,
        day_of_week: dow,
    }
}

/// Run the bump subcommand in calver mode.
fn run_calver(config: &ProjectConfig, opts: &BumpOptions) -> i32 {
    let repo = match git2::Repository::discover(".") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: cannot open repository: {e}");
            return 1;
        }
    };

    let tag_prefix = &config.versioning.tag_prefix;
    let calver_format = &config.versioning.calver_format;

    // Calver does not support pre-release versioning.
    if opts.prerelease.is_some() {
        eprintln!("error: --prerelease is not supported with scheme = \"calver\"");
        return 1;
    }

    // Validate the calver format.
    if let Err(e) = standard_version::calver::validate_format(calver_format) {
        eprintln!("error: invalid calver format: {e}");
        return 1;
    }

    // Find the latest calver tag.
    let current_tag = match git::find_latest_calver_tag(&repo, tag_prefix) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    // Resolve HEAD.
    let head_oid = match repo.head().and_then(|h| h.peel_to_commit().map(|c| c.id())) {
        Ok(oid) => oid,
        Err(e) => {
            eprintln!("error: cannot resolve HEAD: {e}");
            return 1;
        }
    };

    // Collect commits since the tag.
    let tag_oid = current_tag.as_ref().map(|(oid, _)| *oid);
    let raw_commits = match git::walk_commits(&repo, head_oid, tag_oid) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    let prev_ver = current_tag.as_ref().map(|(_, v)| v.as_str());

    // Compute next calver version.
    let date = today_calver_date();
    let new_version = if let Some(ref forced) = opts.release_as {
        forced.clone()
    } else if opts.first_release {
        match standard_version::calver::next_version(calver_format, date, None) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("error: {e}");
                return 1;
            }
        }
    } else {
        match standard_version::calver::next_version(calver_format, date, prev_ver) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("error: {e}");
                return 1;
            }
        }
    };

    eprintln!();
    eprintln!(
        "  {} (calver)",
        format!("{} \u{2192} {new_version}", prev_ver.unwrap_or("none")).bold()
    );

    let workdir = match repo.workdir() {
        Some(w) => w,
        None => {
            eprintln!("error: bare repository not supported");
            return 1;
        }
    };

    let custom_files: Vec<CustomVersionFile> = config
        .version_files
        .iter()
        .map(|vf| CustomVersionFile {
            path: PathBuf::from(&vf.path),
            pattern: vf.regex.clone(),
        })
        .collect();

    // Dry run.
    if opts.dry_run {
        eprintln!();
        match standard_version::detect_version_files(workdir, &custom_files) {
            Ok(detected) if detected.is_empty() => {
                eprintln!("  No version files detected");
            }
            Ok(detected) => {
                eprintln!("  Would update:");
                for f in &detected {
                    let rel = f.path.strip_prefix(workdir).unwrap_or(&f.path).display();
                    eprintln!("    {:<20} {} \u{2192} {new_version}", rel, f.old_version);
                }
            }
            Err(e) => {
                eprintln!("  warning: cannot detect version files: {e}");
            }
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

    // Update version files.
    let version_results: Vec<UpdateResult> =
        match standard_version::update_version_files(workdir, &new_version, &custom_files) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("error: cannot update version files: {e}");
                return 1;
            }
        };

    let cargo_updated = version_results.iter().any(|r| r.name == "Cargo.toml");
    if cargo_updated {
        let status = std::process::Command::new("cargo")
            .args(["update", "--workspace"])
            .status();
        if let Err(e) = status {
            eprintln!("warning: failed to update Cargo.lock: {e}");
        }
    }

    // Generate changelog.
    if !opts.skip_changelog {
        let changelog_config = config.to_changelog_config();
        let host = git::detect_host_from_repo(&repo);
        let changelog_path = workdir.join("CHANGELOG.md");

        let release =
            build_version_release(&raw_commits, &new_version, prev_ver, &changelog_config);

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
    if !version_results.is_empty() {
        eprintln!();
        eprintln!("  Updated:");
        for r in &version_results {
            let rel = r.path.strip_prefix(workdir).unwrap_or(&r.path).display();
            eprintln!(
                "    {:<20} {} \u{2192} {}",
                rel, r.old_version, r.new_version
            );
            if let Some(ref extra) = r.extra {
                eprintln!("    {:<20} {extra}", "");
            }
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

    // Commit.
    if !opts.no_commit {
        let rel_paths: Vec<String> = version_results
            .iter()
            .filter_map(|r| {
                r.path
                    .strip_prefix(workdir)
                    .ok()
                    .map(|p| p.to_string_lossy().into_owned())
            })
            .collect();
        let mut paths_to_stage: Vec<&str> = rel_paths.iter().map(|s| s.as_str()).collect();
        if !opts.skip_changelog {
            paths_to_stage.push("CHANGELOG.md");
        }
        if cargo_updated {
            paths_to_stage.push("Cargo.lock");
        }

        if let Err(e) = git::stage_files(&repo, &paths_to_stage) {
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

    // Tag.
    if !opts.no_commit && !opts.no_tag {
        let tag_name = format!("{tag_prefix}{new_version}");
        let tag_msg = new_version.to_string();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn today_calver_date_is_reasonable() {
        let d = today_calver_date();
        assert!(d.year >= 2024);
        assert!((1..=12).contains(&d.month));
        assert!((1..=31).contains(&d.day));
        assert!((1..=53).contains(&d.iso_week));
        assert!((1..=7).contains(&d.day_of_week));
    }

    #[test]
    fn calver_date_2026_03_16() {
        // 2026-03-16 is a Monday, ISO week 12
        // Days since epoch: (2026-1970)*365 + leap days + day_of_year
        let days = 20528; // 2026-03-16
        let d = calver_date_from_epoch_days(days);
        assert_eq!(d.year, 2026);
        assert_eq!(d.month, 3);
        assert_eq!(d.day, 16);
        assert_eq!(d.day_of_week, 1); // Monday
        assert_eq!(d.iso_week, 12);
    }

    #[test]
    fn calver_date_dec31_to_jan1_boundary() {
        // 2026-12-31 is a Thursday, ISO week 53
        let dec31 = 20818; // 2026-12-31
        let d = calver_date_from_epoch_days(dec31);
        assert_eq!(d.year, 2026);
        assert_eq!(d.month, 12);
        assert_eq!(d.day, 31);
        assert_eq!(d.day_of_week, 4); // Thursday

        // 2027-01-01 is a Friday, ISO week 53 (still belongs to 2026's week 53)
        let jan1 = 20819; // 2027-01-01
        let d = calver_date_from_epoch_days(jan1);
        assert_eq!(d.year, 2027);
        assert_eq!(d.month, 1);
        assert_eq!(d.day, 1);
        assert_eq!(d.day_of_week, 5); // Friday
    }

    #[test]
    fn calver_date_jan1_2024_monday() {
        // 2024-01-01 is a Monday, ISO week 1
        let days = 19723; // 2024-01-01
        let d = calver_date_from_epoch_days(days);
        assert_eq!(d.year, 2024);
        assert_eq!(d.month, 1);
        assert_eq!(d.day, 1);
        assert_eq!(d.day_of_week, 1); // Monday
        assert_eq!(d.iso_week, 1);
    }

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
