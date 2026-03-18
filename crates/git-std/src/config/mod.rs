use std::path::Path;

use serde::Serialize;

mod load;

pub use load::load;
pub(crate) use load::load_with_raw;

#[cfg(test)]
mod tests;

/// Directory patterns scanned for auto-discovered scopes.
const SCOPE_DIRS: &[&str] = &["crates", "packages", "modules"];

/// Discover scope names from workspace directory layout.
///
/// Scans `crates/*/`, `packages/*/`, `modules/*/` under `repo_root` and
/// returns the sorted, deduplicated directory names.
pub fn discover_scopes(repo_root: &Path) -> Vec<String> {
    let mut scopes = Vec::new();
    for dir in SCOPE_DIRS {
        let parent = repo_root.join(dir);
        if let Ok(entries) = std::fs::read_dir(&parent) {
            for entry in entries.flatten() {
                if entry.path().is_dir()
                    && let Some(name) = entry.file_name().to_str()
                {
                    scopes.push(name.to_string());
                }
            }
        }
    }
    scopes.sort();
    scopes.dedup();
    scopes
}

/// Versioning scheme.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Scheme {
    /// Semantic versioning (default).
    #[default]
    Semver,
    /// Calendar versioning.
    Calver,
    /// Patch-only bumps (always increment patch, reject breaking without --force).
    Patch,
}

/// How scopes are resolved.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum ScopesConfig {
    /// No scope validation — any scope (or none) is accepted.
    #[default]
    None,
    /// Auto-discover scopes from workspace layout (`crates/*`, `packages/*`, `modules/*`).
    Auto,
    /// Explicit allowlist of scopes.
    List(Vec<String>),
}

impl Serialize for ScopesConfig {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            ScopesConfig::None => serializer.serialize_none(),
            ScopesConfig::Auto => serializer.serialize_str("auto"),
            ScopesConfig::List(list) => list.serialize(serializer),
        }
    }
}

/// Versioning configuration.
#[derive(Debug, Clone, Serialize)]
pub struct VersioningConfig {
    /// Tag prefix (default `"v"`).
    pub tag_prefix: String,
    /// Default pre-release tag (default `"rc"`).
    pub prerelease_tag: String,
    /// Calver format string (e.g. `"YYYY.MM.PATCH"`).
    pub calver_format: String,
}

impl Default for VersioningConfig {
    fn default() -> Self {
        Self {
            tag_prefix: "v".to_string(),
            prerelease_tag: "rc".to_string(),
            calver_format: standard_version::calver::DEFAULT_FORMAT.to_string(),
        }
    }
}

/// Changelog-specific configuration.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ChangelogConfig {
    pub title: Option<String>,
    pub sections: Option<Vec<(String, String)>>,
    pub hidden: Option<Vec<String>>,
    pub bug_url: Option<String>,
}

/// A user-defined version file entry from `[[version_files]]`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct VersionFileConfig {
    /// Path to the file, relative to the repository root.
    pub path: String,
    /// Regex pattern whose first capture group contains the version string.
    pub regex: String,
}

/// Project configuration loaded from `.git-std.toml`.
#[derive(Debug, Default, Serialize)]
pub struct ProjectConfig {
    pub types: Vec<String>,
    pub scopes: ScopesConfig,
    pub strict: bool,
    pub scheme: Scheme,
    pub changelog: ChangelogConfig,
    pub versioning: VersioningConfig,
    pub version_files: Vec<VersionFileConfig>,
}

impl ProjectConfig {
    /// Build a [`standard_changelog::ChangelogConfig`] from project settings.
    pub fn to_changelog_config(&self) -> standard_changelog::ChangelogConfig {
        let default = standard_changelog::ChangelogConfig::default();
        standard_changelog::ChangelogConfig {
            title: self.changelog.title.clone().unwrap_or(default.title),
            sections: self.changelog.sections.clone().unwrap_or(default.sections),
            hidden: self.changelog.hidden.clone().unwrap_or(default.hidden),
            bug_url: self.changelog.bug_url.clone(),
        }
    }

    /// Resolve the effective scope list.
    ///
    /// Returns the explicit list, auto-discovered names, or an empty vec.
    pub fn resolved_scopes(&self, repo_root: &Path) -> Vec<String> {
        match &self.scopes {
            ScopesConfig::None => Vec::new(),
            ScopesConfig::Auto => discover_scopes(repo_root),
            ScopesConfig::List(list) => list.clone(),
        }
    }

    /// Build a `LintConfig` for `standard_commit::lint`.
    ///
    /// Strict mode is enabled if either the `--strict` CLI flag is passed
    /// or `strict = true` is set in `.git-std.toml`.
    ///
    /// When `scopes = "auto"`, scopes are discovered from the workspace
    /// directory layout under `repo_root`.
    pub fn to_lint_config(&self, strict: bool, repo_root: &Path) -> standard_commit::LintConfig {
        if self.strict || strict {
            let (scopes, require_scope) = match &self.scopes {
                ScopesConfig::None => (None, false),
                ScopesConfig::Auto => {
                    let discovered = discover_scopes(repo_root);
                    if discovered.is_empty() {
                        (None, false)
                    } else {
                        (Some(discovered), true)
                    }
                }
                ScopesConfig::List(list) => (Some(list.clone()), true),
            };
            standard_commit::LintConfig {
                types: Some(self.types.clone()),
                scopes,
                require_scope,
                ..Default::default()
            }
        } else {
            standard_commit::LintConfig::default()
        }
    }
}
