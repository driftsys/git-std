//! JSON version file engines for `package.json` and `deno.json`/`deno.jsonc`.
//!
//! Provides [`JsonVersionFile`] for npm/Node `package.json` files (parsed via
//! `serde_json`) and [`DenoVersionFile`] for Deno manifests (handled with
//! line-level regex to support JSONC comments).

use crate::version_file::{VersionFile, VersionFileError};

// ---------------------------------------------------------------------------
// JsonVersionFile (package.json)
// ---------------------------------------------------------------------------

/// Version file engine for `package.json`.
///
/// Uses `serde_json` to parse and rewrite the file, producing consistent
/// 2-space-indented output with a trailing newline.
#[derive(Debug, Clone, Copy)]
pub struct JsonVersionFile;

impl VersionFile for JsonVersionFile {
    fn name(&self) -> &str {
        "package.json"
    }

    fn filenames(&self) -> &[&str] {
        &["package.json"]
    }

    fn detect(&self, content: &str) -> bool {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(content) else {
            return false;
        };
        value.get("version").and_then(|v| v.as_str()).is_some()
    }

    fn read_version(&self, content: &str) -> Option<String> {
        let value: serde_json::Value = serde_json::from_str(content).ok()?;
        value
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    fn write_version(&self, content: &str, new_version: &str) -> Result<String, VersionFileError> {
        let mut value: serde_json::Value =
            serde_json::from_str(content).map_err(|_| VersionFileError::NoVersionField)?;

        let obj = value
            .as_object_mut()
            .ok_or(VersionFileError::NoVersionField)?;

        if !obj.contains_key("version") {
            return Err(VersionFileError::NoVersionField);
        }

        obj.insert(
            "version".to_string(),
            serde_json::Value::String(new_version.to_string()),
        );

        let mut serialized =
            serde_json::to_string_pretty(&value).map_err(|_| VersionFileError::NoVersionField)?;
        serialized.push('\n');
        Ok(serialized)
    }
}

// ---------------------------------------------------------------------------
// DenoVersionFile (deno.json / deno.jsonc)
// ---------------------------------------------------------------------------

/// Version file engine for `deno.json` and `deno.jsonc`.
///
/// Uses line-level matching to find and replace the `"version"` field so that
/// JSONC comments are preserved. The regex matches the first occurrence of
/// `"version": "..."` in the file content.
#[derive(Debug, Clone, Copy)]
pub struct DenoVersionFile;

/// Regex pattern matching a JSON `"version"` field value.
///
/// Captures the version string in group 1.
const VERSION_PATTERN: &str = r#""version"\s*:\s*"([^"]+)""#;

impl DenoVersionFile {
    /// Compile the version-matching regex (infallible for a known-good pattern).
    fn regex() -> regex::Regex {
        regex::Regex::new(VERSION_PATTERN).expect("valid regex")
    }
}

impl VersionFile for DenoVersionFile {
    fn name(&self) -> &str {
        "deno.json"
    }

    fn filenames(&self) -> &[&str] {
        &["deno.json", "deno.jsonc"]
    }

    fn detect(&self, content: &str) -> bool {
        Self::regex().is_match(content)
    }

    fn read_version(&self, content: &str) -> Option<String> {
        let re = Self::regex();
        re.captures(content)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }

    fn write_version(&self, content: &str, new_version: &str) -> Result<String, VersionFileError> {
        let re = Self::regex();
        if !re.is_match(content) {
            return Err(VersionFileError::NoVersionField);
        }

        // Replace only the first occurrence.
        let mut replaced = false;
        let result = re.replace(content, |caps: &regex::Captures<'_>| {
            if replaced {
                // Return the original match unchanged for subsequent occurrences.
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // =======================================================================
    // JsonVersionFile
    // =======================================================================

    const PACKAGE_JSON: &str = r#"{
  "name": "my-app",
  "version": "1.2.3",
  "description": "An example package"
}
"#;

    const PACKAGE_JSON_NO_VERSION: &str = r#"{
  "name": "my-app",
  "description": "No version here"
}
"#;

    // --- detect ---

    #[test]
    fn json_detect_with_version() {
        assert!(JsonVersionFile.detect(PACKAGE_JSON));
    }

    #[test]
    fn json_detect_without_version() {
        assert!(!JsonVersionFile.detect(PACKAGE_JSON_NO_VERSION));
    }

    #[test]
    fn json_detect_invalid_json() {
        assert!(!JsonVersionFile.detect("not json at all"));
    }

    // --- read_version ---

    #[test]
    fn json_read_version() {
        assert_eq!(
            JsonVersionFile.read_version(PACKAGE_JSON),
            Some("1.2.3".to_string()),
        );
    }

    #[test]
    fn json_read_version_missing() {
        assert_eq!(JsonVersionFile.read_version(PACKAGE_JSON_NO_VERSION), None);
    }

    // --- write_version ---

    #[test]
    fn json_write_version_updates_value() {
        let result = JsonVersionFile
            .write_version(PACKAGE_JSON, "2.0.0")
            .unwrap();
        assert!(result.contains(r#""version": "2.0.0""#));
    }

    #[test]
    fn json_write_version_preserves_other_fields() {
        let result = JsonVersionFile
            .write_version(PACKAGE_JSON, "2.0.0")
            .unwrap();
        assert!(result.contains(r#""name": "my-app""#));
        assert!(result.contains(r#""description": "An example package""#));
    }

    #[test]
    fn json_write_version_trailing_newline() {
        let result = JsonVersionFile
            .write_version(PACKAGE_JSON, "2.0.0")
            .unwrap();
        assert!(result.ends_with('\n'));
    }

    #[test]
    fn json_write_version_no_field_returns_error() {
        let err = JsonVersionFile.write_version(PACKAGE_JSON_NO_VERSION, "1.0.0");
        assert!(err.is_err());
    }

    // =======================================================================
    // DenoVersionFile
    // =======================================================================

    const DENO_JSON: &str = r#"{
  "version": "0.5.0",
  "tasks": {
    "dev": "deno run --watch main.ts"
  }
}
"#;

    const DENO_JSONC: &str = r#"{
  // The current release version.
  "version": "0.5.0",
  "tasks": {
    "dev": "deno run --watch main.ts"
  }
}
"#;

    const DENO_NO_VERSION: &str = r#"{
  "tasks": {
    "dev": "deno run --watch main.ts"
  }
}
"#;

    // --- detect ---

    #[test]
    fn deno_detect_json() {
        assert!(DenoVersionFile.detect(DENO_JSON));
    }

    #[test]
    fn deno_detect_jsonc() {
        assert!(DenoVersionFile.detect(DENO_JSONC));
    }

    #[test]
    fn deno_detect_no_version() {
        assert!(!DenoVersionFile.detect(DENO_NO_VERSION));
    }

    // --- read_version ---

    #[test]
    fn deno_read_version_json() {
        assert_eq!(
            DenoVersionFile.read_version(DENO_JSON),
            Some("0.5.0".to_string()),
        );
    }

    #[test]
    fn deno_read_version_jsonc() {
        assert_eq!(
            DenoVersionFile.read_version(DENO_JSONC),
            Some("0.5.0".to_string()),
        );
    }

    #[test]
    fn deno_read_version_missing() {
        assert_eq!(DenoVersionFile.read_version(DENO_NO_VERSION), None);
    }

    // --- write_version ---

    #[test]
    fn deno_write_version_json() {
        let result = DenoVersionFile.write_version(DENO_JSON, "1.0.0").unwrap();
        assert!(result.contains(r#""version": "1.0.0""#));
        // Other content preserved.
        assert!(result.contains("tasks"));
    }

    #[test]
    fn deno_write_version_jsonc_preserves_comments() {
        let result = DenoVersionFile.write_version(DENO_JSONC, "1.0.0").unwrap();
        assert!(result.contains(r#""version": "1.0.0""#));
        assert!(result.contains("// The current release version."));
    }

    #[test]
    fn deno_write_version_no_field_returns_error() {
        let err = DenoVersionFile.write_version(DENO_NO_VERSION, "1.0.0");
        assert!(err.is_err());
    }

    // =======================================================================
    // Integration tests with tempdir
    // =======================================================================

    #[test]
    fn integration_update_package_json() {
        use crate::version_file::update_version_files;

        let dir = tempfile::tempdir().unwrap();
        let pkg = dir.path().join("package.json");
        std::fs::write(&pkg, PACKAGE_JSON).unwrap();

        let results = update_version_files(dir.path(), "3.0.0", &[]).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].old_version, "1.2.3");
        assert_eq!(results[0].new_version, "3.0.0");
        assert_eq!(results[0].name, "package.json");

        let on_disk = std::fs::read_to_string(&pkg).unwrap();
        assert!(on_disk.contains(r#""version": "3.0.0""#));
    }

    #[test]
    fn integration_update_deno_json() {
        use crate::version_file::update_version_files;

        let dir = tempfile::tempdir().unwrap();
        let deno = dir.path().join("deno.json");
        std::fs::write(&deno, DENO_JSON).unwrap();

        let results = update_version_files(dir.path(), "1.0.0", &[]).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].old_version, "0.5.0");
        assert_eq!(results[0].new_version, "1.0.0");
        assert_eq!(results[0].name, "deno.json");

        let on_disk = std::fs::read_to_string(&deno).unwrap();
        assert!(on_disk.contains(r#""version": "1.0.0""#));
    }

    #[test]
    fn integration_update_deno_jsonc() {
        use crate::version_file::update_version_files;

        let dir = tempfile::tempdir().unwrap();
        let deno = dir.path().join("deno.jsonc");
        std::fs::write(&deno, DENO_JSONC).unwrap();

        let results = update_version_files(dir.path(), "2.0.0", &[]).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].old_version, "0.5.0");
        assert_eq!(results[0].new_version, "2.0.0");
        assert_eq!(results[0].name, "deno.json");

        let on_disk = std::fs::read_to_string(&deno).unwrap();
        assert!(on_disk.contains(r#""version": "2.0.0""#));
        assert!(on_disk.contains("// The current release version."));
    }
}
