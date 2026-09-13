use standard_changelog::VersionRelease;
use yansi::Paint;

use crate::app::OutputFormat;
use crate::config::ProjectConfig;
use crate::contract::ContractMetadata;
use crate::git;
use crate::ui;

use super::error::{lifecycle_failure, machine_or_human_error, plan_diverged};
use super::lifecycle::run_lifecycle_hook;
use super::result::{BumpResultJson, UpdatedFileJson};
use super::version_facts::collect_version_facts;
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
            return machine_or_human_error(
                opts,
                "GITSTD-GIT-OPERATION",
                "bare repository not supported",
                2,
            );
        }
    };
    let workdir = workdir.as_path();

    let custom_files: Vec<standard_version::CustomVersionFile> =
        crate::config::resolve_custom_version_files(workdir, &config.version_files);
    let detected = crate::ecosystem::dry_run_version_files(workdir, &custom_files);
    let lock_files = crate::ecosystem::dry_run_lock_file_names(workdir);
    let changelog_after = planned_changelog(dir, workdir, config, opts, ctx);
    let has_changelog = changelog_after.is_some();
    let canonical = ctx.prev_version.unwrap_or(new_version);
    let facts = collect_version_facts(workdir, canonical, &detected);
    let mut contract = match super::contract::build_contract(
        workdir,
        config,
        opts,
        new_version,
        &detected,
        &lock_files,
        changelog_after.as_deref().map(str::as_bytes),
    ) {
        Ok(contract) => contract,
        Err(error) => return machine_or_human_error(opts, "GITSTD-BUMP-PLAN", error, 2),
    };

    if let Some(expected) = &opts.expect_plan
        && expected != &contract.plan_id
    {
        return plan_diverged(opts, expected, &contract.plan_id);
    }

    // --- Dry run: print plan and exit ---
    if opts.dry_run {
        if opts.format == OutputFormat::Json {
            let result = BumpResultJson {
                metadata: ContractMetadata::current(),
                status: "planned",
                contract,
                version: new_version.clone(),
                previous_version: ctx.prev_version.map(String::from),
                tag: if !opts.no_commit && !opts.no_tag {
                    Some(format!("{tag_prefix}{new_version}"))
                } else {
                    None
                },
                updated_files: detected
                    .iter()
                    .map(|f| UpdatedFileJson {
                        path: f
                            .path
                            .strip_prefix(workdir)
                            .unwrap_or(&f.path)
                            .display()
                            .to_string(),
                        old_version: f.old_version.clone(),
                        new_version: new_version.clone(),
                    })
                    .collect(),
                synced_locks: lock_files.clone(),
                changelog: has_changelog,
                commit: if !opts.no_commit {
                    Some(format!("chore(release): {new_version}"))
                } else {
                    None
                },
                pushed_to: if !opts.no_commit && !opts.no_tag {
                    opts.push.clone()
                } else {
                    None
                },
                version_observations: facts.observations,
                version_mismatches: facts.mismatches,
                commit_oid: None,
                tag_oid: None,
                dry_run: true,
            };
            println!("{}", serde_json::to_string(&result).unwrap());
            return 0;
        }

        ui::blank();

        if detected.is_empty() {
            ui::info("No version files detected");
        } else {
            ui::info("Would update:");
            for f in &detected {
                let rel = f.path.strip_prefix(workdir).unwrap_or(&f.path).display();
                ui::item(
                    &rel.to_string(),
                    &format!("{} \u{2192} {new_version}", f.old_version),
                );
            }
        }
        crate::ecosystem::dry_run_lock_sync(workdir);

        if has_changelog {
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

        if let Some(remote) = &opts.push {
            if !opts.no_commit && !opts.no_tag {
                ui::info(&format!("Would push to {remote}"));
            } else {
                ui::warning(&format!(
                    "Would skip push to {remote}: incompatible with --no-commit or --no-tag"
                ));
            }
        }

        ui::blank();
        return 0;
    }

    // --- Actual execution ---

    if let Err(code) = run_lifecycle_hook("pre-bump", &[], opts.format) {
        return lifecycle_failure(opts, "pre-bump", code);
    }

    // A pre-bump hook is opaque and may have changed a declared input. Recheck
    // a guarded apply before git-std performs its own writes.
    if opts.expect_plan.is_some() {
        let current_detected = crate::ecosystem::dry_run_version_files(workdir, &custom_files);
        let current_locks = crate::ecosystem::dry_run_lock_file_names(workdir);
        let current = match super::contract::build_contract(
            workdir,
            config,
            opts,
            new_version,
            &current_detected,
            &current_locks,
            changelog_after.as_deref().map(str::as_bytes),
        ) {
            Ok(contract) => contract,
            Err(error) => return machine_or_human_error(opts, "GITSTD-BUMP-PLAN", error, 2),
        };
        let expected = opts.expect_plan.as_deref().expect("checked above");
        if current.plan_id != expected {
            return plan_diverged(opts, expected, &current.plan_id);
        }
        contract = current;
    }

    // Update all detected version files and sync ecosystem lock files.
    let bump_result = crate::ecosystem::run_bump(workdir, new_version, &custom_files);
    let version_results = bump_result.update_results;
    let extra_modified = bump_result.modified_paths;
    let synced_locks = bump_result.synced_locks;

    if let Err(code) = run_lifecycle_hook("post-version", &[new_version], opts.format) {
        return lifecycle_failure(opts, "post-version", code);
    }

    // Generate/update changelog.
    if let Some(output) = &changelog_after {
        let changelog_path = workdir.join("CHANGELOG.md");
        if let Err(e) = std::fs::write(&changelog_path, output) {
            return machine_or_human_error(
                opts,
                "GITSTD-IO-WRITE",
                format!("cannot write CHANGELOG.md: {e}"),
                2,
            );
        }
    }

    // Print updated files.
    if !version_results.is_empty() && opts.format != OutputFormat::Json {
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

    if has_changelog && opts.format != OutputFormat::Json {
        ui::blank();
        ui::info("Changelog:");
        ui::item(
            "CHANGELOG.md",
            &format!("prepended {tag_prefix}{new_version} section"),
        );
    }

    // post-changelog hook: runs after CHANGELOG.md is written, before staging/commit.
    if has_changelog && let Err(code) = run_lifecycle_hook("post-changelog", &[], opts.format) {
        return lifecycle_failure(opts, "post-changelog", code);
    }

    // Create commit.
    if !opts.no_commit {
        let mut rel_paths: Vec<String> = version_results
            .iter()
            .filter_map(|r| {
                r.path
                    .strip_prefix(workdir)
                    .ok()
                    .map(|p| p.to_string_lossy().into_owned())
            })
            .collect();
        // Include files modified by ecosystem CLI tools.
        for p in &extra_modified {
            if let Ok(rel) = p.strip_prefix(workdir) {
                rel_paths.push(rel.to_string_lossy().into_owned());
            }
        }
        let mut paths_to_stage: Vec<&str> = rel_paths.iter().map(|s| s.as_str()).collect();
        if has_changelog {
            paths_to_stage.push("CHANGELOG.md");
        }
        // Stage all successfully synced lock files.
        for lock in &synced_locks {
            paths_to_stage.push(lock.as_str());
        }

        if let Err(e) = git::stage_files(workdir, &paths_to_stage) {
            return machine_or_human_error(
                opts,
                "GITSTD-GIT-OPERATION",
                format!("cannot stage files: {e}"),
                2,
            );
        }

        let commit_msg = format!("chore(release): {new_version}");

        if opts.sign {
            if let Err(e) = git::create_signed_commit_only(workdir, &commit_msg, &paths_to_stage) {
                return machine_or_human_error(opts, "GITSTD-GIT-OPERATION", e.to_string(), 2);
            }
        } else if let Err(e) = git::create_commit_only(workdir, &commit_msg, &paths_to_stage) {
            return machine_or_human_error(
                opts,
                "GITSTD-GIT-OPERATION",
                format!("cannot create commit: {e}"),
                2,
            );
        }

        ui::blank();
        if opts.format != OutputFormat::Json {
            ui::info(&format!("Committed: {}", commit_msg.green()));
        }
    }

    // Create annotated tag.
    if !opts.no_commit && !opts.no_tag {
        let tag_name = format!("{tag_prefix}{new_version}");
        let tag_msg = new_version.to_string();

        if opts.sign {
            if let Err(e) = git::create_signed_tag(dir, &tag_name, &tag_msg) {
                return machine_or_human_error(opts, "GITSTD-GIT-OPERATION", e.to_string(), 2);
            }
        } else if let Err(e) = git::create_annotated_tag(dir, &tag_name, &tag_msg) {
            return machine_or_human_error(
                opts,
                "GITSTD-GIT-OPERATION",
                format!("cannot create tag: {e}"),
                2,
            );
        }

        if opts.format != OutputFormat::Json {
            ui::info(&format!("Tagged:    {}", tag_name.green()));
        }
    }

    // Push commit and tags to remote.
    // Skipped (with a warning) when --no-commit or --no-tag is set, because
    // --follow-tags degenerates to a plain branch push when there is no tag.
    if let Some(remote) = &opts.push {
        if opts.no_commit || opts.no_tag {
            ui::warning("--push skipped: incompatible with --no-commit or --no-tag");
        } else if let Err(e) = git::push_follow_tags(dir, remote) {
            return machine_or_human_error(
                opts,
                "GITSTD-GIT-OPERATION",
                format!("cannot push to {remote}: {e}"),
                2,
            );
        } else if opts.format != OutputFormat::Json {
            ui::info(&format!("Pushed to {remote}"));
        }
    }

    // post-bump hook: runs after commit+tag are created (and after push if --push).
    // Skipped when --no-commit is set (nothing was committed or tagged).
    if !opts.no_commit
        && let Err(code) = run_lifecycle_hook("post-bump", &[], opts.format)
    {
        return lifecycle_failure(opts, "post-bump", code);
    }

    if opts.format == OutputFormat::Json {
        let tag_name = if !opts.no_commit && !opts.no_tag {
            Some(format!("{tag_prefix}{new_version}"))
        } else {
            None
        };
        let commit_msg = if !opts.no_commit {
            Some(format!("chore(release): {new_version}"))
        } else {
            None
        };
        let result = BumpResultJson {
            metadata: ContractMetadata::current(),
            status: "applied",
            contract,
            version: new_version.clone(),
            previous_version: ctx.prev_version.map(String::from),
            tag: tag_name,
            updated_files: version_results
                .iter()
                .map(|r| UpdatedFileJson {
                    path: r
                        .path
                        .strip_prefix(workdir)
                        .unwrap_or(&r.path)
                        .display()
                        .to_string(),
                    old_version: r.old_version.clone(),
                    new_version: r.new_version.clone(),
                })
                .collect(),
            synced_locks: synced_locks.clone(),
            changelog: has_changelog,
            commit: commit_msg,
            pushed_to: if !opts.no_commit && !opts.no_tag {
                opts.push.clone()
            } else {
                None
            },
            version_observations: facts.observations,
            version_mismatches: facts.mismatches,
            commit_oid: (!opts.no_commit).then(|| git::head_oid(dir).ok()).flatten(),
            tag_oid: if !opts.no_commit && !opts.no_tag {
                git::resolve_rev(dir, &format!("{tag_prefix}{new_version}")).ok()
            } else {
                None
            },
            dry_run: false,
        };
        println!("{}", serde_json::to_string(&result).unwrap());
    } else {
        ui::blank();
        if opts.push.is_none() {
            ui::info("Push with: git push --follow-tags");
        }
        ui::blank();
    }

    0
}

fn planned_changelog(
    dir: &std::path::Path,
    workdir: &std::path::Path,
    config: &ProjectConfig,
    opts: &BumpOptions,
    ctx: &FinalizeContext<'_>,
) -> Option<String> {
    if opts.skip_changelog {
        return None;
    }
    let changelog_config = config.to_changelog_config();
    let release = build_version_release(
        ctx.raw_commits,
        &ctx.new_version,
        ctx.prev_version,
        &changelog_config,
    )?;
    let existing = std::fs::read_to_string(workdir.join("CHANGELOG.md")).unwrap_or_default();
    Some(standard_changelog::prepend_release(
        &existing,
        &release,
        &changelog_config,
        &git::detect_host(dir),
    ))
}
