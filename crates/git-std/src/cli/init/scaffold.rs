//! Scaffold generation for `git std init`.
//!
//! Owns: `.git-std.toml` config and lifecycle hook templates.

use std::path::Path;

use crate::ui;

use super::{CONFIG_FILE, FileResult};

// ---------------------------------------------------------------------------
// Writers
// ---------------------------------------------------------------------------

/// Write `.git-std.toml` starter config with taplo schema directive.
///
/// When the file does not exist, writes the full template.
/// When it exists and `force` is false, merges missing default keys
/// into the existing config (backing up the original first).
/// When `force` is true, overwrites entirely.
pub fn write_config_file(root: &Path, force: bool) -> FileResult {
    let path = root.join(CONFIG_FILE);

    if !path.exists() || force {
        let template = generate_config_template();
        if let Err(e) = std::fs::write(&path, &template) {
            ui::error(&format!("cannot write {CONFIG_FILE}: {e}"));
            return FileResult::Error;
        }
        return FileResult::Created;
    }

    // File exists and !force → attempt smart merge.
    let existing_content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            ui::error(&format!("cannot read {CONFIG_FILE}: {e}"));
            return FileResult::Error;
        }
    };

    let mut existing: toml::Table = match existing_content.parse() {
        Ok(t) => t,
        Err(_) => {
            // Unparseable config — don't touch it.
            return FileResult::Skipped;
        }
    };

    let defaults = default_config_table();
    let added = merge_defaults(&mut existing, &defaults);

    if added.is_empty() {
        return FileResult::Skipped;
    }

    // Back up existing config.
    let backup = root.join(format!("{CONFIG_FILE}.backup"));
    if let Err(e) = std::fs::copy(&path, &backup) {
        ui::error(&format!("cannot back up {CONFIG_FILE}: {e}"));
        return FileResult::Error;
    }

    // Preserve the schema directive and write merged config.
    let schema_line =
        "#:schema https://driftsys.github.io/git-std/schemas/v1/git-std.schema.json\n\n";
    let merged = format!(
        "{schema_line}{}",
        toml::to_string_pretty(&existing).unwrap_or_default()
    );

    if let Err(e) = std::fs::write(&path, &merged) {
        ui::error(&format!("cannot write {CONFIG_FILE}: {e}"));
        return FileResult::Error;
    }

    for key in &added {
        ui::info(&format!("  added default: {key}"));
    }

    FileResult::Created
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

// ---------------------------------------------------------------------------
// Config merge helpers
// ---------------------------------------------------------------------------

/// Build a TOML table of default config values.
fn default_config_table() -> toml::Table {
    let mut t = toml::Table::new();
    t.insert("scheme".into(), toml::Value::String("semver".into()));
    t.insert("strict".into(), toml::Value::Boolean(false));
    t.insert(
        "types".into(),
        toml::Value::Array(
            [
                "feat", "fix", "docs", "style", "refactor", "perf", "test", "chore", "ci", "build",
                "revert",
            ]
            .iter()
            .map(|s| toml::Value::String((*s).to_string()))
            .collect(),
        ),
    );
    t.insert("scopes".into(), toml::Value::String("auto".into()));
    t
}

/// Deep-merge `defaults` into `existing`, adding only missing keys.
///
/// For table values, recurses into sub-tables. For all other types,
/// the existing value always wins. Returns the list of keys that were added.
fn merge_defaults(existing: &mut toml::Table, defaults: &toml::Table) -> Vec<String> {
    let mut added = Vec::new();
    for (key, default_val) in defaults {
        match existing.get_mut(key) {
            Some(toml::Value::Table(existing_sub)) => {
                if let toml::Value::Table(default_sub) = default_val {
                    let sub_added = merge_defaults(existing_sub, default_sub);
                    if !sub_added.is_empty() {
                        added.push(key.clone());
                    }
                }
            }
            Some(_) => {}
            None => {
                existing.insert(key.clone(), default_val.clone());
                added.push(key.clone());
            }
        }
    }
    added
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
        let s = include_str!("../../../../../skills/std-commit/SKILL.md");
        assert!(s.starts_with("---\nname: std-commit\n"));
        assert!(s.contains("git std --context"));
        assert!(s.contains("git std commit"));
    }

    #[test]
    fn std_commit_skill_includes_message_guidelines() {
        let s = include_str!("../../../../../skills/std-commit/SKILL.md");
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
        let s = include_str!("../../../../../skills/std-bump/SKILL.md");
        assert!(s.starts_with("---\nname: std-bump\n"));
        assert!(s.contains("git std bump --dry-run"));
        assert!(s.contains("--push"));
    }

    #[test]
    fn merge_adds_missing_keys() {
        let mut existing: toml::Table = r#"scheme = "semver""#.parse().unwrap();
        let defaults = default_config_table();
        let added = merge_defaults(&mut existing, &defaults);
        assert!(added.contains(&"strict".to_string()));
        assert!(added.contains(&"types".to_string()));
        assert!(!added.contains(&"scheme".to_string()));
        assert_eq!(
            existing["scheme"],
            toml::Value::String("semver".to_string())
        );
    }

    #[test]
    fn merge_preserves_user_values() {
        let mut existing: toml::Table = r#"
scheme = "calver"
types = ["feat", "fix"]
"#
        .parse()
        .unwrap();
        let defaults = default_config_table();
        merge_defaults(&mut existing, &defaults);
        assert_eq!(
            existing["scheme"],
            toml::Value::String("calver".to_string())
        );
        // User's shorter types list is preserved
        if let toml::Value::Array(arr) = &existing["types"] {
            assert_eq!(arr.len(), 2);
        } else {
            panic!("types should be an array");
        }
    }

    #[test]
    fn merge_no_changes_returns_empty() {
        let mut existing: toml::Table = r#"
scheme = "semver"
strict = false
types = ["feat"]
scopes = "auto"
"#
        .parse()
        .unwrap();
        let defaults = default_config_table();
        let added = merge_defaults(&mut existing, &defaults);
        assert!(added.is_empty());
    }

    #[test]
    fn write_config_merges_existing() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join(CONFIG_FILE);
        std::fs::write(&config_path, "scheme = \"calver\"\n").unwrap();

        let result = write_config_file(dir.path(), false);
        assert!(matches!(result, FileResult::Created));

        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("scheme = \"calver\""));
        assert!(content.contains("strict"));
        assert!(content.contains("types"));

        // Backup should exist
        assert!(dir.path().join(format!("{CONFIG_FILE}.backup")).exists());
    }
}
