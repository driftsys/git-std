use yansi::Paint;

use crate::config::ProjectConfig;
use crate::git;
use crate::ui;

use super::{BumpOptions, FinalizeContext, detect, finalize_bump};

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

    let date = detect::today_calver_date();
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
