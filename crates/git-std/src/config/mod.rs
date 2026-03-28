use std::path::Path;

use serde::Serialize;

pub(crate) mod deps;
mod load;
mod workspace;

pub use load::load;
pub(crate) use load::load_with_raw;
pub(crate) use workspace::discover_packages;

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

/// Default tag template for per-package tags.
pub const DEFAULT_TAG_TEMPLATE: &str = "{name}@{version}";

/// Versioning configuration.
#[derive(Debug, Clone, Serialize)]
pub struct VersioningConfig {
    /// Tag prefix (default `"v"`).
    pub tag_prefix: String,
    /// Default pre-release tag (default `"rc"`).
    pub prerelease_tag: String,
    /// Calver format string (e.g. `"YYYY.MM.PATCH"`).
    pub calver_format: String,
    /// Tag template for per-package tags (default `"{name}@{version}"`).
    ///
    /// Supports `{name}` and `{version}` placeholders.
    pub tag_template: String,
}

impl Default for VersioningConfig {
    fn default() -> Self {
        Self {
            tag_prefix: "v".to_string(),
            prerelease_tag: "rc".to_string(),
            calver_format: standard_version::calver::DEFAULT_FORMAT.to_string(),
            tag_template: DEFAULT_TAG_TEMPLATE.to_string(),
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

/// Per-package configuration for monorepo workspaces.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PackageConfig {
    /// Package name (used in tags and changelog headings).
    pub name: String,
    /// Path to the package root, relative to the repository root.
    pub path: String,
    /// Override the global versioning scheme for this package.
    pub scheme: Option<Scheme>,
    /// Override version files for this package.
    pub version_files: Option<Vec<VersionFileConfig>>,
    /// Override changelog configuration for this package.
    pub changelog: Option<ChangelogConfig>,
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
    /// Enable per-package monorepo versioning.
    pub monorepo: bool,
    /// Explicit package definitions (auto-discovered when empty and `monorepo = true`).
    pub packages: Vec<PackageConfig>,
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
    /// When `monorepo = true`, package names and the project name are always
    /// appended to the scope list.
    ///
    /// When `packages` is `Some`, uses the provided list instead of
    /// re-discovering from disk — avoids redundant filesystem scans.
    pub fn resolved_scopes(
        &self,
        repo_root: &Path,
        packages: Option<&[PackageConfig]>,
    ) -> Vec<String> {
        let mut scopes = match &self.scopes {
            ScopesConfig::None if self.monorepo => discover_scopes(repo_root),
            ScopesConfig::None => return Vec::new(),
            ScopesConfig::Auto => discover_scopes(repo_root),
            ScopesConfig::List(list) => list.clone(),
        };

        if self.monorepo {
            let owned;
            let pkgs = match packages {
                Some(p) => p,
                None => {
                    owned = self.resolved_packages(repo_root);
                    &owned
                }
            };
            for pkg in pkgs {
                if !scopes.contains(&pkg.name) {
                    scopes.push(pkg.name.clone());
                }
            }
            scopes.sort();
            scopes.dedup();
        }

        scopes
    }

    /// Resolve the effective package list.
    ///
    /// Returns explicit `[[packages]]` if non-empty, otherwise auto-discovers
    /// from workspace manifests when `monorepo = true`.
    pub fn resolved_packages(&self, repo_root: &Path) -> Vec<PackageConfig> {
        if !self.packages.is_empty() {
            return self.packages.clone();
        }
        if self.monorepo {
            discover_packages(repo_root)
        } else {
            Vec::new()
        }
    }

    /// Build a `LintConfig` for `standard_commit::lint`.
    ///
    /// Strict mode is enabled if either the `--strict` CLI flag is passed
    /// or `strict = true` is set in `.git-std.toml`.
    ///
    /// When `scopes = "auto"`, scopes are discovered from the workspace
    /// directory layout under `repo_root`. When `monorepo = true`, package
    /// names are always included regardless of scope mode.
    pub fn to_lint_config(&self, strict: bool, repo_root: &Path) -> standard_commit::LintConfig {
        if self.strict || strict {
            let (scopes, require_scope) = if self.monorepo {
                let resolved = self.resolved_scopes(repo_root, None);
                if resolved.is_empty() {
                    (None, false)
                } else {
                    (Some(resolved), true)
                }
            } else {
                match &self.scopes {
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
                }
            };
            // `chore(release)` is the standard commit message produced by
            // `git std bump`. Always allow it so the tool's own commits
            // pass validation regardless of the configured scope list.
            let scopes = scopes.map(|mut s| {
                if !s.iter().any(|v| v == "release") {
                    s.push("release".to_string());
                }
                s
            });
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
