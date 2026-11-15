use super::types::{refuse, require};
use crate::config::{self, ProjectConfig};
use anyhow::Result;

/// Validate release-relevant fields before using the existing tolerant loader.
pub(super) fn parse(content: &str, diagnostics: &mut Vec<String>) -> Result<ProjectConfig> {
    let table: toml::Table = content
        .parse()
        .map_err(|e| refuse(format!("invalid target configuration: {e}")))?;
    validate(&table, "root", diagnostics)?;
    Ok(config::parse_config(content))
}

fn validate(table: &toml::Table, kind: &str, diagnostics: &mut Vec<String>) -> Result<()> {
    for (key, value) in table {
        let is_valid = match (kind, key.as_str()) {
            ("root", "monorepo" | "strict") => value.is_bool(),
            (_, "scheme") => value
                .as_str()
                .is_some_and(|v| matches!(v, "semver" | "calver" | "patch")),
            ("root", "types" | "refs_required") | ("changelog", "hidden") => value
                .as_array()
                .is_some_and(|a| a.iter().all(toml::Value::is_str)),
            ("changelog", "sections") => value
                .as_table()
                .is_some_and(|t| t.values().all(toml::Value::is_str)),
            ("root", "scopes") => {
                value.as_str() == Some("auto")
                    || value
                        .as_array()
                        .is_some_and(|a| a.iter().all(toml::Value::is_str))
            }
            ("root", "versioning") | (_, "changelog") => {
                let t = value
                    .as_table()
                    .ok_or_else(|| refuse(format!("{key} must be a table")))?;
                validate(t, key, diagnostics)?;
                true
            }
            (_, "version_files") | ("root", "packages") => {
                let a = value
                    .as_array()
                    .ok_or_else(|| refuse(format!("{key} must be an array")))?;
                for item in a {
                    let t = item
                        .as_table()
                        .ok_or_else(|| refuse(format!("{key} entry must be a table")))?;
                    validate(t, key, diagnostics)?;
                    for required in if key == "packages" {
                        ["name", "path"]
                    } else {
                        ["path", "regex"]
                    } {
                        require(
                            t.get(required).is_some_and(toml::Value::is_str),
                            format!("{key} requires {required}"),
                        )?;
                    }
                }
                true
            }
            ("root", "release_branch" | "default_scope")
            | ("versioning", "tag_prefix" | "tag_template" | "prerelease_tag" | "calver_format")
            | ("version_files", "path" | "regex")
            | ("packages", "name" | "path")
            | ("changelog", "title" | "bug_url") => value.is_str(),
            _ => {
                diagnostics.push(format!("unknown configuration key: {kind}.{key}"));
                true
            }
        };
        require(
            is_valid,
            format!("invalid type or value for configuration {kind}.{key}"),
        )?;
    }
    Ok(())
}
