use yansi::Paint;

use crate::config::{ProjectConfig, Scheme};
use crate::git;
use crate::ui;

use super::apply::finalize_bump;
use super::detect::today_calver_date;
use super::lifecycle::run_lifecycle_hook;
use super::{BumpOptions, FinalizeContext};

/// Parse a `--release-as` value as a semver bump level.
///
/// Returns `Some(level)` for `"patch"`, `"minor"`, `"major"` (case-insensitive).
/// Returns `None` for anything else (treated as an exact version string).
pub(super) fn parse_release_level(s: &str) -> Option<standard_version::BumpLevel> {
    match s.to_ascii_lowercase().as_str() {
        "patch" => Some(standard_version::BumpLevel::Patch),
        "minor" => Some(standard_version::BumpLevel::Minor),
        "major" => Some(standard_version::BumpLevel::Major),
        _ => None,
    }
}

/// Run the bump subcommand in patch-only mode.
pub(super) fn run_patch(config: &ProjectConfig, opts: &BumpOptions) -> i32 {
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

    if let Some(ref forced) = opts.release_as
        && parse_release_level(forced).is_some_and(|l| l != standard_version::BumpLevel::Patch)
    {
        ui::error("patch-only scheme does not support --release-as minor or --release-as major");
        return 1;
    }

    let cur_ver = current_version
        .as_ref()
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| semver::Version::new(0, 0, 0));

    let new_version = semver::Version::new(cur_ver.major, cur_ver.minor, cur_ver.patch + 1);

    ui::blank();
    ui::info(&format!(
        "{} (patch)",
        format!("{cur_ver} \u{2192} {new_version}").bold(),
    ));

    let prev_ver_str = current_version.as_ref().map(|(_, v)| v.to_string());

    let new_version_str = new_version.to_string();

    if !opts.dry_run
        && let Err(code) = run_lifecycle_hook("post-version", &[&new_version_str])
    {
        return code;
    }

    let ctx = FinalizeContext {
        new_version: new_version_str,
        prev_version: prev_ver_str.as_deref(),
        raw_commits: &raw_commits,
    };

    finalize_bump(dir, config, opts, &ctx)
}

/// Run the bump subcommand in calver mode.
pub(super) fn run_calver(config: &ProjectConfig, opts: &BumpOptions) -> i32 {
    let dir = std::path::Path::new(".");

    let tag_prefix = &config.versioning.tag_prefix;
    let calver_format = &config.versioning.calver_format;

    if opts.prerelease.is_some() {
        ui::error("--prerelease is not supported with scheme = \"calver\"");
        return 1;
    }

    if let Some(ref forced) = opts.release_as
        && parse_release_level(forced).is_some()
    {
        ui::error("--release-as patch/minor/major is not supported with scheme = \"calver\"");
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
    ui::info(&format!(
        "{} (calver)",
        format!("{} \u{2192} {new_version}", prev_ver.unwrap_or("none")).bold(),
    ));

    if !opts.dry_run
        && let Err(code) = run_lifecycle_hook("post-version", &[&new_version])
    {
        return code;
    }

    let ctx = FinalizeContext {
        new_version: new_version.clone(),
        prev_version: prev_ver,
        raw_commits: &raw_commits,
    };

    finalize_bump(dir, config, opts, &ctx)
}

/// Run the standard semver bump path.
pub(super) fn run_semver(config: &ProjectConfig, opts: &BumpOptions) -> i32 {
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
        if let Some(level) = parse_release_level(forced) {
            standard_version::apply_bump(&cur_ver, level)
        } else {
            match semver::Version::parse(forced) {
                Ok(v) => v,
                Err(e) => {
                    ui::error(&format!("invalid --release-as version '{forced}': {e}"));
                    return 1;
                }
            }
        }
    } else if opts.first_release {
        cur_ver.clone()
    } else {
        let summary = standard_version::summarise(&parsed);
        let bump_level = match standard_version::determine_bump(&parsed) {
            Some(level) => level,
            None => {
                if !opts.force {
                    ui::blank();
                    ui::heading(
                        "Analysing commits since ",
                        &format!("{}...", cur_ver_str.bold()),
                    );
                    ui::detail("no bump-worthy commits found");
                    ui::blank();
                    return 0;
                }
                // --force: allow bump even with no bump-worthy commits (defaults to patch)
                standard_version::BumpLevel::Patch
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
        if let Some(level) = parse_release_level(forced) {
            let level_name = match level {
                standard_version::BumpLevel::Patch => "patch",
                standard_version::BumpLevel::Minor => "minor",
                standard_version::BumpLevel::Major => "major",
            };
            format!("forced {level_name}")
        } else {
            format!("forced as {forced}")
        }
    } else if let Some(level) = standard_version::determine_bump(&parsed) {
        let is_pre1 = cur_ver.major == 0;
        let reason = match (level, is_pre1) {
            (standard_version::BumpLevel::Major, true) => {
                "minor \u{2014} breaking change (pre-1.0)"
            }
            (standard_version::BumpLevel::Minor, true) => "patch \u{2014} new feature (pre-1.0)",
            (standard_version::BumpLevel::Patch, true) => "patch \u{2014} bug fix",
            (standard_version::BumpLevel::Major, false) => {
                "major \u{2014} breaking change detected"
            }
            (standard_version::BumpLevel::Minor, false) => "minor \u{2014} new feature",
            (standard_version::BumpLevel::Patch, false) => "patch \u{2014} bug fix",
        };
        reason.to_string()
    } else {
        // No commits found, but --force was used
        "forced patch (no commits)".to_string()
    };

    ui::blank();
    ui::info(&format!(
        "{} ({bump_reason})",
        format!("{cur_ver} \u{2192} {new_version}").bold(),
    ));

    let prev_ver_str = current_version.as_ref().map(|(_, v)| v.to_string());
    let new_version_str = new_version.to_string();

    if !opts.dry_run
        && let Err(code) = run_lifecycle_hook("post-version", &[&new_version_str])
    {
        return code;
    }

    let ctx = FinalizeContext {
        new_version: new_version_str,
        prev_version: prev_ver_str.as_deref(),
        raw_commits: &raw_commits,
    };

    finalize_bump(dir, config, opts, &ctx)
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
        ui::detail(&parts.join(", "));
    }
}

/// Dispatch to the correct bump mode based on config scheme.
pub(super) fn dispatch(config: &ProjectConfig, opts: &BumpOptions) -> i32 {
    if config.scheme == Scheme::Calver {
        return run_calver(config, opts);
    }
    if config.scheme == Scheme::Patch {
        return run_patch(config, opts);
    }
    run_semver(config, opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_release_level_recognises_all_levels() {
        assert_eq!(
            parse_release_level("patch"),
            Some(standard_version::BumpLevel::Patch)
        );
        assert_eq!(
            parse_release_level("minor"),
            Some(standard_version::BumpLevel::Minor)
        );
        assert_eq!(
            parse_release_level("major"),
            Some(standard_version::BumpLevel::Major)
        );
        // Case-insensitive.
        assert_eq!(
            parse_release_level("PATCH"),
            Some(standard_version::BumpLevel::Patch)
        );
        assert_eq!(
            parse_release_level("Minor"),
            Some(standard_version::BumpLevel::Minor)
        );
        assert_eq!(
            parse_release_level("MAJOR"),
            Some(standard_version::BumpLevel::Major)
        );
    }

    #[test]
    fn parse_release_level_returns_none_for_version_string() {
        assert_eq!(parse_release_level("1.2.3"), None);
        assert_eq!(parse_release_level("2.0.0"), None);
        assert_eq!(parse_release_level("0.1.0-rc.1"), None);
        assert_eq!(parse_release_level(""), None);
        assert_eq!(parse_release_level("minorr"), None);
    }
}
