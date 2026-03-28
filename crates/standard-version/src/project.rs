//! Project manifest version file engines.
//!
//! Supports `project.toml`, `project.json`, and `project.yaml` — the
//! driftsys project manifest format. Each file has a top-level `version`
//! field. No ecosystem tooling; always uses native string manipulation.

use std::sync::LazyLock;

use crate::version_file::{VersionFile, VersionFileError};

/// Regex matching `"version": "..."` in JSON (same as `json.rs`).
static JSON_VERSION_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r#""version"\s*:\s*"([^"]+)""#).expect("valid regex"));

// ---------------------------------------------------------------------------
// project.toml
// ---------------------------------------------------------------------------

/// Version file engine for `project.toml`.
///
/// Detects a top-level `version = "..."` (before any `[section]` header).
#[derive(Debug, Clone, Copy)]
pub struct ProjectTomlVersionFile;

impl VersionFile for ProjectTomlVersionFile {
    fn name(&self) -> &str {
        "project.toml"
    }

    fn filenames(&self) -> &[&str] {
        &["project.toml"]
    }

    fn detect(&self, content: &str) -> bool {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                return false;
            }
            if trimmed.starts_with("version") && trimmed.contains('=') {
                return true;
            }
        }
        false
    }

    fn read_version(&self, content: &str) -> Option<String> {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                return None;
            }
            if trimmed.starts_with("version")
                && let Some(eq_pos) = trimmed.find('=')
            {
                let value = trimmed[eq_pos + 1..].trim();
                return Some(value.trim_matches('"').to_string());
            }
        }
        None
    }

    fn write_version(&self, content: &str, new_version: &str) -> Result<String, VersionFileError> {
        let mut result = String::new();
        let mut replaced = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if !replaced
                && !trimmed.starts_with('[')
                && trimmed.starts_with("version")
                && let Some(eq_pos) = line.find('=')
            {
                let prefix = &line[..=eq_pos];
                result.push_str(prefix);
                result.push_str(&format!(" \"{new_version}\""));
                result.push('\n');
                replaced = true;
                continue;
            }
            result.push_str(line);
            result.push('\n');
        }

        if !replaced {
            return Err(VersionFileError::NoVersionField);
        }
        if !content.ends_with('\n') && result.ends_with('\n') {
            result.pop();
        }
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// project.json
// ---------------------------------------------------------------------------

/// Version file engine for `project.json`.
///
/// Uses regex matching to support both strict JSON and JSONC with comments.
#[derive(Debug, Clone, Copy)]
pub struct ProjectJsonVersionFile;

impl VersionFile for ProjectJsonVersionFile {
    fn name(&self) -> &str {
        "project.json"
    }

    fn filenames(&self) -> &[&str] {
        &["project.json"]
    }

    fn detect(&self, content: &str) -> bool {
        JSON_VERSION_RE.is_match(content)
    }

    fn read_version(&self, content: &str) -> Option<String> {
        JSON_VERSION_RE
            .captures(content)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }

    fn write_version(&self, content: &str, new_version: &str) -> Result<String, VersionFileError> {
        let re = &*JSON_VERSION_RE;
        if !re.is_match(content) {
            return Err(VersionFileError::NoVersionField);
        }
        let mut replaced = false;
        let result = re.replace(content, |caps: &regex::Captures<'_>| {
            if replaced {
                return caps[0].to_string();
            }
            replaced = true;
            let full = &caps[0];
            let version_start = caps.get(1).unwrap().start() - caps.get(0).unwrap().start();
            let version_end = caps.get(1).unwrap().end() - caps.get(0).unwrap().start();
            format!(
                "{}{}{}",
                &full[..version_start],
                new_version,
                &full[version_end..],
            )
        });
        Ok(result.into_owned())
    }
}

// ---------------------------------------------------------------------------
// project.yaml
// ---------------------------------------------------------------------------

/// Version file engine for `project.yaml`.
///
/// Detects a top-level `version:` field (not indented).
#[derive(Debug, Clone, Copy)]
pub struct ProjectYamlVersionFile;

impl VersionFile for ProjectYamlVersionFile {
    fn name(&self) -> &str {
        "project.yaml"
    }

    fn filenames(&self) -> &[&str] {
        &["project.yaml"]
    }

    fn detect(&self, content: &str) -> bool {
        content
            .lines()
            .any(|line| line.starts_with("version:") && line.len() > "version:".len())
    }

    fn read_version(&self, content: &str) -> Option<String> {
        for line in content.lines() {
            if let Some(value) = line.strip_prefix("version:") {
                let value = value.trim().trim_matches('"').trim_matches('\'');
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        None
    }

    fn write_version(&self, content: &str, new_version: &str) -> Result<String, VersionFileError> {
        let mut result = String::new();
        let mut replaced = false;

        for line in content.lines() {
            if !replaced && line.starts_with("version:") {
                result.push_str(&format!("version: \"{new_version}\""));
                result.push('\n');
                replaced = true;
                continue;
            }
            result.push_str(line);
            result.push('\n');
        }

        if !replaced {
            return Err(VersionFileError::NoVersionField);
        }
        if !content.ends_with('\n') && result.ends_with('\n') {
            result.pop();
        }
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // === project.toml ===

    const TOML: &str = r#"name = "io.driftsys.myapp"
description = "My application"
version = "0.1.0"
license = "MIT"
"#;

    const TOML_NO_VERSION: &str = "name = \"io.driftsys.myapp\"\n";

    const TOML_VERSION_IN_SECTION: &str = r#"name = "io.driftsys.myapp"

[metadata]
version = "0.1.0"
"#;

    #[test]
    fn toml_detect() {
        assert!(ProjectTomlVersionFile.detect(TOML));
    }

    #[test]
    fn toml_detect_no_version() {
        assert!(!ProjectTomlVersionFile.detect(TOML_NO_VERSION));
    }

    #[test]
    fn toml_detect_ignores_section() {
        assert!(!ProjectTomlVersionFile.detect(TOML_VERSION_IN_SECTION));
    }

    #[test]
    fn toml_read() {
        assert_eq!(
            ProjectTomlVersionFile.read_version(TOML),
            Some("0.1.0".to_string()),
        );
    }

    #[test]
    fn toml_write() {
        let result = ProjectTomlVersionFile.write_version(TOML, "2.0.0").unwrap();
        assert!(result.contains("version = \"2.0.0\""));
        assert!(result.contains("license = \"MIT\""));
    }

    #[test]
    fn toml_write_no_version_errors() {
        assert!(
            ProjectTomlVersionFile
                .write_version(TOML_NO_VERSION, "1.0.0")
                .is_err()
        );
    }

    // === project.json ===

    const JSON: &str = r#"{
  "name": "io.driftsys.myapp",
  "version": "0.1.0",
  "description": "My application"
}
"#;

    const JSON_NO_VERSION: &str = r#"{
  "name": "io.driftsys.myapp"
}
"#;

    #[test]
    fn json_detect() {
        assert!(ProjectJsonVersionFile.detect(JSON));
    }

    #[test]
    fn json_detect_no_version() {
        assert!(!ProjectJsonVersionFile.detect(JSON_NO_VERSION));
    }

    #[test]
    fn json_read() {
        assert_eq!(
            ProjectJsonVersionFile.read_version(JSON),
            Some("0.1.0".to_string()),
        );
    }

    #[test]
    fn json_write() {
        let result = ProjectJsonVersionFile.write_version(JSON, "2.0.0").unwrap();
        assert!(result.contains(r#""version": "2.0.0""#));
        assert!(result.contains(r#""name": "io.driftsys.myapp""#));
    }

    // === project.yaml ===

    const YAML: &str = "name: io.driftsys.myapp\nversion: \"0.1.0\"\nlicense: MIT\n";
    const YAML_UNQUOTED: &str = "name: io.driftsys.myapp\nversion: 0.1.0\nlicense: MIT\n";
    const YAML_NO_VERSION: &str = "name: io.driftsys.myapp\nlicense: MIT\n";

    #[test]
    fn yaml_detect() {
        assert!(ProjectYamlVersionFile.detect(YAML));
    }

    #[test]
    fn yaml_detect_unquoted() {
        assert!(ProjectYamlVersionFile.detect(YAML_UNQUOTED));
    }

    #[test]
    fn yaml_detect_no_version() {
        assert!(!ProjectYamlVersionFile.detect(YAML_NO_VERSION));
    }

    #[test]
    fn yaml_read_quoted() {
        assert_eq!(
            ProjectYamlVersionFile.read_version(YAML),
            Some("0.1.0".to_string()),
        );
    }

    #[test]
    fn yaml_read_unquoted() {
        assert_eq!(
            ProjectYamlVersionFile.read_version(YAML_UNQUOTED),
            Some("0.1.0".to_string()),
        );
    }

    #[test]
    fn yaml_write() {
        let result = ProjectYamlVersionFile.write_version(YAML, "2.0.0").unwrap();
        assert!(result.contains("version: \"2.0.0\""));
        assert!(result.contains("license: MIT"));
    }

    #[test]
    fn yaml_write_no_version_errors() {
        assert!(
            ProjectYamlVersionFile
                .write_version(YAML_NO_VERSION, "1.0.0")
                .is_err()
        );
    }
}
