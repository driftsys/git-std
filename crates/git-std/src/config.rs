use std::path::Path;

/// Config filename.
const CONFIG_FILE: &str = ".git-std.toml";

/// Directory patterns scanned for auto-discovered scopes.
const SCOPE_DIRS: &[&str] = &["crates", "packages", "modules"];

/// Default conventional commit types used when `.git-std.toml` has no `types` list.
const DEFAULT_TYPES: &[&str] = &[
    "feat", "fix", "docs", "style", "refactor", "perf", "test", "chore", "ci", "build",
];

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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

/// Versioning configuration.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone, Default)]
pub struct ChangelogConfig {
    pub title: Option<String>,
    pub sections: Option<Vec<(String, String)>>,
    pub hidden: Option<Vec<String>>,
    pub bug_url: Option<String>,
}

/// A user-defined version file entry from `[[version_files]]`.
#[derive(Debug, Clone, Default)]
pub struct VersionFileConfig {
    /// Path to the file, relative to the repository root.
    pub path: String,
    /// Regex pattern whose first capture group contains the version string.
    pub regex: String,
}

/// Project configuration loaded from `.git-std.toml`.
#[derive(Debug, Default)]
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

fn default_types() -> Vec<String> {
    DEFAULT_TYPES.iter().map(|t| (*t).to_string()).collect()
}

/// Load configuration from `.git-std.toml` in the given directory, or return defaults.
pub fn load(dir: &Path) -> ProjectConfig {
    let path = dir.join(CONFIG_FILE);
    match std::fs::read_to_string(&path) {
        Ok(content) => parse_config(&content),
        Err(_) => ProjectConfig {
            types: default_types(),
            scopes: ScopesConfig::None,
            strict: false,
            scheme: Scheme::default(),
            changelog: ChangelogConfig::default(),
            versioning: VersioningConfig::default(),
            version_files: Vec::new(),
        },
    }
}

fn parse_config(content: &str) -> ProjectConfig {
    let table: toml::Table = match content.parse() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("warning: invalid .git-std.toml, using defaults: {e}");
            return ProjectConfig {
                types: default_types(),
                scopes: ScopesConfig::None,
                strict: false,
                scheme: Scheme::default(),
                changelog: ChangelogConfig::default(),
                versioning: VersioningConfig::default(),
                version_files: Vec::new(),
            };
        }
    };

    let types = match table.get("types").and_then(|v| v.as_array()) {
        Some(arr) => {
            let parsed: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            if parsed.is_empty() {
                default_types()
            } else {
                parsed
            }
        }
        None => default_types(),
    };

    let scopes = match table.get("scopes") {
        Some(toml::Value::String(s)) if s == "auto" => ScopesConfig::Auto,
        Some(toml::Value::Array(arr)) => {
            let list: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            if list.is_empty() {
                ScopesConfig::None
            } else {
                ScopesConfig::List(list)
            }
        }
        _ => ScopesConfig::None,
    };

    let strict = table
        .get("strict")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let scheme = match table.get("scheme").and_then(|v| v.as_str()) {
        Some("calver") => Scheme::Calver,
        Some("patch") => Scheme::Patch,
        _ => Scheme::Semver,
    };

    let changelog = parse_changelog_config(&table);
    let versioning = parse_versioning_config(&table);
    let version_files = parse_version_files(&table);

    // Validate calver_format when scheme is calver.
    let versioning = if scheme == Scheme::Calver {
        if let Err(e) = standard_version::calver::validate_format(&versioning.calver_format) {
            eprintln!(
                "warning: invalid calver_format '{}': {e} — using default",
                versioning.calver_format
            );
            VersioningConfig {
                calver_format: standard_version::calver::DEFAULT_FORMAT.to_string(),
                ..versioning
            }
        } else {
            versioning
        }
    } else {
        versioning
    };

    ProjectConfig {
        types,
        scopes,
        strict,
        scheme,
        changelog,
        versioning,
        version_files,
    }
}

fn parse_versioning_config(table: &toml::Table) -> VersioningConfig {
    let versioning_table = match table.get("versioning").and_then(|v| v.as_table()) {
        Some(t) => t,
        None => return VersioningConfig::default(),
    };

    let defaults = VersioningConfig::default();

    let tag_prefix = versioning_table
        .get("tag_prefix")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or(defaults.tag_prefix);

    let prerelease_tag = versioning_table
        .get("prerelease_tag")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or(defaults.prerelease_tag);

    let calver_format = versioning_table
        .get("calver_format")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or(defaults.calver_format);

    VersioningConfig {
        tag_prefix,
        prerelease_tag,
        calver_format,
    }
}

fn parse_version_files(table: &toml::Table) -> Vec<VersionFileConfig> {
    let Some(arr) = table.get("version_files").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    arr.iter()
        .filter_map(|entry| {
            let t = entry.as_table()?;
            let path = t.get("path")?.as_str()?.to_string();
            let regex = t.get("regex")?.as_str()?.to_string();
            Some(VersionFileConfig { path, regex })
        })
        .collect()
}

fn parse_changelog_config(table: &toml::Table) -> ChangelogConfig {
    let changelog_table = match table.get("changelog").and_then(|v| v.as_table()) {
        Some(t) => t,
        None => return ChangelogConfig::default(),
    };

    let title = changelog_table
        .get("title")
        .and_then(|v| v.as_str())
        .map(String::from);

    let hidden = changelog_table
        .get("hidden")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

    let bug_url = changelog_table
        .get("bug_url")
        .and_then(|v| v.as_str())
        .map(String::from);

    let sections = changelog_table
        .get("sections")
        .and_then(|v| v.as_table())
        .map(|t| {
            t.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        });

    ChangelogConfig {
        title,
        sections,
        hidden,
        bug_url,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_types_when_no_config() {
        let dir = tempfile::tempdir().unwrap();
        let config = load(dir.path());
        assert_eq!(config.types.len(), DEFAULT_TYPES.len());
        assert!(config.types.contains(&"feat".to_string()));
        assert_eq!(config.scopes, ScopesConfig::None);
    }

    #[test]
    fn custom_types() {
        let config = parse_config(r#"types = ["feat", "fix", "custom"]"#);
        assert_eq!(config.types, vec!["feat", "fix", "custom"]);
    }

    #[test]
    fn scopes_explicit_list() {
        let config = parse_config("scopes = [\"auth\", \"api\"]\n");
        assert_eq!(
            config.scopes,
            ScopesConfig::List(vec!["auth".to_string(), "api".to_string()])
        );
    }

    #[test]
    fn scopes_auto() {
        let config = parse_config("scopes = \"auto\"\n");
        assert_eq!(config.scopes, ScopesConfig::Auto);
    }

    #[test]
    fn no_scopes_means_none() {
        let config = parse_config(r#"types = ["feat"]"#);
        assert_eq!(config.scopes, ScopesConfig::None);
    }

    #[test]
    fn invalid_toml_uses_defaults() {
        let config = parse_config("not valid toml {{{{");
        assert_eq!(config.types.len(), DEFAULT_TYPES.len());
    }

    #[test]
    fn to_lint_config_not_strict() {
        let dir = tempfile::tempdir().unwrap();
        let config = ProjectConfig {
            types: vec!["feat".into()],
            scopes: ScopesConfig::List(vec!["auth".into()]),
            ..Default::default()
        };
        let lint = config.to_lint_config(false, dir.path());
        assert!(lint.types.is_none());
        assert!(lint.scopes.is_none());
        assert!(!lint.require_scope);
    }

    #[test]
    fn to_lint_config_strict() {
        let dir = tempfile::tempdir().unwrap();
        let config = ProjectConfig {
            types: vec!["feat".into()],
            scopes: ScopesConfig::List(vec!["auth".into()]),
            ..Default::default()
        };
        let lint = config.to_lint_config(true, dir.path());
        assert_eq!(lint.types, Some(vec!["feat".into()]));
        assert_eq!(lint.scopes, Some(vec!["auth".into()]));
        assert!(lint.require_scope);
    }

    #[test]
    fn to_lint_config_strict_no_scopes() {
        let dir = tempfile::tempdir().unwrap();
        let config = ProjectConfig {
            types: vec!["feat".into()],
            scopes: ScopesConfig::None,
            ..Default::default()
        };
        let lint = config.to_lint_config(true, dir.path());
        assert!(lint.scopes.is_none());
        assert!(!lint.require_scope);
    }

    #[test]
    fn strict_from_config() {
        let config = parse_config("strict = true\n");
        assert!(config.strict);
    }

    #[test]
    fn strict_default_false() {
        let config = parse_config(r#"types = ["feat"]"#);
        assert!(!config.strict);
    }

    #[test]
    fn to_lint_config_strict_from_config() {
        let dir = tempfile::tempdir().unwrap();
        let config = ProjectConfig {
            types: vec!["feat".into()],
            scopes: ScopesConfig::List(vec!["auth".into()]),
            strict: true,
            ..Default::default()
        };
        // strict=true in config, flag=false → still strict
        let lint = config.to_lint_config(false, dir.path());
        assert_eq!(lint.types, Some(vec!["feat".into()]));
        assert_eq!(lint.scopes, Some(vec!["auth".into()]));
        assert!(lint.require_scope);
    }

    #[test]
    fn version_files_parsed() {
        let config = parse_config(
            r#"
[[version_files]]
path = "pom.xml"
regex = '<version>([^<]+)</version>'

[[version_files]]
path = "Chart.yaml"
regex = 'version:\s*(.+)'
"#,
        );
        assert_eq!(config.version_files.len(), 2);
        assert_eq!(config.version_files[0].path, "pom.xml");
        assert_eq!(config.version_files[0].regex, "<version>([^<]+)</version>");
        assert_eq!(config.version_files[1].path, "Chart.yaml");
    }

    #[test]
    fn version_files_default_empty() {
        let config = parse_config(r#"types = ["feat"]"#);
        assert!(config.version_files.is_empty());
    }

    #[test]
    fn scheme_defaults_to_semver() {
        let config = parse_config(r#"types = ["feat"]"#);
        assert_eq!(config.scheme, Scheme::Semver);
    }

    #[test]
    fn scheme_calver_parsed() {
        let config = parse_config("scheme = \"calver\"\n");
        assert_eq!(config.scheme, Scheme::Calver);
    }

    #[test]
    fn scheme_patch_parsed() {
        let config = parse_config("scheme = \"patch\"\n");
        assert_eq!(config.scheme, Scheme::Patch);
    }

    #[test]
    fn scheme_unknown_falls_back_to_semver() {
        let config = parse_config("scheme = \"unknown\"\n");
        assert_eq!(config.scheme, Scheme::Semver);
    }

    #[test]
    fn discover_scopes_from_crates() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("crates/auth")).unwrap();
        std::fs::create_dir_all(dir.path().join("crates/api")).unwrap();
        let scopes = discover_scopes(dir.path());
        assert_eq!(scopes, vec!["api", "auth"]);
    }

    #[test]
    fn discover_scopes_from_packages_and_modules() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("packages/ui")).unwrap();
        std::fs::create_dir_all(dir.path().join("modules/core")).unwrap();
        let scopes = discover_scopes(dir.path());
        assert_eq!(scopes, vec!["core", "ui"]);
    }

    #[test]
    fn discover_scopes_deduplicates() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("crates/shared")).unwrap();
        std::fs::create_dir_all(dir.path().join("packages/shared")).unwrap();
        let scopes = discover_scopes(dir.path());
        assert_eq!(scopes, vec!["shared"]);
    }

    #[test]
    fn discover_scopes_ignores_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("crates/auth")).unwrap();
        std::fs::write(dir.path().join("crates/README.md"), "hi").unwrap();
        let scopes = discover_scopes(dir.path());
        assert_eq!(scopes, vec!["auth"]);
    }

    #[test]
    fn discover_scopes_empty_when_no_matching_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let scopes = discover_scopes(dir.path());
        assert!(scopes.is_empty());
    }

    #[test]
    fn discover_scopes_non_standard_names() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("crates/my-crate_v2")).unwrap();
        std::fs::create_dir_all(dir.path().join("crates/123")).unwrap();
        let scopes = discover_scopes(dir.path());
        assert_eq!(scopes, vec!["123", "my-crate_v2"]);
    }

    #[test]
    fn to_lint_config_auto_discovers_scopes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("crates/auth")).unwrap();
        std::fs::create_dir_all(dir.path().join("crates/api")).unwrap();
        let config = ProjectConfig {
            types: vec!["feat".into()],
            scopes: ScopesConfig::Auto,
            ..Default::default()
        };
        let lint = config.to_lint_config(true, dir.path());
        assert_eq!(lint.scopes, Some(vec!["api".into(), "auth".into()]));
        assert!(lint.require_scope);
    }

    #[test]
    fn to_lint_config_auto_empty_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let config = ProjectConfig {
            types: vec!["feat".into()],
            scopes: ScopesConfig::Auto,
            ..Default::default()
        };
        let lint = config.to_lint_config(true, dir.path());
        assert!(lint.scopes.is_none());
        assert!(!lint.require_scope);
    }

    #[test]
    fn resolved_scopes_auto() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("packages/web")).unwrap();
        let config = ProjectConfig {
            scopes: ScopesConfig::Auto,
            ..Default::default()
        };
        assert_eq!(config.resolved_scopes(dir.path()), vec!["web"]);
    }

    #[test]
    fn resolved_scopes_list() {
        let dir = tempfile::tempdir().unwrap();
        let config = ProjectConfig {
            scopes: ScopesConfig::List(vec!["auth".into()]),
            ..Default::default()
        };
        assert_eq!(config.resolved_scopes(dir.path()), vec!["auth"]);
    }

    #[test]
    fn resolved_scopes_none() {
        let dir = tempfile::tempdir().unwrap();
        let config = ProjectConfig {
            scopes: ScopesConfig::None,
            ..Default::default()
        };
        assert!(config.resolved_scopes(dir.path()).is_empty());
    }

    #[test]
    fn calver_format_default() {
        let config = parse_config(r#"types = ["feat"]"#);
        assert_eq!(
            config.versioning.calver_format,
            standard_version::calver::DEFAULT_FORMAT
        );
    }

    #[test]
    fn calver_format_custom() {
        let config = parse_config(
            r#"
[versioning]
calver_format = "YYYY.0M.PATCH"
"#,
        );
        assert_eq!(config.versioning.calver_format, "YYYY.0M.PATCH");
    }

    #[test]
    fn calver_format_valid_no_fallback() {
        let config = parse_config(
            r#"
scheme = "calver"

[versioning]
calver_format = "YYYY.0M.PATCH"
"#,
        );
        assert_eq!(config.scheme, Scheme::Calver);
        assert_eq!(config.versioning.calver_format, "YYYY.0M.PATCH");
    }

    #[test]
    fn calver_format_invalid_falls_back_to_default() {
        let config = parse_config(
            r#"
scheme = "calver"

[versioning]
calver_format = "YYYY.INVALID"
"#,
        );
        assert_eq!(config.scheme, Scheme::Calver);
        assert_eq!(
            config.versioning.calver_format,
            standard_version::calver::DEFAULT_FORMAT
        );
    }

    #[test]
    fn calver_format_yy_0m_patch() {
        let fmt = "YY.0M.PATCH";
        assert!(standard_version::calver::validate_format(fmt).is_ok());
        let config = parse_config(&format!("[versioning]\ncalver_format = \"{fmt}\"\n"));
        assert_eq!(config.versioning.calver_format, fmt);
    }

    #[test]
    fn calver_format_yyyy_ww_patch() {
        let fmt = "YYYY.WW.PATCH";
        assert!(standard_version::calver::validate_format(fmt).is_ok());
        let config = parse_config(&format!("[versioning]\ncalver_format = \"{fmt}\"\n"));
        assert_eq!(config.versioning.calver_format, fmt);
    }

    #[test]
    fn calver_tokens_yy() {
        assert!(standard_version::calver::validate_format("YY.PATCH").is_ok());
    }

    #[test]
    fn calver_tokens_0m() {
        assert!(standard_version::calver::validate_format("YYYY.0M.PATCH").is_ok());
    }

    #[test]
    fn calver_tokens_dd() {
        assert!(standard_version::calver::validate_format("YYYY.MM.DD.PATCH").is_ok());
    }

    #[test]
    fn calver_tokens_ww() {
        assert!(standard_version::calver::validate_format("YY.WW.PATCH").is_ok());
    }

    #[test]
    fn version_files_with_regex_pattern() {
        let config = parse_config(
            r#"
[[version_files]]
path = "build.gradle"
regex = 'version\s*=\s*"([^"]+)"'

[[version_files]]
path = "setup.py"
regex = 'version="([^"]+)"'
"#,
        );
        assert_eq!(config.version_files.len(), 2);
        assert_eq!(config.version_files[0].path, "build.gradle");
        assert!(config.version_files[0].regex.contains("version"));
        assert_eq!(config.version_files[1].path, "setup.py");
    }

    #[test]
    fn version_files_missing_path_skipped() {
        let config = parse_config(
            r#"
[[version_files]]
regex = 'version="([^"]+)"'
"#,
        );
        assert!(config.version_files.is_empty());
    }

    #[test]
    fn changelog_hidden_types() {
        let config = parse_config(
            r#"
[changelog]
hidden = ["chore", "ci", "test"]
"#,
        );
        assert_eq!(
            config.changelog.hidden,
            Some(vec![
                "chore".to_string(),
                "ci".to_string(),
                "test".to_string()
            ])
        );
    }

    #[test]
    fn changelog_hidden_default_none() {
        let config = parse_config(r#"types = ["feat"]"#);
        assert!(config.changelog.hidden.is_none());
    }

    #[test]
    fn empty_toml_uses_defaults() {
        let config = parse_config("");
        assert_eq!(config.types.len(), DEFAULT_TYPES.len());
        assert_eq!(config.scopes, ScopesConfig::None);
        assert!(!config.strict);
        assert_eq!(config.scheme, Scheme::Semver);
        assert!(config.version_files.is_empty());
        assert!(config.changelog.hidden.is_none());
    }

    #[test]
    fn malformed_toml_warns_and_uses_defaults() {
        let config = parse_config("{{invalid toml content!!");
        assert_eq!(config.types.len(), DEFAULT_TYPES.len());
        assert_eq!(config.scheme, Scheme::Semver);
        assert!(!config.strict);
    }

    #[test]
    fn scheme_semver_explicit() {
        let config = parse_config("scheme = \"semver\"\n");
        assert_eq!(config.scheme, Scheme::Semver);
    }

    #[test]
    fn non_calver_scheme_ignores_invalid_format() {
        let config = parse_config(
            r#"
scheme = "semver"

[versioning]
calver_format = "YYYY.INVALID"
"#,
        );
        assert_eq!(config.scheme, Scheme::Semver);
        // Invalid format is kept as-is because scheme is not calver.
        assert_eq!(config.versioning.calver_format, "YYYY.INVALID");
    }
}
