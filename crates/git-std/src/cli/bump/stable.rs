use yansi::Paint;

use crate::config::ProjectConfig;
use crate::git;
use crate::ui;

use super::apply::finalize_bump;
use super::{BumpOptions, FinalizeContext};

/// Run the bump subcommand in stable-branch mode.
pub(super) fn run_stable(config: &ProjectConfig, opts: &BumpOptions) -> i32 {
    use crate::config::Scheme;

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
        ui::info("Creating stable branch...");
        ui::item("Branch:", &stable_branch_name);
        ui::item("Scheme:", "patch (patch-only bumps)");
        ui::blank();
        ui::info(&format!(
            "Would commit: chore(release): stabilize v{}.{}",
            cur_ver.major, cur_ver.minor,
        ));
        ui::blank();
        ui::info(&format!("Advancing {original_branch}..."));
        ui::detail(&format!(
            "{} ({bump_kind})",
            format!("{cur_ver} \u{2192} {new_version}").bold(),
        ));
        ui::blank();
        ui::info(&format!("Would commit: chore(release): {new_version}"));
        ui::info(&format!("Would tag:    {tag_prefix}{new_version}"));
        ui::blank();
        ui::info("Push with:");
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
    ui::info("Creating stable branch...");
    ui::item("Branch:", &stable_branch_name);
    ui::item("Scheme:", "patch (patch-only bumps)");
    ui::blank();
    ui::info(&format!("Committed: {}", stabilize_msg.green()));

    if let Err(e) = git::checkout_branch(dir, &original_branch) {
        ui::error(&format!("cannot checkout branch '{original_branch}': {e}"));
        return 1;
    }

    ui::blank();
    ui::info(&format!("Advancing {original_branch}..."));
    ui::detail(&format!(
        "{} ({bump_kind})",
        format!("{cur_ver} \u{2192} {new_version}").bold(),
    ));

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

    ui::info(&format!(
        "Push stable: git push origin {stable_branch_name}"
    ));
    ui::blank();

    0
}

/// Update or add `scheme = "patch"` in a `.git-std.toml` config string.
pub(super) fn update_scheme_in_config(existing: &str, scheme: &str) -> String {
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

        if is_scheme_key(trimmed) && !in_versioning {
            result.push_str(&format!("scheme = \"{scheme}\"\n"));
            found_scheme = true;
            continue;
        }

        if in_versioning && is_scheme_key(trimmed) {
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

/// Check if a trimmed TOML line is exactly the `scheme` key (not `scheme_other`, etc.).
fn is_scheme_key(trimmed: &str) -> bool {
    if let Some(rest) = trimmed.strip_prefix("scheme") {
        let rest = rest.trim_start();
        rest.starts_with('=')
    } else {
        false
    }
}
