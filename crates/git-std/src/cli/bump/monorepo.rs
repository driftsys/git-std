//! Per-package version planning for monorepo workspaces.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;
use yansi::Paint;

use crate::app::OutputFormat;
use crate::config::deps::{self, DependencyGraph};
use crate::config::{PackageConfig, ProjectConfig};
use crate::git;
use crate::ui;

use super::BumpOptions;

/// A per-package bump plan entry.
pub(crate) struct PackageBumpPlan {
    /// Package name.
    pub name: String,
    /// Path to the package root, relative to the repository root.
    pub path: String,
    /// Previous version string, if a tag was found.
    pub prev_version: Option<String>,
    /// Computed next version string.
    pub new_version: String,
    /// Determined bump level.
    pub bump_level: standard_version::BumpLevel,
    /// Raw commits `(sha, message)` that touched this package.
    pub raw_commits: Vec<(String, String)>,
    /// Full tag name for the new version.
    pub tag_name: String,
    /// If this bump was caused (or elevated) by a dependency cascade, the
    /// source package name is recorded here.
    pub cascade_from: Option<String>,
}

/// JSON schema for a per-package bump plan entry.
#[derive(Serialize)]
struct PackagePlanJson {
    name: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_version: Option<String>,
    new_version: String,
    bump_level: String,
    tag: String,
    commit_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    cascade_from: Option<String>,
}

/// JSON schema for the full monorepo bump plan.
#[derive(Serialize)]
struct MonorepoPlanJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    root_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    root_previous_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    root_tag: Option<String>,
    packages: Vec<PackagePlanJson>,
    dry_run: bool,
}

/// Find the latest tag matching a per-package tag template.
///
/// Given a tag template like `{name}@{version}`, computes the prefix for
/// the package name (e.g. `core@`) and finds the best semver tag from the
/// pre-collected tag list.
fn find_latest_package_tag(
    tags: &[(String, String)],
    template: &str,
    pkg_name: &str,
) -> Option<(String, semver::Version)> {
    let prefix = template
        .replace("{name}", pkg_name)
        .replace("{version}", "");

    let mut best: Option<(String, semver::Version)> = None;
    for (oid, name) in tags {
        let ver_str = match name.strip_prefix(&prefix) {
            Some(s) => s,
            None => continue,
        };
        let ver = match semver::Version::parse(ver_str) {
            Ok(v) => v,
            Err(_) => continue,
        };
        match &best {
            Some((_, current_best)) if ver <= *current_best => {}
            _ => best = Some((oid.clone(), ver)),
        }
    }
    best
}

/// Build a tag name from the template, package name, and version string.
fn build_tag_name(template: &str, pkg_name: &str, version: &str) -> String {
    template
        .replace("{name}", pkg_name)
        .replace("{version}", version)
}

/// Format a bump level as a human-readable reason string.
fn bump_reason(level: standard_version::BumpLevel) -> &'static str {
    match level {
        standard_version::BumpLevel::Major => "major \u{2014} breaking change",
        standard_version::BumpLevel::Minor => "minor \u{2014} new feature",
        standard_version::BumpLevel::Patch => "patch \u{2014} bug fix",
    }
}

/// Plan a single package bump. Returns `None` if no bump-worthy commits exist.
fn plan_package(
    dir: &Path,
    pkg: &PackageConfig,
    head_oid: &str,
    tag_template: &str,
    tags: &[(String, String)],
) -> Option<PackageBumpPlan> {
    let latest_tag = find_latest_package_tag(tags, tag_template, &pkg.name);

    let tag_oid = latest_tag.as_ref().map(|(oid, _)| oid.as_str());
    let raw_commits = match git::walk_commits_for_path(dir, head_oid, tag_oid, &[&pkg.path]) {
        Ok(c) => c,
        Err(e) => {
            ui::warning(&format!("{}: cannot walk commits: {e}", pkg.name));
            return None;
        }
    };

    if raw_commits.is_empty() {
        return None;
    }

    let parsed: Vec<standard_commit::ConventionalCommit> = raw_commits
        .iter()
        .filter_map(|(_, msg)| standard_commit::parse(msg).ok())
        .collect();

    let bump_level = standard_version::determine_bump(&parsed)?;

    let cur_ver = latest_tag
        .as_ref()
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| semver::Version::new(0, 1, 0));

    let new_version = if latest_tag.is_none() {
        // First release: default to 0.1.0.
        semver::Version::new(0, 1, 0)
    } else {
        standard_version::apply_bump(&cur_ver, bump_level)
    };

    let prev_version = latest_tag.as_ref().map(|(_, v)| v.to_string());
    let tag_name = build_tag_name(tag_template, &pkg.name, &new_version.to_string());

    Some(PackageBumpPlan {
        name: pkg.name.clone(),
        path: pkg.path.clone(),
        prev_version,
        new_version: new_version.to_string(),
        bump_level,
        raw_commits,
        tag_name,
        cascade_from: None,
    })
}

/// Plan version bumps for all packages in a monorepo.
///
/// In dry-run mode, prints the plan. Otherwise, applies the bump:
/// updates version files, generates changelogs, creates a commit, and tags.
pub(super) fn plan_monorepo_bump(
    config: &ProjectConfig,
    opts: &BumpOptions,
    packages_filter: &[String],
) -> i32 {
    let dir = Path::new(".");

    let workdir = match git::workdir(dir) {
        Ok(w) => w,
        Err(_) => {
            ui::error("bare repository not supported");
            return 1;
        }
    };

    let all_packages = config.resolved_packages(&workdir);
    if all_packages.is_empty() {
        ui::error(
            "monorepo = true but no packages found (configure [[packages]] or use a supported workspace layout)",
        );
        return 1;
    }

    let packages: Vec<&PackageConfig> = if packages_filter.is_empty() {
        all_packages.iter().collect()
    } else {
        let filtered: Vec<&PackageConfig> = all_packages
            .iter()
            .filter(|p| packages_filter.iter().any(|f| f == &p.name))
            .collect();
        if filtered.is_empty() {
            ui::error("no packages matched the --package filter");
            return 1;
        }
        filtered
    };

    let head_oid = match git::head_oid(dir) {
        Ok(oid) => oid,
        Err(e) => {
            ui::error(&format!("cannot resolve HEAD: {e}"));
            return 1;
        }
    };

    let tag_template = &config.versioning.tag_template;

    // Collect all tags once to avoid O(n) git subprocess calls.
    let all_tags = match git::collect_tags(&workdir) {
        Ok(t) => t,
        Err(e) => {
            ui::error(&format!("cannot read tags: {e}"));
            return 1;
        }
    };

    // Plan per-package bumps.
    let mut package_plans: Vec<PackageBumpPlan> = Vec::new();
    for pkg in &packages {
        if let Some(plan) = plan_package(&workdir, pkg, &head_oid, tag_template, &all_tags) {
            package_plans.push(plan);
        }
    }

    // Apply dependency cascade when not filtering to specific packages.
    if packages_filter.is_empty() {
        let dep_graph = deps::resolve_dependency_graph(&workdir, &all_packages);
        if !dep_graph.is_empty() {
            apply_cascade(
                &mut package_plans,
                &all_packages,
                &dep_graph,
                tag_template,
                &all_tags,
            );
        }
    }

    // Plan root version bump via existing dispatch logic.
    let root_plan = plan_root(config, dir);

    if package_plans.is_empty() && root_plan.is_none() {
        ui::blank();
        ui::info("no bump-worthy commits found in any package");
        ui::blank();
        return 0;
    }

    if opts.dry_run {
        if opts.format == OutputFormat::Json {
            print_plan_json(&root_plan, &package_plans);
        } else {
            print_plan_text(&root_plan, &package_plans, &config.versioning.tag_prefix);
        }
        return 0;
    }

    finalize_monorepo_bump(dir, &workdir, config, opts, &root_plan, &package_plans)
}

/// Apply dependency cascade: when a package bumps, its dependents get at
/// least a patch bump. Iterates until stable (transitive cascade).
fn apply_cascade(
    plans: &mut Vec<PackageBumpPlan>,
    all_packages: &[PackageConfig],
    dep_graph: &DependencyGraph,
    tag_template: &str,
    tags: &[(String, String)],
) {
    let pkg_by_name: std::collections::HashMap<&str, &PackageConfig> =
        all_packages.iter().map(|p| (p.name.as_str(), p)).collect();

    // Iterate until no new cascade bumps are added.
    loop {
        let bumped: HashSet<String> = plans.iter().map(|p| p.name.clone()).collect();
        let mut new_cascades: Vec<PackageBumpPlan> = Vec::new();

        for plan in plans.iter() {
            for dependent_name in dep_graph.dependents_of(&plan.name) {
                if bumped.contains(dependent_name.as_str()) {
                    continue;
                }
                let Some(pkg) = pkg_by_name.get(dependent_name.as_str()) else {
                    continue;
                };

                let cascade_plan = create_cascade_plan(pkg, tag_template, &plan.name, tags);
                if let Some(cp) = cascade_plan {
                    new_cascades.push(cp);
                }
            }
        }

        if new_cascades.is_empty() {
            break;
        }

        // Deduplicate (a package may be reachable from multiple bumped deps).
        let mut seen = HashSet::new();
        new_cascades.retain(|p| seen.insert(p.name.clone()));

        plans.extend(new_cascades);
    }
}

/// Create a cascade patch bump for a dependent package.
fn create_cascade_plan(
    pkg: &PackageConfig,
    tag_template: &str,
    cascade_source: &str,
    tags: &[(String, String)],
) -> Option<PackageBumpPlan> {
    let latest_tag = find_latest_package_tag(tags, tag_template, &pkg.name);

    let cur_ver = latest_tag
        .as_ref()
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| semver::Version::new(0, 1, 0));

    let new_version = if latest_tag.is_none() {
        semver::Version::new(0, 1, 0)
    } else {
        standard_version::apply_bump(&cur_ver, standard_version::BumpLevel::Patch)
    };

    let prev_version = latest_tag.as_ref().map(|(_, v)| v.to_string());
    let tag_name = build_tag_name(tag_template, &pkg.name, &new_version.to_string());

    Some(PackageBumpPlan {
        name: pkg.name.clone(),
        path: pkg.path.clone(),
        prev_version,
        new_version: new_version.to_string(),
        bump_level: standard_version::BumpLevel::Patch,
        raw_commits: Vec::new(),
        tag_name,
        cascade_from: Some(cascade_source.to_string()),
    })
}

/// Minimal root version info for the plan output.
struct RootPlan {
    prev_version: Option<String>,
    new_version: String,
    tag: String,
    raw_commits: Vec<(String, String)>,
}

/// Compute the root version bump using existing logic.
fn plan_root(config: &ProjectConfig, dir: &Path) -> Option<RootPlan> {
    let tag_prefix = &config.versioning.tag_prefix;

    let current_version = match git::find_latest_version_tag(dir, tag_prefix) {
        Ok(Some((oid, ver))) => Some((oid, ver)),
        Ok(None) => None,
        Err(_) => None,
    };

    let head_oid = match git::head_oid(dir) {
        Ok(oid) => oid,
        Err(_) => return None,
    };

    let tag_oid = current_version.as_ref().map(|(oid, _)| oid.as_str());
    let raw_commits = match git::walk_commits(dir, &head_oid, tag_oid) {
        Ok(c) => c,
        Err(_) => return None,
    };

    let parsed: Vec<standard_commit::ConventionalCommit> = raw_commits
        .iter()
        .filter_map(|(_, msg)| standard_commit::parse(msg).ok())
        .collect();

    let bump_level = standard_version::determine_bump(&parsed)?;

    let cur_ver = current_version
        .as_ref()
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| semver::Version::new(0, 0, 0));

    let new_version = standard_version::apply_bump(&cur_ver, bump_level);
    let prev_version = current_version.as_ref().map(|(_, v)| v.to_string());
    let tag = format!("{tag_prefix}{new_version}");

    Some(RootPlan {
        prev_version,
        new_version: new_version.to_string(),
        tag,
        raw_commits,
    })
}

// ── Finalize (actual apply) ─────────────────────────────────────────

/// JSON output schema for the monorepo bump result.
#[derive(Serialize)]
struct MonorepoBumpResultJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    root_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    root_previous_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    root_tag: Option<String>,
    packages: Vec<PackagePlanJson>,
    synced_locks: Vec<String>,
    changelog: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit: Option<String>,
    dry_run: bool,
}

/// Execute the monorepo bump: write version files, generate changelogs,
/// create a single commit, and apply multiple tags.
fn finalize_monorepo_bump(
    dir: &Path,
    workdir: &Path,
    config: &ProjectConfig,
    opts: &BumpOptions,
    root_plan: &Option<RootPlan>,
    package_plans: &[PackageBumpPlan],
) -> i32 {
    let changelog_config = config.to_changelog_config();
    let host = git::detect_host(dir);

    let all_packages = config.resolved_packages(workdir);
    let pkg_configs: std::collections::HashMap<&str, &PackageConfig> =
        all_packages.iter().map(|p| (p.name.as_str(), p)).collect();

    let mut all_modified: Vec<PathBuf> = Vec::new();
    let mut all_synced_locks: Vec<String> = Vec::new();

    // 1. Update version files per-package via ecosystem tooling.
    for plan in package_plans {
        let pkg_dir = workdir.join(&plan.path);

        let custom_files: Vec<standard_version::CustomVersionFile> = pkg_configs
            .get(plan.name.as_str())
            .and_then(|pc| pc.version_files.as_ref())
            .map(|vfs| {
                vfs.iter()
                    .map(|vf| standard_version::CustomVersionFile {
                        path: PathBuf::from(&vf.path),
                        pattern: vf.regex.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let bump_result = crate::ecosystem::run_bump(&pkg_dir, &plan.new_version, &custom_files);

        for r in &bump_result.update_results {
            if opts.format != OutputFormat::Json {
                let rel = r.path.strip_prefix(workdir).unwrap_or(&r.path).display();
                ui::item(
                    &rel.to_string(),
                    &format!("{} \u{2192} {}", r.old_version, r.new_version),
                );
            }
        }

        all_modified.extend(bump_result.update_results.iter().map(|r| r.path.clone()));
        all_modified.extend(bump_result.modified_paths);
        for lock in bump_result.synced_locks {
            if !all_synced_locks.contains(&lock) {
                all_synced_locks.push(lock);
            }
        }
    }

    // Also update root version files if there is a root plan.
    if let Some(root) = root_plan {
        let custom_files: Vec<standard_version::CustomVersionFile> = config
            .version_files
            .iter()
            .map(|vf| standard_version::CustomVersionFile {
                path: PathBuf::from(&vf.path),
                pattern: vf.regex.clone(),
            })
            .collect();
        let bump_result = crate::ecosystem::run_bump(workdir, &root.new_version, &custom_files);
        for r in &bump_result.update_results {
            if opts.format != OutputFormat::Json {
                let rel = r.path.strip_prefix(workdir).unwrap_or(&r.path).display();
                ui::item(
                    &rel.to_string(),
                    &format!("{} \u{2192} {}", r.old_version, r.new_version),
                );
            }
        }
        all_modified.extend(bump_result.update_results.iter().map(|r| r.path.clone()));
        all_modified.extend(bump_result.modified_paths);
        for lock in bump_result.synced_locks {
            if !all_synced_locks.contains(&lock) {
                all_synced_locks.push(lock);
            }
        }
    }

    // 2. Generate per-package changelogs.
    if !opts.skip_changelog {
        for plan in package_plans {
            let pkg_changelog_path = workdir.join(&plan.path).join("CHANGELOG.md");

            let release = super::apply::build_version_release(
                &plan.raw_commits,
                &plan.new_version,
                plan.prev_version.as_deref(),
                &changelog_config,
            );
            if let Some(release) = release {
                let existing = std::fs::read_to_string(&pkg_changelog_path).unwrap_or_default();
                let output = standard_changelog::prepend_release(
                    &existing,
                    &release,
                    &changelog_config,
                    &host,
                );
                if let Err(e) = std::fs::write(&pkg_changelog_path, &output) {
                    ui::warning(&format!("{}: cannot write CHANGELOG.md: {e}", plan.name));
                } else {
                    all_modified.push(pkg_changelog_path);
                }
            }
        }

        // 3. Generate root changelog (all commits from root plan).
        if let Some(root) = root_plan {
            let root_changelog_path = workdir.join("CHANGELOG.md");
            let release = super::apply::build_version_release(
                &root.raw_commits,
                &root.new_version,
                root.prev_version.as_deref(),
                &changelog_config,
            );
            if let Some(release) = release {
                let existing = std::fs::read_to_string(&root_changelog_path).unwrap_or_default();
                let output = standard_changelog::prepend_release(
                    &existing,
                    &release,
                    &changelog_config,
                    &host,
                );
                if let Err(e) = std::fs::write(&root_changelog_path, &output) {
                    ui::error(&format!("cannot write root CHANGELOG.md: {e}"));
                    return 1;
                }
                all_modified.push(root_changelog_path);
            }
        }
    }

    // Print changelog info for text output.
    if !opts.skip_changelog && opts.format != OutputFormat::Json {
        ui::blank();
        ui::info("Changelog:");
        for plan in package_plans {
            if !plan.raw_commits.is_empty() {
                ui::item(
                    &format!("{}/CHANGELOG.md", plan.path),
                    &format!("prepended {} section", plan.tag_name),
                );
            }
        }
        if root_plan.is_some() {
            ui::item(
                "CHANGELOG.md",
                &format!("prepended {} section", root_plan.as_ref().unwrap().tag),
            );
        }
    }

    // 4. Stage all modified files and create commit.
    if !opts.no_commit {
        let mut paths_to_stage: Vec<String> = all_modified
            .iter()
            .filter_map(|p| {
                p.strip_prefix(workdir)
                    .ok()
                    .map(|r| r.to_string_lossy().into_owned())
            })
            .collect();
        for lock in &all_synced_locks {
            paths_to_stage.push(lock.clone());
        }
        let stage_refs: Vec<&str> = paths_to_stage.iter().map(|s| s.as_str()).collect();
        if let Err(e) = git::stage_files(dir, &stage_refs) {
            ui::error(&format!("cannot stage files: {e}"));
            return 1;
        }

        // 5. Single commit with aggregated message.
        let commit_msg = build_commit_message(root_plan, package_plans);

        if opts.sign {
            if let Err(e) = git::create_signed_commit(dir, &commit_msg) {
                ui::error(&e.to_string());
                return 1;
            }
        } else if let Err(e) = git::create_commit(dir, &commit_msg) {
            ui::error(&format!("cannot create commit: {e}"));
            return 1;
        }

        if opts.format != OutputFormat::Json {
            ui::blank();
            ui::info(&format!("Committed: {}", commit_msg.green()));
        }
    }

    // 6. Multiple tags on the commit.
    if !opts.no_commit && !opts.no_tag {
        // Root tag first.
        if let Some(root) = root_plan {
            let tag_msg = root.new_version.clone();
            if opts.sign {
                if let Err(e) = git::create_signed_tag(dir, &root.tag, &tag_msg) {
                    ui::error(&e.to_string());
                    return 1;
                }
            } else if let Err(e) = git::create_annotated_tag(dir, &root.tag, &tag_msg) {
                ui::error(&format!("cannot create tag: {e}"));
                return 1;
            }
            if opts.format != OutputFormat::Json {
                ui::info(&format!("Tagged:    {}", root.tag.green()));
            }
        }

        // Per-package tags.
        for plan in package_plans {
            let tag_msg = format!("{} {}", plan.name, plan.new_version);
            if opts.sign {
                if let Err(e) = git::create_signed_tag(dir, &plan.tag_name, &tag_msg) {
                    ui::error(&e.to_string());
                    return 1;
                }
            } else if let Err(e) = git::create_annotated_tag(dir, &plan.tag_name, &tag_msg) {
                ui::error(&format!("cannot create tag: {e}"));
                return 1;
            }
            if opts.format != OutputFormat::Json {
                ui::info(&format!("Tagged:    {}", plan.tag_name.green()));
            }
        }
    }

    // JSON output.
    if opts.format == OutputFormat::Json {
        let commit_msg = if !opts.no_commit {
            Some(build_commit_message(root_plan, package_plans))
        } else {
            None
        };
        let result = MonorepoBumpResultJson {
            root_version: root_plan.as_ref().map(|r| r.new_version.clone()),
            root_previous_version: root_plan.as_ref().and_then(|r| r.prev_version.clone()),
            root_tag: if !opts.no_commit && !opts.no_tag {
                root_plan.as_ref().map(|r| r.tag.clone())
            } else {
                None
            },
            packages: package_plans
                .iter()
                .map(|p| PackagePlanJson {
                    name: p.name.clone(),
                    path: p.path.clone(),
                    previous_version: p.prev_version.clone(),
                    new_version: p.new_version.clone(),
                    bump_level: format!("{:?}", p.bump_level).to_lowercase(),
                    tag: p.tag_name.clone(),
                    commit_count: p.raw_commits.len(),
                    cascade_from: p.cascade_from.clone(),
                })
                .collect(),
            synced_locks: all_synced_locks,
            changelog: !opts.skip_changelog,
            commit: commit_msg,
            dry_run: false,
        };
        println!("{}", serde_json::to_string(&result).unwrap());
    } else {
        ui::blank();
        ui::info("Push with: git push --follow-tags");
        ui::blank();
    }

    0
}

/// Build the aggregated commit message for a monorepo release.
///
/// Format: `chore(release): v1.0.0, core@1.2.0, cli@0.5.0`
fn build_commit_message(root_plan: &Option<RootPlan>, package_plans: &[PackageBumpPlan]) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(root) = root_plan {
        parts.push(format!("v{}", root.new_version));
    }
    for plan in package_plans {
        parts.push(format!("{}@{}", plan.name, plan.new_version));
    }
    format!("chore(release): {}", parts.join(", "))
}

/// Print the monorepo bump plan as human-readable text.
fn print_plan_text(
    root_plan: &Option<RootPlan>,
    package_plans: &[PackageBumpPlan],
    tag_prefix: &str,
) {
    ui::blank();

    if let Some(root) = root_plan {
        let prev = root.prev_version.as_deref().unwrap_or("none");
        ui::heading(
            "Root: ",
            &format!(
                "{} (tag: {})",
                format!("{prev} \u{2192} {}", root.new_version).bold(),
                format!("{tag_prefix}{}", root.new_version).bold(),
            ),
        );
    }

    if !package_plans.is_empty() {
        ui::blank();
        ui::heading("Packages:", "");
        for plan in package_plans {
            let prev = plan.prev_version.as_deref().unwrap_or("none");
            let reason = match &plan.cascade_from {
                Some(source) => format!("patch — dependency cascade from {source}"),
                None => bump_reason(plan.bump_level).to_string(),
            };
            ui::info(&format!(
                "{}: {} ({})",
                plan.name.bold(),
                format!("{prev} \u{2192} {}", plan.new_version).bold(),
                reason,
            ));
            ui::detail(&format!(
                "tag: {}  ({} commit{})",
                plan.tag_name,
                plan.raw_commits.len(),
                if plan.raw_commits.len() == 1 { "" } else { "s" },
            ));
        }
    }

    ui::blank();
}

/// Print the monorepo bump plan as JSON.
fn print_plan_json(root_plan: &Option<RootPlan>, package_plans: &[PackageBumpPlan]) {
    let result = MonorepoPlanJson {
        root_version: root_plan.as_ref().map(|r| r.new_version.clone()),
        root_previous_version: root_plan.as_ref().and_then(|r| r.prev_version.clone()),
        root_tag: root_plan.as_ref().map(|r| r.tag.clone()),
        packages: package_plans
            .iter()
            .map(|p| PackagePlanJson {
                name: p.name.clone(),
                path: p.path.clone(),
                previous_version: p.prev_version.clone(),
                new_version: p.new_version.clone(),
                bump_level: format!("{:?}", p.bump_level).to_lowercase(),
                tag: p.tag_name.clone(),
                commit_count: p.raw_commits.len(),
                cascade_from: p.cascade_from.clone(),
            })
            .collect(),
        dry_run: true,
    };
    println!("{}", serde_json::to_string(&result).unwrap());
}
