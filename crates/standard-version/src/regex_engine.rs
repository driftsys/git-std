//! Regex-based version file engine for user-defined `[[version_files]]`.
//!
//! Implements version detection, reading, and writing for arbitrary files
//! matched by path and a regex pattern whose first capture group contains
//! the version string.

use std::path::{Path, PathBuf};

use regex::Regex;

use crate::version_file::{CustomVersionFile, VersionFileError};

/// A version file engine driven by a user-supplied regex.
///
/// The regex must contain at least one capture group. The first capture
/// group is treated as the version string for both reading and writing.
#[derive(Debug)]
pub struct RegexVersionFile {
    /// Path to the file, relative to the repository root.
    path: PathBuf,
    /// Compiled regex pattern.
    pattern: Regex,
}

impl RegexVersionFile {
    /// Create a new engine from a [`CustomVersionFile`] config entry.
    ///
    /// # Errors
    ///
    /// Returns [`VersionFileError::InvalidRegex`] if the pattern fails to
    /// compile or contains no capture groups.
    pub fn new(custom: &CustomVersionFile) -> Result<Self, VersionFileError> {
        let pattern = Regex::new(&custom.pattern)
            .map_err(|e| VersionFileError::InvalidRegex(format!("invalid regex: {e}")))?;

        if pattern.captures_len() < 2 {
            return Err(VersionFileError::InvalidRegex(
                "regex must contain at least one capture group".to_string(),
            ));
        }

        Ok(Self {
            path: custom.path.clone(),
            pattern,
        })
    }

    /// Human-readable name (the file path as a string).
    pub fn name(&self) -> String {
        self.path.display().to_string()
    }

    /// The file path relative to the repository root.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Check if `content` contains a match for the regex pattern.
    pub fn detect(&self, content: &str) -> bool {
        self.pattern.is_match(content)
    }

    /// Extract the version string from the first capture group.
    pub fn read_version(&self, content: &str) -> Option<String> {
        self.pattern
            .captures(content)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }

    /// Return updated content with the first capture group replaced by
    /// `new_version`, preserving all surrounding text.
    ///
    /// # Errors
    ///
    /// Returns [`VersionFileError::NoVersionField`] if the regex does not
    /// match `content`.
    pub fn write_version(
        &self,
        content: &str,
        new_version: &str,
    ) -> Result<String, VersionFileError> {
        let caps = self
            .pattern
            .captures(content)
            .ok_or(VersionFileError::NoVersionField)?;

        let group = caps.get(1).ok_or(VersionFileError::NoVersionField)?;

        let mut result = String::with_capacity(content.len());
        result.push_str(&content[..group.start()]);
        result.push_str(new_version);
        result.push_str(&content[group.end()..]);

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn custom(path: &str, pattern: &str) -> CustomVersionFile {
        CustomVersionFile {
            path: PathBuf::from(path),
            pattern: pattern.to_string(),
        }
    }

    // --- constructor ---

    #[test]
    fn new_with_valid_regex() {
        let engine = RegexVersionFile::new(&custom("pom.xml", r"<version>(.*?)</version>"));
        assert!(engine.is_ok());
    }

    #[test]
    fn new_with_no_capture_group_errors() {
        let engine = RegexVersionFile::new(&custom("file.txt", r"version = \d+\.\d+\.\d+"));
        assert!(engine.is_err());
        let err = engine.unwrap_err().to_string();
        assert!(
            err.contains("capture group"),
            "expected capture group error, got: {err}"
        );
    }

    #[test]
    fn new_with_malformed_regex_errors() {
        let engine = RegexVersionFile::new(&custom("file.txt", r"(unclosed"));
        assert!(engine.is_err());
        let err = engine.unwrap_err().to_string();
        assert!(
            err.contains("invalid regex"),
            "expected invalid regex error, got: {err}"
        );
    }

    // --- detect ---

    #[test]
    fn detect_positive() {
        let engine =
            RegexVersionFile::new(&custom("pom.xml", r"<version>(.*?)</version>")).unwrap();
        assert!(engine.detect("<version>1.2.3</version>"));
    }

    #[test]
    fn detect_negative() {
        let engine =
            RegexVersionFile::new(&custom("pom.xml", r"<version>(.*?)</version>")).unwrap();
        assert!(!engine.detect("<name>my-project</name>"));
    }

    // --- read_version ---

    #[test]
    fn read_version_extracts_first_capture_group() {
        let engine =
            RegexVersionFile::new(&custom("pom.xml", r"<version>(.*?)</version>")).unwrap();
        let version = engine.read_version("<version>1.2.3</version>");
        assert_eq!(version, Some("1.2.3".to_string()));
    }

    #[test]
    fn read_version_returns_none_on_no_match() {
        let engine =
            RegexVersionFile::new(&custom("pom.xml", r"<version>(.*?)</version>")).unwrap();
        assert_eq!(engine.read_version("<name>foo</name>"), None);
    }

    // --- write_version ---

    #[test]
    fn write_version_replaces_preserving_context() {
        let engine =
            RegexVersionFile::new(&custom("pom.xml", r"<version>(.*?)</version>")).unwrap();
        let content = "<project>\n  <version>1.2.3</version>\n</project>";
        let updated = engine.write_version(content, "2.0.0").unwrap();
        assert_eq!(updated, "<project>\n  <version>2.0.0</version>\n</project>");
    }

    #[test]
    fn write_version_error_on_no_match() {
        let engine =
            RegexVersionFile::new(&custom("pom.xml", r"<version>(.*?)</version>")).unwrap();
        let result = engine.write_version("<name>foo</name>", "1.0.0");
        assert!(result.is_err());
    }

    // --- XML-like pattern ---

    #[test]
    fn xml_version_roundtrip() {
        let xml = r#"<?xml version="1.0"?>
<project>
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.example</groupId>
  <artifactId>my-app</artifactId>
  <version>1.0.0-SNAPSHOT</version>
</project>"#;

        let engine = RegexVersionFile::new(&custom(
            "pom.xml",
            r"<version>([^<]+)</version>(?s:.)*</project>",
        ))
        .unwrap();
        assert!(engine.detect(xml));
        // Note: this matches modelVersion first; use a more specific pattern
        // in practice. Let's test with the specific artifact version pattern.
        let engine2 = RegexVersionFile::new(&custom(
            "pom.xml",
            r"<artifactId>my-app</artifactId>\s*<version>([^<]+)</version>",
        ))
        .unwrap();
        assert_eq!(
            engine2.read_version(xml),
            Some("1.0.0-SNAPSHOT".to_string())
        );
        let updated = engine2.write_version(xml, "2.0.0").unwrap();
        assert!(updated.contains("<version>2.0.0</version>"));
        assert!(updated.contains("<modelVersion>4.0.0</modelVersion>"));
    }

    // --- CMake-like pattern ---

    #[test]
    fn cmake_version_roundtrip() {
        let cmake = "cmake_minimum_required(VERSION 3.14)\nproject(myapp VERSION 1.2.3)\n";
        let engine = RegexVersionFile::new(&custom(
            "CMakeLists.txt",
            r"project\(myapp VERSION ([^\)]+)\)",
        ))
        .unwrap();
        assert!(engine.detect(cmake));
        assert_eq!(engine.read_version(cmake), Some("1.2.3".to_string()));
        let updated = engine.write_version(cmake, "3.0.0").unwrap();
        assert!(updated.contains("project(myapp VERSION 3.0.0)"));
        assert!(updated.contains("cmake_minimum_required(VERSION 3.14)"));
    }

    // --- integration via update_version_files ---

    #[test]
    fn update_version_files_processes_custom_file() {
        let dir = tempfile::tempdir().unwrap();
        let pom = dir.path().join("pom.xml");
        fs::write(&pom, "<project>\n  <version>1.0.0</version>\n</project>\n").unwrap();

        let custom_files = vec![custom("pom.xml", r"<version>([^<]+)</version>")];
        let results =
            crate::version_file::update_version_files(dir.path(), "2.0.0", &custom_files).unwrap();

        // Find the custom file result (there may also be built-in results).
        let pom_result = results.iter().find(|r| r.name == "pom.xml");
        assert!(pom_result.is_some(), "expected pom.xml in results");
        let r = pom_result.unwrap();
        assert_eq!(r.old_version, "1.0.0");
        assert_eq!(r.new_version, "2.0.0");

        let on_disk = fs::read_to_string(&pom).unwrap();
        assert!(on_disk.contains("<version>2.0.0</version>"));
    }

    #[test]
    fn update_version_files_skips_missing_custom_file() {
        let dir = tempfile::tempdir().unwrap();
        // No pom.xml on disk.
        let custom_files = vec![custom("pom.xml", r"<version>([^<]+)</version>")];
        let results =
            crate::version_file::update_version_files(dir.path(), "2.0.0", &custom_files).unwrap();
        assert!(
            results.iter().all(|r| r.name != "pom.xml"),
            "missing file should be skipped"
        );
    }

    #[test]
    fn update_version_files_skips_non_matching_regex() {
        let dir = tempfile::tempdir().unwrap();
        let txt = dir.path().join("version.txt");
        fs::write(&txt, "no version here\n").unwrap();

        let custom_files = vec![custom("version.txt", r"version = ([^\n]+)")];
        let results =
            crate::version_file::update_version_files(dir.path(), "2.0.0", &custom_files).unwrap();
        assert!(
            results.iter().all(|r| r.name != "version.txt"),
            "non-matching regex should be skipped"
        );
    }
}
