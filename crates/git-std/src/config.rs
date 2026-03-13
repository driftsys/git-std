use std::path::Path;

/// Default conventional commit types used when `.versionrc` has no `[[types]]`.
const DEFAULT_TYPES: &[&str] = &[
    "feat", "fix", "docs", "style", "refactor", "perf", "test", "chore", "ci", "build",
];

/// A commit type entry from `[[types]]` in `.versionrc`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TypeEntry {
    pub r#type: String,
    pub section: Option<String>,
    pub hidden: bool,
}

/// Project configuration loaded from `.versionrc`.
#[derive(Debug, Default)]
pub struct ProjectConfig {
    pub types: Vec<TypeEntry>,
    pub scopes: Option<Vec<String>>,
}

impl ProjectConfig {
    /// Extract just the type names for validation.
    pub fn type_names(&self) -> Vec<String> {
        self.types.iter().map(|t| t.r#type.clone()).collect()
    }

    /// Build a `LintConfig` for `standard_commit::lint`.
    pub fn to_lint_config(&self, strict: bool) -> standard_commit::LintConfig {
        if strict {
            standard_commit::LintConfig {
                types: Some(self.type_names()),
                scopes: self.scopes.clone(),
                require_scope: self.scopes.is_some(),
                ..Default::default()
            }
        } else {
            standard_commit::LintConfig::default()
        }
    }
}

fn default_type_entries() -> Vec<TypeEntry> {
    DEFAULT_TYPES
        .iter()
        .map(|t| TypeEntry {
            r#type: (*t).to_string(),
            section: None,
            hidden: false,
        })
        .collect()
}

/// Load configuration from `.versionrc` in the given directory, or return defaults.
pub fn load(dir: &Path) -> ProjectConfig {
    let path = dir.join(".versionrc");
    match std::fs::read_to_string(&path) {
        Ok(content) => parse_versionrc(&content),
        Err(_) => ProjectConfig {
            types: default_type_entries(),
            scopes: None,
        },
    }
}

fn parse_versionrc(content: &str) -> ProjectConfig {
    let table: toml::Table = match content.parse() {
        Ok(t) => t,
        Err(_) => {
            return ProjectConfig {
                types: default_type_entries(),
                scopes: None,
            };
        }
    };

    let types = match table.get("types").and_then(|v| v.as_array()) {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                let t = v.as_table()?;
                let type_name = t.get("type")?.as_str()?.to_string();
                let section = t.get("section").and_then(|s| s.as_str()).map(String::from);
                let hidden = t.get("hidden").and_then(|h| h.as_bool()).unwrap_or(false);
                Some(TypeEntry {
                    r#type: type_name,
                    section,
                    hidden,
                })
            })
            .collect(),
        None => default_type_entries(),
    };

    let scopes = table.get("scopes").and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()
    });

    ProjectConfig { types, scopes }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_types_when_no_versionrc() {
        let dir = tempfile::tempdir().unwrap();
        let config = load(dir.path());
        assert_eq!(config.types.len(), DEFAULT_TYPES.len());
        assert!(config.type_names().contains(&"feat".to_string()));
        assert!(config.scopes.is_none());
    }

    #[test]
    fn custom_types_from_versionrc() {
        let config = parse_versionrc(
            r#"
[[types]]
type = "feat"
section = "Features"

[[types]]
type = "fix"
section = "Bug Fixes"

[[types]]
type = "custom"
"#,
        );
        assert_eq!(config.type_names(), vec!["feat", "fix", "custom"]);
        assert_eq!(config.types[0].section.as_deref(), Some("Features"));
        assert_eq!(config.types[2].section, None);
    }

    #[test]
    fn hidden_types() {
        let config = parse_versionrc(
            r#"
[[types]]
type = "chore"
hidden = true

[[types]]
type = "feat"
section = "Features"
"#,
        );
        assert!(config.types[0].hidden);
        assert!(!config.types[1].hidden);
    }

    #[test]
    fn scopes_from_versionrc() {
        let config = parse_versionrc("scopes = [\"auth\", \"api\"]\n");
        assert_eq!(
            config.scopes,
            Some(vec!["auth".to_string(), "api".to_string()])
        );
    }

    #[test]
    fn no_scopes_means_none() {
        let config = parse_versionrc(
            r#"
[[types]]
type = "feat"
"#,
        );
        assert!(config.scopes.is_none());
    }

    #[test]
    fn invalid_toml_uses_defaults() {
        let config = parse_versionrc("not valid toml {{{{");
        assert_eq!(config.types.len(), DEFAULT_TYPES.len());
    }

    #[test]
    fn to_lint_config_not_strict() {
        let config = ProjectConfig {
            types: vec![TypeEntry {
                r#type: "feat".into(),
                section: None,
                hidden: false,
            }],
            scopes: Some(vec!["auth".into()]),
        };
        let lint = config.to_lint_config(false);
        assert!(lint.types.is_none());
        assert!(lint.scopes.is_none());
        assert!(!lint.require_scope);
    }

    #[test]
    fn to_lint_config_strict() {
        let config = ProjectConfig {
            types: vec![TypeEntry {
                r#type: "feat".into(),
                section: None,
                hidden: false,
            }],
            scopes: Some(vec!["auth".into()]),
        };
        let lint = config.to_lint_config(true);
        assert_eq!(lint.types, Some(vec!["feat".into()]));
        assert_eq!(lint.scopes, Some(vec!["auth".into()]));
        assert!(lint.require_scope);
    }

    #[test]
    fn to_lint_config_strict_no_scopes() {
        let config = ProjectConfig {
            types: vec![TypeEntry {
                r#type: "feat".into(),
                section: None,
                hidden: false,
            }],
            scopes: None,
        };
        let lint = config.to_lint_config(true);
        assert!(lint.scopes.is_none());
        assert!(!lint.require_scope);
    }
}
