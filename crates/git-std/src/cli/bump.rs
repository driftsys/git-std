use std::path::PathBuf;

use standard_changelog::VersionRelease;
use standard_version::{CustomVersionFile, UpdateResult};
use yansi::Paint;

use crate::config::{ProjectConfig, Scheme};
use crate::git;
use crate::ui;

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
    /// Allow breaking changes in patch-only scheme.
    pub force: bool,
    /// Create a stable branch for patch-only releases.
    ///
    /// `None` = flag not used, `Some(None)` = `--stable` without value
    /// (auto-generate branch name), `Some(Some(name))` = custom branch name.
    pub stable: Option<Option<String>>,
    /// Use minor bump instead of major when advancing main after `--stable`.
    pub minor: bool,
}

/// Run the bump subcommand. Returns the exit code.
pub fn run(config: &ProjectConfig, opts: &BumpOptions) -> i32 {
    if opts.stable.is_some() {
        return run_stable(config, opts);
    }
    if config.scheme == Scheme::Calver {
        return run_calver(config, opts);
    }
    if config.scheme == Scheme::Patch {
        return run_patch(config, opts);
    }

    let dir = std::path::Path::new(".");

    let tag_prefix = &config.versioning.tag_prefix;

    // Step 1: Find latest version tag.
    let current_version = match git::find_latest_version_tag(dir, tag_prefix) {
        Ok(Some((oid, ver))) => Some((oid, ver)),
        Ok(None) => None,
        Err(e) => {
            ui::error(&e.to_string());
            return 1;
        }
    };

    // Step 2: Collect commits since that tag.
    let head_oid = match git::head_oid(dir) {
        Ok(oid) => oid,
        Err(e) => {
            ui::error(&format!("cannot resolve HEAD: {e}"));
            return 1;
        }
    };

    let tag_oid = current_version.as_ref().map(|(oid, _)| oid.as_str());
    let raw_commits = match git::walk_commits(dir, &head_oid, tag_oid) {
        Ok(c) => c,
        Err(e) => {
            ui::error(&e.to_string());
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
                ui::error(&format!("invalid --release-as version '{forced}': {e}"));
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
                ui::blank();
                ui::heading(
                    "Analysing commits since ",
                    &format!("{}...", cur_ver_str.bold()),
                );
                eprintln!(
                    "{DETAIL}no bump-worthy commits found",
                    DETAIL = ui::DETAIL_INDENT
                );
                ui::blank();
                return 0;
            }
        };

        ui::blank();
        ui::heading(
            "Analysing commits since ",
            &format!("{}...", cur_ver_str.bold()),
        );
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

    ui::blank();
    eprintln!(
        "{INDENT}{} ({bump_reason})",
        format!("{cur_ver} \u{2192} {new_version}").bold(),
        INDENT = ui::INDENT,
    );

    let prev_ver_str = current_version.as_ref().map(|(_, v)| v.to_string());

    let ctx = FinalizeContext {
        new_version: new_version.to_string(),
        prev_version: prev_ver_str.as_deref(),
        raw_commits: &raw_commits,
    };

    finalize_bump(dir, config, opts, &ctx)
}

/// Context passed from the version-computation phase to the shared finalize logic.
struct FinalizeContext<'a> {
    /// The new version string (semver or calver).
    new_version: String,
    /// The previous version string, if any (used for changelog compare links).
    prev_version: Option<&'a str>,
    /// Raw commits since the last tag, used for changelog generation.
    raw_commits: &'a [(String, String)],
}

/// Shared finalize logic for both semver and calver bump paths.
///
/// Handles workdir resolution, custom version files, dry-run output,
/// version file updates, changelog generation, commit creation, and tagging.
fn finalize_bump(
    dir: &std::path::Path,
    config: &ProjectConfig,
    opts: &BumpOptions,
    ctx: &FinalizeContext<'_>,
) -> i32 {
    let tag_prefix = &config.versioning.tag_prefix;
    let new_version = &ctx.new_version;

    let workdir = match git::workdir(dir) {
        Ok(w) => w,
        Err(_) => {
            ui::error("bare repository not supported");
            return 1;
        }
    };
    let workdir = workdir.as_path();

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
        ui::blank();

        match standard_version::detect_version_files(workdir, &custom_files) {
            Ok(detected) if detected.is_empty() => {
                eprintln!("{INDENT}No version files detected", INDENT = ui::INDENT);
            }
            Ok(detected) => {
                eprintln!("{INDENT}Would update:", INDENT = ui::INDENT);
                for f in &detected {
                    let rel = f.path.strip_prefix(workdir).unwrap_or(&f.path).display();
                    ui::item(
                        &rel.to_string(),
                        &format!("{} \u{2192} {new_version}", f.old_version),
                    );
                }
            }
            Err(e) => {
                eprintln!(
                    "{INDENT}warning: cannot detect version files: {e}",
                    INDENT = ui::INDENT,
                );
            }
        }

        if !opts.skip_changelog {
            eprintln!(
                "{INDENT}Would update: CHANGELOG.md         prepend {tag_prefix}{new_version} section",
                INDENT = ui::INDENT,
            );
        }

        if !opts.no_commit {
            eprintln!(
                "{INDENT}Would commit: chore(release): {new_version}",
                INDENT = ui::INDENT,
            );
        }

        if !opts.no_commit && !opts.no_tag {
            eprintln!(
                "{INDENT}Would tag:    {tag_prefix}{new_version}",
                INDENT = ui::INDENT,
            );
        }

        ui::blank();
        return 0;
    }

    // --- Actual execution ---

    // Update all detected version files.
    let version_results: Vec<UpdateResult> =
        match standard_version::update_version_files(workdir, new_version, &custom_files) {
            Ok(r) => r,
            Err(e) => {
                ui::error(&format!("cannot update version files: {e}"));
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
            ui::warning(&format!("failed to update Cargo.lock: {e}"));
        }
    }

    // Generate/update changelog.
    if !opts.skip_changelog {
        let changelog_config = config.to_changelog_config();
        let host = git::detect_host(dir);
        let changelog_path = workdir.join("CHANGELOG.md");

        let release = build_version_release(
            ctx.raw_commits,
            new_version,
            ctx.prev_version,
            &changelog_config,
        );

        if let Some(release) = release {
            let existing = std::fs::read_to_string(&changelog_path).unwrap_or_default();
            let output =
                standard_changelog::prepend_release(&existing, &release, &changelog_config, &host);
            if let Err(e) = std::fs::write(&changelog_path, &output) {
                ui::error(&format!("cannot write CHANGELOG.md: {e}"));
                return 1;
            }
        }
    }

    // Print updated files.
    if !version_results.is_empty() {
        ui::blank();
        eprintln!("{INDENT}Updated:", INDENT = ui::INDENT);
        for r in &version_results {
            let rel = r.path.strip_prefix(workdir).unwrap_or(&r.path).display();
            ui::item(
                &rel.to_string(),
                &format!("{} \u{2192} {}", r.old_version, r.new_version),
            );
            if let Some(ref extra) = r.extra {
                ui::item("", extra);
            }
        }
    }

    if !opts.skip_changelog {
        ui::blank();
        eprintln!("{INDENT}Changelog:", INDENT = ui::INDENT);
        ui::item(
            "CHANGELOG.md",
            &format!("prepended {tag_prefix}{new_version} section"),
        );
    }

    // Create commit.
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

        if let Err(e) = git::stage_files(dir, &paths_to_stage) {
            ui::error(&format!("cannot stage files: {e}"));
            return 1;
        }

        let commit_msg = format!("chore(release): {new_version}");

        if opts.sign {
            if let Err(e) = git::create_signed_commit(dir, &commit_msg) {
                ui::error(&e.to_string());
                return 1;
            }
        } else if let Err(e) = git::create_commit(dir, &commit_msg) {
            ui::error(&format!("cannot create commit: {e}"));
            return 1;
        }

        ui::blank();
        eprintln!(
            "{INDENT}Committed: {}",
            commit_msg.green(),
            INDENT = ui::INDENT,
        );
    }

    // Create annotated tag.
    if !opts.no_commit && !opts.no_tag {
        let tag_name = format!("{tag_prefix}{new_version}");
        let tag_msg = new_version.to_string();

        if opts.sign {
            if let Err(e) = git::create_signed_tag(dir, &tag_name, &tag_msg) {
                ui::error(&e.to_string());
                return 1;
            }
        } else if let Err(e) = git::create_annotated_tag(dir, &tag_name, &tag_msg) {
            ui::error(&format!("cannot create tag: {e}"));
            return 1;
        }

        eprintln!(
            "{INDENT}Tagged:    {}",
            tag_name.green(),
            INDENT = ui::INDENT,
        );
    }

    ui::blank();
    eprintln!(
        "{INDENT}Push with: git push --follow-tags",
        INDENT = ui::INDENT,
    );
    ui::blank();

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
        eprintln!("{DETAIL}{}", parts.join(", "), DETAIL = ui::DETAIL_INDENT);
    }
}

/// Build a `VersionRelease` from raw commits for changelog generation.
fn build_version_release(
    commits: &[(String, String)],
    version: &str,
    prev_tag: Option<&str>,
    config: &standard_changelog::ChangelogConfig,
) -> Option<VersionRelease> {
    let pairs: Vec<(String, &str)> = commits
        .iter()
        .map(|(oid, msg)| (oid[..7].to_string(), msg.as_str()))
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
    let secs = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => {
            ui::warning(&format!(
                "system clock failure ({e}), falling back to Unix epoch"
            ));
            0
        }
    };
    calver_date_from_epoch_days(secs.div_euclid(86400) as i32)
}

/// Compute a [`CalverDate`] from days since the Unix epoch.
fn calver_date_from_epoch_days(days: i32) -> standard_version::calver::CalverDate {
    // Howard Hinnant's civil_from_days algorithm.
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i32 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    let dow = ((days + 3).rem_euclid(7) + 1) as u32;
    let jan1_days = {
        let ys = y - 1;
        let eras = ys.div_euclid(400);
        let yoes = ys.rem_euclid(400) as u32;
        let ms: u32 = 10;
        let ds: u32 = 1;
        let doys = (153 * ms + 2) / 5 + ds - 1;
        let does = yoes * 365 + yoes / 4 - yoes / 100 + doys;
        eras * 146097 + does as i32 - 719468
    };
    let ordinal = days - jan1_days + 1;
    let jan1_dow = (jan1_days + 3).rem_euclid(7) + 1;

    let iso_week = {
        let w = (ordinal - dow as i32 + 10) / 7;
        if w < 1 {
            let prev_jan1_dow = (jan1_days - 1 + 3).rem_euclid(7) + 1;
            if prev_jan1_dow == 4
                || (prev_jan1_dow == 3 && {
                    let py = y - 1;
                    py % 4 == 0 && (py % 100 != 0 || py % 400 == 0)
                })
            {
                53
            } else {
                52
            }
        } else if w > 52 {
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

/// Run the bump subcommand in stable-branch mode.
fn run_stable(config: &ProjectConfig, opts: &BumpOptions) -> i32 {
    if config.scheme == Scheme::Calver {
        ui::error("--stable is not supported with scheme = \"calver\"");
        return 1;
    }

    let dir = std::path::Path::new(".");

    let tag_prefix = &config.versioning.tag_prefix;

    let current_version = match git::find_latest_version_tag(dir, tag_prefix) {
        Ok(Some((oid, ver))) => Some((oid, ver)),
        Ok(None) => None,
        Err(e) => {
            ui::error(&e.to_string());
            return 1;
        }
    };

    let cur_ver = current_version
        .as_ref()
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| semver::Version::new(0, 0, 0));

    match git::is_working_tree_dirty(dir) {
        Ok(true) => {
            ui::error("working tree has uncommitted changes");
            return 1;
        }
        Err(e) => {
            ui::error(&format!("cannot check working tree status: {e}"));
            return 1;
        }
        Ok(false) => {}
    }

    let stable_branch_name = match &opts.stable {
        Some(Some(name)) => name.clone(),
        _ => format!("stable-v{}.{}", cur_ver.major, cur_ver.minor),
    };

    match git::branch_exists(dir, &stable_branch_name) {
        Ok(true) => {
            ui::error(&format!("branch '{stable_branch_name}' already exists"));
            return 1;
        }
        Ok(false) => {}
        Err(e) => {
            ui::error(&format!("cannot check branch: {e}"));
            return 1;
        }
    }

    let original_branch = match git::current_branch(dir) {
        Ok(name) => name,
        Err(e) => {
            ui::error(&format!("cannot resolve HEAD: {e}"));
            return 1;
        }
    };

    let new_version = if opts.minor {
        semver::Version::new(cur_ver.major, cur_ver.minor + 1, 0)
    } else {
        semver::Version::new(cur_ver.major + 1, 0, 0)
    };

    let bump_kind = if opts.minor { "minor" } else { "major" };

    if opts.dry_run {
        ui::blank();
        eprintln!("{INDENT}Creating stable branch...", INDENT = ui::INDENT);
        ui::item("Branch:", &stable_branch_name);
        ui::item("Scheme:", "patch (patch-only bumps)");
        ui::blank();
        eprintln!(
            "{INDENT}Would commit: chore(release): stabilize v{}.{}",
            cur_ver.major,
            cur_ver.minor,
            INDENT = ui::INDENT,
        );
        ui::blank();
        eprintln!(
            "{INDENT}Advancing {original_branch}...",
            INDENT = ui::INDENT
        );
        eprintln!(
            "{DETAIL}{} ({bump_kind})",
            format!("{cur_ver} \u{2192} {new_version}").bold(),
            DETAIL = ui::DETAIL_INDENT,
        );
        ui::blank();
        eprintln!(
            "{INDENT}Would commit: chore(release): {new_version}",
            INDENT = ui::INDENT,
        );
        eprintln!(
            "{INDENT}Would tag:    {tag_prefix}{new_version}",
            INDENT = ui::INDENT,
        );
        ui::blank();
        eprintln!("{INDENT}Push with:", INDENT = ui::INDENT);
        ui::item("", &format!("git push origin {stable_branch_name}"));
        ui::item("", "git push --follow-tags");
        ui::blank();
        return 0;
    }

    // --- Actual execution ---

    if let Err(e) = git::create_branch(dir, &stable_branch_name) {
        ui::error(&format!("cannot create branch: {e}"));
        return 1;
    }

    if let Err(e) = git::checkout_branch(dir, &stable_branch_name) {
        ui::error(&format!("cannot checkout branch: {e}"));
        return 1;
    }

    let workdir = match git::workdir(dir) {
        Ok(w) => w,
        Err(_) => {
            ui::error("bare repository not supported");
            return 1;
        }
    };

    let config_path = workdir.join(".git-std.toml");
    let config_content = if config_path.exists() {
        let existing = std::fs::read_to_string(&config_path).unwrap_or_default();
        update_scheme_in_config(&existing, "patch")
    } else {
        "[versioning]\nscheme = \"patch\"\n".to_string()
    };

    if let Err(e) = std::fs::write(&config_path, &config_content) {
        ui::error(&format!("cannot write .git-std.toml: {e}"));
        return 1;
    }

    if let Err(e) = git::stage_files(dir, &[".git-std.toml"]) {
        ui::error(&format!("cannot stage files: {e}"));
        return 1;
    }

    let stabilize_msg = format!(
        "chore(release): stabilize v{}.{}",
        cur_ver.major, cur_ver.minor
    );

    if let Err(e) = git::create_commit(dir, &stabilize_msg) {
        ui::error(&format!("cannot create commit: {e}"));
        return 1;
    }

    ui::blank();
    eprintln!("{INDENT}Creating stable branch...", INDENT = ui::INDENT);
    ui::item("Branch:", &stable_branch_name);
    ui::item("Scheme:", "patch (patch-only bumps)");
    ui::blank();
    eprintln!(
        "{INDENT}Committed: {}",
        stabilize_msg.green(),
        INDENT = ui::INDENT,
    );

    if let Err(e) = git::checkout_branch(dir, &original_branch) {
        ui::error(&format!("cannot checkout branch '{original_branch}': {e}"));
        return 1;
    }

    ui::blank();
    eprintln!(
        "{INDENT}Advancing {original_branch}...",
        INDENT = ui::INDENT
    );
    eprintln!(
        "{DETAIL}{} ({bump_kind})",
        format!("{cur_ver} \u{2192} {new_version}").bold(),
        DETAIL = ui::DETAIL_INDENT,
    );

    let head_oid = match git::head_oid(dir) {
        Ok(oid) => oid,
        Err(e) => {
            ui::error(&format!("cannot resolve HEAD: {e}"));
            return 1;
        }
    };

    let tag_oid = current_version.as_ref().map(|(oid, _)| oid.as_str());
    let raw_commits = match git::walk_commits(dir, &head_oid, tag_oid) {
        Ok(c) => c,
        Err(e) => {
            ui::error(&e.to_string());
            return 1;
        }
    };

    let prev_ver_str = current_version.as_ref().map(|(_, v)| v.to_string());

    let ctx = FinalizeContext {
        new_version: new_version.to_string(),
        prev_version: prev_ver_str.as_deref(),
        raw_commits: &raw_commits,
    };

    let exit = finalize_bump(dir, config, opts, &ctx);
    if exit != 0 {
        return exit;
    }

    eprintln!(
        "{INDENT}Push stable: git push origin {stable_branch_name}",
        INDENT = ui::INDENT,
    );
    ui::blank();

    0
}

/// Update or add `scheme = "patch"` in a `.git-std.toml` config string.
fn update_scheme_in_config(existing: &str, scheme: &str) -> String {
    let mut result = String::new();
    let mut found_scheme = false;
    let mut in_versioning = false;
    let mut has_versioning = false;

    for line in existing.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') {
            if trimmed == "[versioning]" {
                has_versioning = true;
                in_versioning = true;
            } else {
                in_versioning = false;
            }
        }

        if trimmed.starts_with("scheme") && trimmed.contains('=') && !in_versioning {
            result.push_str(&format!("scheme = \"{scheme}\"\n"));
            found_scheme = true;
            continue;
        }

        if in_versioning && trimmed.starts_with("scheme") && trimmed.contains('=') {
            result.push_str(&format!("scheme = \"{scheme}\"\n"));
            found_scheme = true;
            continue;
        }

        result.push_str(line);
        result.push('\n');
    }

    if !found_scheme {
        if has_versioning {
            let mut new_result = String::new();
            for line in result.lines() {
                new_result.push_str(line);
                new_result.push('\n');
                if line.trim() == "[versioning]" {
                    new_result.push_str(&format!("scheme = \"{scheme}\"\n"));
                }
            }
            return new_result;
        }
        result.insert_str(0, &format!("scheme = \"{scheme}\"\n"));
    }

    result
}

/// Run the bump subcommand in patch-only mode.
fn run_patch(config: &ProjectConfig, opts: &BumpOptions) -> i32 {
    let dir = std::path::Path::new(".");

    let tag_prefix = &config.versioning.tag_prefix;

    let current_version = match git::find_latest_version_tag(dir, tag_prefix) {
        Ok(Some((oid, ver))) => Some((oid, ver)),
        Ok(None) => None,
        Err(e) => {
            ui::error(&e.to_string());
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

    let tag_oid = current_version.as_ref().map(|(oid, _)| oid.as_str());
    let raw_commits = match git::walk_commits(dir, &head_oid, tag_oid) {
        Ok(c) => c,
        Err(e) => {
            ui::error(&e.to_string());
            return 1;
        }
    };

    let has_breaking = raw_commits
        .iter()
        .filter_map(|(_, msg)| standard_commit::parse(msg).ok())
        .any(|c| c.is_breaking);

    if has_breaking && !opts.force {
        ui::error("breaking change not allowed on patch-only branch (use --force to override)");
        return 1;
    }

    let cur_ver = current_version
        .as_ref()
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| semver::Version::new(0, 0, 0));

    let new_version = semver::Version::new(cur_ver.major, cur_ver.minor, cur_ver.patch + 1);

    ui::blank();
    eprintln!(
        "{INDENT}{} (patch)",
        format!("{cur_ver} \u{2192} {new_version}").bold(),
        INDENT = ui::INDENT,
    );

    let prev_ver_str = current_version.as_ref().map(|(_, v)| v.to_string());

    let ctx = FinalizeContext {
        new_version: new_version.to_string(),
        prev_version: prev_ver_str.as_deref(),
        raw_commits: &raw_commits,
    };

    finalize_bump(dir, config, opts, &ctx)
}

/// Run the bump subcommand in calver mode.
fn run_calver(config: &ProjectConfig, opts: &BumpOptions) -> i32 {
    let dir = std::path::Path::new(".");

    let tag_prefix = &config.versioning.tag_prefix;
    let calver_format = &config.versioning.calver_format;

    if opts.prerelease.is_some() {
        ui::error("--prerelease is not supported with scheme = \"calver\"");
        return 1;
    }

    let current_tag = match git::find_latest_calver_tag(dir, tag_prefix) {
        Ok(v) => v,
        Err(e) => {
            ui::error(&e.to_string());
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

    let tag_oid = current_tag.as_ref().map(|(oid, _)| oid.as_str());
    let raw_commits = match git::walk_commits(dir, &head_oid, tag_oid) {
        Ok(c) => c,
        Err(e) => {
            ui::error(&e.to_string());
            return 1;
        }
    };

    let prev_ver = current_tag.as_ref().map(|(_, v)| v.as_str());

    let date = today_calver_date();
    let new_version = if let Some(ref forced) = opts.release_as {
        forced.clone()
    } else if opts.first_release {
        match standard_version::calver::next_version(calver_format, date, None) {
            Ok(v) => v,
            Err(e) => {
                ui::error(&e.to_string());
                return 1;
            }
        }
    } else {
        match standard_version::calver::next_version(calver_format, date, prev_ver) {
            Ok(v) => v,
            Err(e) => {
                ui::error(&e.to_string());
                return 1;
            }
        }
    };

    ui::blank();
    eprintln!(
        "{INDENT}{} (calver)",
        format!("{} \u{2192} {new_version}", prev_ver.unwrap_or("none")).bold(),
        INDENT = ui::INDENT,
    );

    let ctx = FinalizeContext {
        new_version: new_version.clone(),
        prev_version: prev_ver,
        raw_commits: &raw_commits,
    };

    finalize_bump(dir, config, opts, &ctx)
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
        let days = 20528;
        let d = calver_date_from_epoch_days(days);
        assert_eq!(d.year, 2026);
        assert_eq!(d.month, 3);
        assert_eq!(d.day, 16);
        assert_eq!(d.day_of_week, 1);
        assert_eq!(d.iso_week, 12);
    }

    #[test]
    fn calver_date_dec31_to_jan1_boundary() {
        let dec31 = 20818;
        let d = calver_date_from_epoch_days(dec31);
        assert_eq!(d.year, 2026);
        assert_eq!(d.month, 12);
        assert_eq!(d.day, 31);
        assert_eq!(d.day_of_week, 4);

        let jan1 = 20819;
        let d = calver_date_from_epoch_days(jan1);
        assert_eq!(d.year, 2027);
        assert_eq!(d.month, 1);
        assert_eq!(d.day, 1);
        assert_eq!(d.day_of_week, 5);
    }

    #[test]
    fn calver_date_jan1_2024_monday() {
        let days = 19723;
        let d = calver_date_from_epoch_days(days);
        assert_eq!(d.year, 2024);
        assert_eq!(d.month, 1);
        assert_eq!(d.day, 1);
        assert_eq!(d.day_of_week, 1);
        assert_eq!(d.iso_week, 1);
    }

    #[test]
    fn chrono_date_format() {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let date = standard_changelog::format_date(secs);
        assert_eq!(date.len(), 10);
        assert_eq!(&date[4..5], "-");
        assert_eq!(&date[7..8], "-");
    }
}
