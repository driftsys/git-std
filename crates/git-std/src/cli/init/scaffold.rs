//! Scaffold generation for `git std init`.
//!
//! Owns: `.git-std.toml` config, lifecycle hook templates, agent skill files
//! and their `.claude/skills/` symlinks.

use std::path::Path;

use crate::ui;

use super::{
    AGENTS_SKILL_BUMP_DIR, AGENTS_SKILL_COMMIT_DIR, CLAUDE_SKILL_BUMP, CLAUDE_SKILL_COMMIT,
    CONFIG_FILE, FileResult,
};

// ---------------------------------------------------------------------------
// Writers
// ---------------------------------------------------------------------------

/// Write `.git-std.toml` starter config with taplo schema directive.
pub fn write_config_file(root: &Path, force: bool) -> FileResult {
    let path = root.join(CONFIG_FILE);
    if path.exists() && !force {
        return FileResult::Skipped;
    }

    let template = generate_config_template();
    if let Err(e) = std::fs::write(&path, &template) {
        ui::error(&format!("cannot write {CONFIG_FILE}: {e}"));
        return FileResult::Error;
    }

    FileResult::Created
}

/// Create a symlink from `.agents/skills/<name>/SKILL.md` to `../../skills/<name>.md`.
pub fn write_skill_source(
    root: &Path,
    skill_dir: &str,
    skill_name: &str,
    force: bool,
) -> FileResult {
    let skill_path = root.join(skill_dir).join("SKILL.md");

    // Ensure parent directory exists
    if let Some(parent) = skill_path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        ui::error(&format!("cannot create {}: {e}", parent.display()));
        return FileResult::Error;
    }

    // Remove existing file/symlink if force is set
    if skill_path.exists() || skill_path.symlink_metadata().is_ok() {
        if !force {
            return FileResult::Skipped;
        }
        if let Err(e) = std::fs::remove_file(&skill_path) {
            ui::error(&format!("cannot remove {}: {e}", skill_path.display()));
            return FileResult::Error;
        }
    }

    // Create relative symlink: .agents/skills/std-commit/SKILL.md → ../../skills/std-commit.md
    let relative_target = format!("../../skills/{skill_name}.md");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        if let Err(e) = symlink(&relative_target, &skill_path) {
            ui::error(&format!(
                "cannot create symlink {}: {e}",
                skill_path.display()
            ));
            return FileResult::Error;
        }
    }
    #[cfg(not(unix))]
    {
        // On non-Unix, write a text file pointing to the target as a fallback
        if let Err(e) = std::fs::write(&skill_path, format!("{relative_target}\n")) {
            ui::error(&format!("cannot write {}: {e}", skill_path.display()));
            return FileResult::Error;
        }
    }

    FileResult::Created
}

/// Create a `.claude/skills/` symlink pointing back to `.agents/skills/`.
pub fn write_skill_symlink(root: &Path, link: &str, target: &str, force: bool) -> FileResult {
    // Ensure .claude/skills/ exists
    let link_path = root.join(link);
    if let Some(parent) = link_path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        ui::error(&format!("cannot create {}: {e}", parent.display()));
        return FileResult::Error;
    }
    if link_path.exists() || link_path.symlink_metadata().is_ok() {
        if !force {
            return FileResult::Skipped;
        }
        let _ = std::fs::remove_file(&link_path);
    }
    // Relative symlink: from .claude/skills/std-commit → ../../.agents/skills/std-commit
    let relative_target = format!("../../{target}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        if let Err(e) = symlink(&relative_target, &link_path) {
            ui::error(&format!("cannot create symlink {link}: {e}"));
            return FileResult::Error;
        }
    }
    #[cfg(not(unix))]
    {
        // On non-Unix, write a text file pointing to the target as a fallback
        if let Err(e) = std::fs::write(&link_path, format!("{relative_target}\n")) {
            ui::error(&format!("cannot write {link}: {e}"));
            return FileResult::Error;
        }
    }
    FileResult::Created
}

/// Return all skill definitions for scaffolding.
///
/// Each tuple: `(skill_name, skill_dir, claude_link)`.
pub fn skill_definitions() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("std-commit", AGENTS_SKILL_COMMIT_DIR, CLAUDE_SKILL_COMMIT),
        ("std-bump", AGENTS_SKILL_BUMP_DIR, CLAUDE_SKILL_BUMP),
    ]
}

// ---------------------------------------------------------------------------
// Generated content
// ---------------------------------------------------------------------------

/// Generate the `.git-std.toml` starter config content.
fn generate_config_template() -> String {
    "\
#:schema https://driftsys.github.io/git-std/schemas/v1/git-std.schema.json

# scheme = \"semver\"          # semver | calver | patch
# strict = false             # enforce types/scopes without --strict flag
# types = [\"feat\", \"fix\", \"docs\", \"style\", \"refactor\",
#           \"perf\", \"test\", \"chore\", \"ci\", \"build\", \"revert\"]
# scopes = \"auto\"            # \"auto\" | [\"scope1\", \"scope2\"] | omit
"
    .to_string()
}

/// Generate a bump lifecycle hook template for the given hook name.
pub fn generate_lifecycle_hook_template(hook_name: &str) -> String {
    match hook_name {
        "pre-bump" => "\
# git-std hooks — pre-bump.hooks
#
# Runs before version detection. Non-zero exit aborts the bump.
# Use for: guard checks (clean tree, correct branch, tests pass).
#
#   !  required   abort bump on failure
#   ?  advisory   warn on failure, never abort
#
# Examples:
#   ! cargo test --workspace
#   ! git diff --exit-code   # abort if working tree is dirty
#
"
        .to_string(),
        "post-version" => "\
# git-std hooks — post-version.hooks
#
# Runs after version files are updated. $1 is the new version string.
# Use for: building artifacts, stamping binaries, generating manifests.
#
#   !  required   abort bump on failure
#   ?  advisory   warn on failure, never abort
#
# Examples:
#   ! cargo build --release
#   ? cp target/release/mybin dist/
#
"
        .to_string(),
        "post-changelog" => "\
# git-std hooks — post-changelog.hooks
#
# Runs after CHANGELOG.md is written, before staging and commit.
# Use for: linting or reformatting the changelog.
#
#   !  required   abort bump on failure
#   ?  advisory   warn on failure, never abort
#
# Examples:
#   ? npx markdownlint CHANGELOG.md
#
"
        .to_string(),
        "post-bump" => "\
# git-std hooks — post-bump.hooks
#
# Runs after commit and tag are created (and after push if --push).
# Use for: publishing, deployment, notifications.
#
#   !  required   report failure
#   ?  advisory   warn on failure, always continues
#
# Examples:
#   ! cargo publish
#   ? curl -X POST https://hooks.slack.com/...
#
"
        .to_string(),
        _ => format!("# git-std hooks — {hook_name}.hooks\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_template_has_schema_directive() {
        let t = generate_config_template();
        assert!(t.starts_with("#:schema "));
        assert!(t.contains("git-std.schema.json"));
    }

    #[test]
    fn config_template_has_commented_fields() {
        let t = generate_config_template();
        assert!(t.contains("# scheme"));
        assert!(t.contains("# strict"));
        assert!(t.contains("# types"));
        assert!(t.contains("# scopes"));
    }

    #[test]
    fn lifecycle_hook_templates_have_headers() {
        for hook in super::super::LIFECYCLE_HOOKS {
            let t = generate_lifecycle_hook_template(hook);
            assert!(
                t.contains(&format!("# git-std hooks — {hook}.hooks")),
                "{hook}.hooks template should have header"
            );
            assert!(
                t.contains("!  required"),
                "{hook}.hooks should document ! sigil"
            );
            assert!(
                t.contains("?  advisory"),
                "{hook}.hooks should document ? sigil"
            );
        }
    }

    #[test]
    fn pre_bump_template_mentions_when_it_runs() {
        let t = generate_lifecycle_hook_template("pre-bump");
        assert!(t.contains("before version detection"));
        assert!(t.contains("abort bump on failure"));
    }

    #[test]
    fn post_version_template_mentions_version_arg() {
        let t = generate_lifecycle_hook_template("post-version");
        assert!(t.contains("$1 is the new version string"));
    }

    #[test]
    fn post_changelog_template_mentions_when_it_runs() {
        let t = generate_lifecycle_hook_template("post-changelog");
        assert!(t.contains("after CHANGELOG.md is written"));
    }

    #[test]
    fn post_bump_template_mentions_when_it_runs() {
        let t = generate_lifecycle_hook_template("post-bump");
        assert!(t.contains("after commit and tag are created"));
    }

    #[test]
    fn std_commit_skill_has_frontmatter() {
        let s = include_str!("../../../../../skills/std-commit.md");
        assert!(s.starts_with("---\nname: std-commit\n"));
        assert!(s.contains("git std --context"));
        assert!(s.contains("git std commit"));
    }

    #[test]
    fn std_commit_skill_includes_message_guidelines() {
        let s = include_str!("../../../../../skills/std-commit.md");
        assert!(
            s.contains("50 characters"),
            "skill should document 50 char limit"
        );
        assert!(
            s.contains("72 characters"),
            "skill should document 72 char body wrap"
        );
        assert!(s.contains("--body"), "skill should mention --body flag");
        assert!(
            s.contains("what") && s.contains("why"),
            "skill should explain what/why guidance"
        );
    }

    #[test]
    fn std_bump_skill_has_frontmatter() {
        let s = include_str!("../../../../../skills/std-bump.md");
        assert!(s.starts_with("---\nname: std-bump\n"));
        assert!(s.contains("git std bump --dry-run"));
        assert!(s.contains("--push"));
    }
}
