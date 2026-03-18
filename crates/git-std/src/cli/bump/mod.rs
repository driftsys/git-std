use std::path::PathBuf;

use standard_changelog::VersionRelease;
use standard_version::{CustomVersionFile, UpdateResult};
use yansi::Paint;

use crate::config::{ProjectConfig, Scheme};
use crate::git;
use crate::ui;

mod apply;
mod detect;
mod plan;

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

/// Context passed from the version-computation phase to the shared finalize logic.
pub(super) struct FinalizeContext<'a> {
    /// The new version string (semver or calver).
    pub(super) new_version: String,
    /// The previous version string, if any (used for changelog compare links).
    pub(super) prev_version: Option<&'a str>,
    /// Raw commits since the last tag, used for changelog generation.
    pub(super) raw_commits: &'a [(String, String)],
}

/// Run the bump subcommand. Returns the exit code.
pub fn run(config: &ProjectConfig, opts: &BumpOptions) -> i32 {
    if opts.stable.is_some() {
        return apply::run_stable(config, opts);
    }
    if config.scheme == Scheme::Calver {
        return plan::run_calver(config, opts);
    }
    if config.scheme == Scheme::Patch {
        return plan::run_patch(config, opts);
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

/// Shared finalize logic for both semver and calver bump paths.
///
/// Handles workdir resolution, custom version files, dry-run output,
/// version file updates, changelog generation, commit creation, and tagging.
pub(super) fn finalize_bump(
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
    let mut release =
        super::changelog::build_release_from_commits(commits, version, prev_tag, config)?;

    // Use today's date.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    release.date = standard_changelog::format_date(secs);

    Some(release)
}
