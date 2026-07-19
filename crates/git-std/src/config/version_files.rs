//! Glob-aware resolution of `[[version_files]]` config entries into flat
//! [`standard_version::CustomVersionFile`] lists.
//!
//! A `path` is treated as a glob when it contains any of `* ? [ {`; otherwise
//! it is a literal path, preserving full backward compatibility with
//! existing configs.

use std::path::{Path, PathBuf};

use super::VersionFileConfig;
use crate::ui;

/// Characters that mark a `path` string as a glob pattern rather than a
/// literal path.
const GLOB_CHARS: &[char] = &['*', '?', '[', '{'];

/// Returns `true` if `path` contains any glob metacharacter.
pub(crate) fn is_glob(path: &str) -> bool {
    path.contains(GLOB_CHARS)
}

/// Expand a single `{a,b,c}` brace-alternation group in `pattern` into
/// multiple literal patterns, one per alternative. Supports multiple groups
/// in the same pattern via a cartesian product. Returns `pattern` unchanged
/// (as the sole element) if it contains no brace group.
///
/// The `glob` crate does not implement brace alternation itself, so this
/// expansion happens before patterns are handed to [`glob::glob`].
fn expand_braces(pattern: &str) -> Vec<String> {
    let Some(open) = pattern.find('{') else {
        return vec![pattern.to_string()];
    };
    let Some(close_offset) = pattern[open..].find('}') else {
        return vec![pattern.to_string()];
    };
    let close = open + close_offset;

    let prefix = &pattern[..open];
    let alternatives = &pattern[open + 1..close];
    let suffix = &pattern[close + 1..];

    let mut expanded = Vec::new();
    for alt in alternatives.split(',') {
        for rest in expand_braces(suffix) {
            expanded.push(format!("{prefix}{alt}{rest}"));
        }
    }
    expanded
}

/// Expand a glob pattern relative to `root`, returning matching file paths
/// relative to `root` (sorted), plus whether any sub-pattern was invalid
/// glob syntax.
///
/// Handles brace alternation (`{a,b}`) via [`expand_braces`] before
/// delegating to [`glob::glob`], since that crate doesn't support it
/// natively. Invalid glob syntax emits a warning here rather than being
/// treated as a silent zero-match; the `bool` return lets the caller avoid
/// piling a redundant "no files matched" warning on top of it.
fn expand_file_glob(root: &Path, pattern: &str) -> (Vec<PathBuf>, bool) {
    let mut results = Vec::new();
    let mut had_error = false;
    for sub_pattern in expand_braces(pattern) {
        let full_pattern = root.join(&sub_pattern);
        let pattern_str = full_pattern.to_string_lossy();

        match glob::glob(&pattern_str) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    if entry.is_file()
                        && let Ok(rel) = entry.strip_prefix(root)
                    {
                        results.push(rel.to_path_buf());
                    }
                }
            }
            Err(e) => {
                had_error = true;
                ui::warning(&format!(
                    "invalid version_files glob pattern: {pattern}: {e}"
                ));
            }
        }
    }
    results.sort();
    results.dedup();
    (results, had_error)
}

/// Resolve `[[version_files]]` config entries into a flat list of
/// [`standard_version::CustomVersionFile`], expanding glob `path` entries
/// into one entry per matched file (all sharing the same `regex`).
///
/// Literal paths are passed through unchanged, exactly as before glob
/// support was added. Glob entries that match zero files emit a warning via
/// [`ui::warning`] — unless the pattern itself was invalid glob syntax, in
/// which case [`expand_file_glob`] already warned about that, and the
/// redundant "no files matched" warning is skipped.
pub(crate) fn resolve_custom_version_files(
    root: &Path,
    configs: &[VersionFileConfig],
) -> Vec<standard_version::CustomVersionFile> {
    let mut resolved = Vec::new();
    for config in configs {
        if is_glob(&config.path) {
            let (matches, had_error) = expand_file_glob(root, &config.path);
            if matches.is_empty() && !had_error {
                ui::warning(&format!(
                    "no files matched version_files glob: {}",
                    config.path
                ));
            }
            for path in matches {
                resolved.push(standard_version::CustomVersionFile {
                    path,
                    pattern: config.regex.clone(),
                });
            }
        } else {
            resolved.push(standard_version::CustomVersionFile {
                path: PathBuf::from(&config.path),
                pattern: config.regex.clone(),
            });
        }
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(root: &Path, path: &str, content: &str) {
        let full = root.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, content).unwrap();
    }

    // ── is_glob ─────────────────────────────────────────────

    #[test]
    fn is_glob_true_for_star() {
        assert!(is_glob("skills/**/*.md"));
    }

    #[test]
    fn is_glob_true_for_question_mark() {
        assert!(is_glob("file?.txt"));
    }

    #[test]
    fn is_glob_true_for_bracket() {
        assert!(is_glob("file[0-9].txt"));
    }

    #[test]
    fn is_glob_true_for_brace() {
        assert!(is_glob("file.{md,txt}"));
    }

    #[test]
    fn is_glob_false_for_literal_path() {
        assert!(!is_glob("Cargo.toml"));
    }

    #[test]
    fn is_glob_false_for_nested_literal_path() {
        assert!(!is_glob("crates/git-std/Cargo.toml"));
    }

    // ── expand_file_glob ────────────────────────────────────

    #[test]
    fn expand_file_glob_matches_multiple_files() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), "skills/alpha/SKILL.md", "version: 0.1.0");
        write_file(dir.path(), "skills/beta/SKILL.md", "version: 0.2.0");

        let (matches, had_error) = expand_file_glob(dir.path(), "skills/*/SKILL.md");
        assert_eq!(
            matches,
            vec![
                PathBuf::from("skills/alpha/SKILL.md"),
                PathBuf::from("skills/beta/SKILL.md"),
            ]
        );
        assert!(!had_error);
    }

    #[test]
    fn expand_file_glob_matches_none() {
        let dir = tempfile::tempdir().unwrap();
        let (matches, had_error) = expand_file_glob(dir.path(), "skills/*/SKILL.md");
        assert!(matches.is_empty());
        assert!(!had_error);
    }

    #[test]
    fn expand_file_glob_invalid_pattern_returns_empty_and_flags_error() {
        let dir = tempfile::tempdir().unwrap();
        // An unterminated `[` character class is invalid glob syntax.
        let (matches, had_error) = expand_file_glob(dir.path(), "skills/[/SKILL.md");
        assert!(matches.is_empty());
        assert!(
            had_error,
            "invalid glob syntax should be flagged distinctly from a valid zero-match"
        );
    }

    #[test]
    fn expand_file_glob_matches_recursive_pattern() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), "skills/a/SKILL.md", "version: 0.1.0");
        write_file(dir.path(), "skills/nested/b/SKILL.md", "version: 0.2.0");

        let (matches, had_error) = expand_file_glob(dir.path(), "skills/**/SKILL.md");
        assert_eq!(
            matches,
            vec![
                PathBuf::from("skills/a/SKILL.md"),
                PathBuf::from("skills/nested/b/SKILL.md"),
            ]
        );
        assert!(!had_error);
    }

    #[test]
    fn expand_file_glob_ignores_directories() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), "skills/alpha.md", "version: 0.1.0");
        std::fs::create_dir_all(dir.path().join("skills/beta")).unwrap();

        // "skills/*" matches both the "alpha.md" file and the "beta"
        // directory; only the file should be returned.
        let (matches, had_error) = expand_file_glob(dir.path(), "skills/*");
        assert_eq!(matches, vec![PathBuf::from("skills/alpha.md")]);
        assert!(!had_error);
    }

    // ── expand_braces ───────────────────────────────────────

    #[test]
    fn expand_braces_expands_single_group() {
        let expanded = expand_braces("file.{md,txt}");
        assert_eq!(expanded, vec!["file.md", "file.txt"]);
    }

    #[test]
    fn expand_braces_returns_unchanged_when_no_braces() {
        let expanded = expand_braces("skills/*/SKILL.md");
        assert_eq!(expanded, vec!["skills/*/SKILL.md"]);
    }

    #[test]
    fn expand_braces_expands_multiple_groups() {
        let mut expanded = expand_braces("{a,b}/file.{md,txt}");
        expanded.sort();
        assert_eq!(
            expanded,
            vec!["a/file.md", "a/file.txt", "b/file.md", "b/file.txt"]
        );
    }

    #[test]
    fn expand_file_glob_matches_brace_alternation() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), "skills/alpha/SKILL.md", "version: 0.1.0");
        write_file(dir.path(), "skills/alpha/SKILL.txt", "version: 0.1.0");
        write_file(dir.path(), "skills/alpha/SKILL.json", "version: 0.1.0");

        let (matches, had_error) = expand_file_glob(dir.path(), "skills/alpha/SKILL.{md,txt}");
        assert_eq!(
            matches,
            vec![
                PathBuf::from("skills/alpha/SKILL.md"),
                PathBuf::from("skills/alpha/SKILL.txt"),
            ]
        );
        assert!(!had_error);
    }

    // ── resolve_custom_version_files ────────────────────────

    #[test]
    fn resolve_mixes_literal_and_glob_entries() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), "skills/alpha/SKILL.md", "version: 0.1.0");
        write_file(dir.path(), "skills/beta/SKILL.md", "version: 0.2.0");
        write_file(dir.path(), "Cargo.toml", "version = \"0.1.0\"");

        let configs = vec![
            VersionFileConfig {
                path: "Cargo.toml".to_string(),
                regex: "version = \"(.+)\"".to_string(),
            },
            VersionFileConfig {
                path: "skills/*/SKILL.md".to_string(),
                regex: "version: (.+)".to_string(),
            },
        ];

        let resolved = resolve_custom_version_files(dir.path(), &configs);

        assert_eq!(resolved.len(), 3);
        assert_eq!(resolved[0].path, PathBuf::from("Cargo.toml"));
        assert_eq!(resolved[0].pattern, "version = \"(.+)\"");
        assert_eq!(resolved[1].path, PathBuf::from("skills/alpha/SKILL.md"));
        assert_eq!(resolved[1].pattern, "version: (.+)");
        assert_eq!(resolved[2].path, PathBuf::from("skills/beta/SKILL.md"));
        assert_eq!(resolved[2].pattern, "version: (.+)");
    }

    #[test]
    fn resolve_glob_with_zero_matches_returns_no_entries() {
        let dir = tempfile::tempdir().unwrap();
        let configs = vec![VersionFileConfig {
            path: "skills/*/SKILL.md".to_string(),
            regex: "version: (.+)".to_string(),
        }];

        let resolved = resolve_custom_version_files(dir.path(), &configs);

        assert!(resolved.is_empty());
    }

    #[test]
    fn resolve_literal_path_passes_through_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let configs = vec![VersionFileConfig {
            path: "does/not/exist.toml".to_string(),
            regex: "version = \"(.+)\"".to_string(),
        }];

        let resolved = resolve_custom_version_files(dir.path(), &configs);

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].path, PathBuf::from("does/not/exist.toml"));
    }
}
