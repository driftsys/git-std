//! Scaffold generation for `git std init`.
//!
//! Owns: `.git-std.toml` config, lifecycle hook templates, agent skill files
//! and their `.claude/skills/` symlinks.

use std::path::Path;

use crate::ui;

use super::{
    AGENTS_SKILL_BUMP_DIR, AGENTS_SKILL_BUMP_FILE, AGENTS_SKILL_COMMIT_DIR,
    AGENTS_SKILL_COMMIT_FILE, CLAUDE_SKILL_BUMP, CLAUDE_SKILL_COMMIT, CONFIG_FILE, FileResult,
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

/// Write an agent skill file under `.agents/skills/`.
pub fn write_skill(root: &Path, dir: &str, file: &str, content: &str, force: bool) -> FileResult {
    let file_path = root.join(file);
    if file_path.exists() && !force {
        return FileResult::Skipped;
    }
    if let Err(e) = std::fs::create_dir_all(root.join(dir)) {
        ui::error(&format!("cannot create {dir}: {e}"));
        return FileResult::Error;
    }
    if let Err(e) = std::fs::write(&file_path, content) {
        ui::error(&format!("cannot write {file}: {e}"));
        return FileResult::Error;
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
/// Each tuple: `(dir, file, claude_link, content)`.
pub fn skill_definitions() -> Vec<(&'static str, &'static str, &'static str, String)> {
    vec![
        (
            AGENTS_SKILL_COMMIT_DIR,
            AGENTS_SKILL_COMMIT_FILE,
            CLAUDE_SKILL_COMMIT,
            generate_std_commit_skill(),
        ),
        (
            AGENTS_SKILL_BUMP_DIR,
            AGENTS_SKILL_BUMP_FILE,
            CLAUDE_SKILL_BUMP,
            generate_std_bump_skill(),
        ),
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

fn generate_std_commit_skill() -> String {
    "\
---
name: std-commit
description: Author a conventional commit for staged changes using git std — use when asked to \"commit\", \"write a commit\", or \"commit my changes\".
---

Run `git std --context`, then author a `git std commit` invocation for the staged changes.

## Rules

- If `git std --version` fails:
  - If `./bootstrap` exists at repo root, ask: \"git std is not installed —
    run `./bootstrap` to install it?\" If confirmed, run it.
  - Otherwise ask: \"git std is not installed — install it now?\" If confirmed,
    run `curl -fsSL https://driftsys.github.io/git-std/install.sh | bash`
- Use only the **Types** and **Scopes** listed in the context — never invent either.
- If scopes are `(required, strict)`, `--scope` is mandatory.
- If the output signals `Not bootstrapped` or `Nothing staged`, print the message and stop.
- Match changed file paths against workspace package names to determine `--scope`.
  If the diff spans multiple scopes, pick the most-changed one.
- `--message`: imperative mood, lowercase, no trailing period.
- If the diff contains a clear breaking change, add `--breaking \"short description\"`.
- For issue refs:
  - If context specifies that refs are required for this commit type, ask for the
    ref and do not proceed without one.
  - Otherwise, if `--type` is `feat` or `fix`, ask:
    \"Related issue? (e.g. #123 — leave blank to skip)\"
  - If the branch name follows `{type}/{issue}-{description}`, extract the issue
    number and pre-fill it as the default.
  - If provided, append `--footer \"Closes #N\"`.

Do not run the command without the user's approval.
"
    .to_string()
}

fn generate_std_bump_skill() -> String {
    "\
---
name: std-bump
description: Bump the project version using git std — use when asked to \"bump\", \"release\", \"cut a release\", or \"tag a version\".
---

Orchestrate a version bump using `git std bump`.

## Rules

- If `git std --version` fails:
  - If `./bootstrap` exists at repo root, ask: \"git std is not installed —
    run `./bootstrap` to install it?\" If confirmed, run it.
  - Otherwise ask: \"git std is not installed — install it now?\" If confirmed,
    run `curl -fsSL https://driftsys.github.io/git-std/install.sh | bash`
- Run `git std --context` to assess project state:
  - If `Not bootstrapped`, stop and print the message.
  - If not on a stable branch (main/master), suggest `--prerelease` unless
    the user explicitly asks for a stable release.
  - If context shows no tag yet, use `--first-release`.
- Run `git std bump --dry-run` and show the output. Ask: \"Proceed with this bump?\"
  Do not continue without confirmation.
- If the workspace has multiple packages, ask: \"Bump all packages or specific
  ones? (leave blank for all, or list e.g. git-std, standard-commit)\"
  Add `--package` flags if specific packages are named.
- Ask: \"Push commit and tags after? (--push)\" before running.
- Run `git std bump [--prerelease] [--first-release] [--package ...] [--push]`
  with the confirmed flags.

Do not run any bump command without the user's approval.
"
    .to_string()
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
        let s = generate_std_commit_skill();
        assert!(s.starts_with("---\nname: std-commit\n"));
        assert!(s.contains("git std --context"));
        assert!(s.contains("git std commit"));
    }

    #[test]
    fn std_bump_skill_has_frontmatter() {
        let s = generate_std_bump_skill();
        assert!(s.starts_with("---\nname: std-bump\n"));
        assert!(s.contains("git std bump --dry-run"));
        assert!(s.contains("--push"));
    }
}
