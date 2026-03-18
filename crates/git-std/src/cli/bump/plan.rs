use yansi::Paint;

use crate::config::{ProjectConfig, Scheme};
use crate::git;
use crate::ui;

use super::apply::finalize_bump;
use super::detect::today_calver_date;
use super::{BumpOptions, FinalizeContext};

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

    let ctx = FinalizeContext {
        new_version: new_version.to_string(),
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
                ui::detail("no bump-worthy commits found");
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
    ui::info(&format!(
        "{} ({bump_reason})",
        format!("{cur_ver} \u{2192} {new_version}").bold(),
    ));

    let prev_ver_str = current_version.as_ref().map(|(_, v)| v.to_string());

    let ctx = FinalizeContext {
        new_version: new_version.to_string(),
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
