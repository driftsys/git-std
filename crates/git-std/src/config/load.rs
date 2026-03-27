use std::path::Path;

use super::{
    ChangelogConfig, ProjectConfig, Scheme, ScopesConfig, VersionFileConfig, VersioningConfig,
};

/// Config filename.
const CONFIG_FILE: &str = ".git-std.toml";

/// Default conventional commit types used when `.git-std.toml` has no `types` list.
pub(crate) const DEFAULT_TYPES: &[&str] = &[
    "feat", "fix", "docs", "style", "refactor", "perf", "test", "chore", "ci", "build", "revert",
];

pub(crate) fn default_types() -> Vec<String> {
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

/// Load configuration along with the raw TOML table for source-tracking.
///
/// Returns `(config, raw_table)` where `raw_table` is `Some` when a
/// `.git-std.toml` file was found and successfully parsed.
pub(crate) fn load_with_raw(dir: &Path) -> (ProjectConfig, Option<toml::Table>) {
    let path = dir.join(CONFIG_FILE);
    match std::fs::read_to_string(&path) {
        Ok(content) => match content.parse::<toml::Table>() {
            Ok(table) => {
                let cfg = build_config(&table);
                (cfg, Some(table))
            }
            Err(e) => {
                eprintln!("warning: invalid .git-std.toml, using defaults: {e}");
                (default_config(), None)
            }
        },
        Err(_) => (default_config(), None),
    }
}

pub(crate) fn parse_config(content: &str) -> ProjectConfig {
    let table: toml::Table = match content.parse() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("warning: invalid .git-std.toml, using defaults: {e}");
            return default_config();
        }
    };
    build_config(&table)
}

fn default_config() -> ProjectConfig {
    ProjectConfig {
        types: default_types(),
        scopes: ScopesConfig::None,
        strict: false,
        scheme: Scheme::default(),
        changelog: ChangelogConfig::default(),
        versioning: VersioningConfig::default(),
        version_files: Vec::new(),
    }
}

fn build_config(table: &toml::Table) -> ProjectConfig {
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

    let changelog = parse_changelog_config(table);
    let versioning = parse_versioning_config(table);
    let version_files = parse_version_files(table);

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
