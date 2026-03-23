use std::path::PathBuf;

use standard_changelog::VersionRelease;
use yansi::Paint;

use crate::config::ProjectConfig;
use crate::git;
use crate::ui;

use super::lock_sync;
use super::{BumpOptions, FinalizeContext};

/// Build a `VersionRelease` from raw commits for changelog generation.
pub(super) fn build_version_release(
    commits: &[(String, String)],
    version: &str,
    prev_tag: Option<&str>,
    config: &standard_changelog::ChangelogConfig,
) -> Option<VersionRelease> {
    let mut release =
        super::super::changelog::build_release_from_commits(commits, version, prev_tag, config)?;

    // Use today's date.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    release.date = standard_changelog::format_date(secs);

    Some(release)
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

    let custom_files: Vec<standard_version::CustomVersionFile> = config
        .version_files
        .iter()
        .map(|vf| standard_version::CustomVersionFile {
            path: PathBuf::from(&vf.path),
            pattern: vf.regex.clone(),
        })
        .collect();

    // --- Dry run: print plan and exit ---
    if opts.dry_run {
        ui::blank();

        let dry_cargo_updated = match standard_version::detect_version_files(workdir, &custom_files)
        {
            Ok(detected) if detected.is_empty() => {
                ui::info("No version files detected");
                false
            }
            Ok(detected) => {
                ui::info("Would update:");
                for f in &detected {
                    let rel = f.path.strip_prefix(workdir).unwrap_or(&f.path).display();
                    ui::item(
                        &rel.to_string(),
                        &format!("{} \u{2192} {new_version}", f.old_version),
                    );
                }
                detected.iter().any(|f| f.name.ends_with("Cargo.toml"))
            }
            Err(e) => {
                ui::warning(&format!("cannot detect version files: {e}"));
                false
            }
        };

        lock_sync::dry_run_lock_files(workdir, dry_cargo_updated);

        if !opts.skip_changelog {
            ui::info(&format!(
                "Would update: CHANGELOG.md         prepend {tag_prefix}{new_version} section"
            ));
        }

        if !opts.no_commit {
            ui::info(&format!("Would commit: chore(release): {new_version}"));
        }

        if !opts.no_commit && !opts.no_tag {
            ui::info(&format!("Would tag:    {tag_prefix}{new_version}"));
        }

        ui::blank();
        return 0;
    }

    // --- Actual execution ---

    // Update all detected version files.
    let version_results: Vec<standard_version::UpdateResult> =
        match standard_version::update_version_files(workdir, new_version, &custom_files) {
            Ok(r) => r,
            Err(e) => {
                ui::error(&format!("cannot update version files: {e}"));
                return 1;
            }
        };

    // Sync ecosystem lock files.
    let cargo_updated = version_results
        .iter()
        .any(|r| r.name.ends_with("Cargo.toml"));
    let synced_locks = lock_sync::sync_lock_files(workdir, cargo_updated);

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
        ui::info("Updated:");
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
        ui::info("Changelog:");
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
        // Stage all successfully synced lock files.
        for lock in &synced_locks {
            paths_to_stage.push(lock.as_str());
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
        ui::info(&format!("Committed: {}", commit_msg.green()));
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

        ui::info(&format!("Tagged:    {}", tag_name.green()));
    }

    ui::blank();
    ui::info("Push with: git push --follow-tags");
    ui::blank();

    0
}
