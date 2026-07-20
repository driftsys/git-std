//! Detect staged paths that don't match any resolved commit scope, or that
//! look like a better fit for the meta-scope than any single discovered one.
//!
//! Only meaningful for `ScopesConfig::Auto`: an explicit `List` is a closed
//! set of scopes by design and isn't expected to auto-expand when a new
//! top-level directory appears.

use super::SCOPE_DIRS;

/// Top-level directories that are conventionally never commit scopes.
const NON_SCOPE_DIRS: &[&str] = &[
    "docs",
    "scripts",
    "node_modules",
    "target",
    "dist",
    "build",
    "vendor",
];

/// Return the first staged file's top-level directory that doesn't match any
/// entry in `resolved_scopes`.
///
/// Ignores root-level files (no directory component), dotfiles/dot-directories,
/// the [`SCOPE_DIRS`] parents (`crates`/`packages`/`modules` themselves aren't
/// scopes — their children are), and [`NON_SCOPE_DIRS`].
pub fn unmatched_scope_dir(files: &[String], resolved_scopes: &[String]) -> Option<String> {
    for file in files {
        let Some((top, _rest)) = file.split_once('/') else {
            continue;
        };
        if top.starts_with('.') {
            continue;
        }
        if SCOPE_DIRS.contains(&top) || NON_SCOPE_DIRS.contains(&top) {
            continue;
        }
        if resolved_scopes.iter().any(|s| s == top) {
            continue;
        }
        return Some(top.to_string());
    }
    None
}

/// Return the candidate scope name for a staged file path, or `None` for a
/// root-level file (no directory component).
///
/// For files under a [`SCOPE_DIRS`] parent (e.g. `crates/api/...`), the
/// candidate is the child directory name (`api`) — the actual discovered
/// scope. For any other top-level directory, the candidate is that
/// directory name itself.
fn candidate_scope(file: &str) -> Option<String> {
    let (top, rest) = file.split_once('/')?;
    if SCOPE_DIRS.contains(&top) {
        let (child, _) = rest.split_once('/').unwrap_or((rest, ""));
        if child.is_empty() {
            None
        } else {
            Some(child.to_string())
        }
    } else {
        Some(top.to_string())
    }
}

/// Detect whether staged files look like a better fit for the meta-scope
/// than any single discovered scope: either every file is root-level (no
/// directory component), or the files' candidate scopes span two or more
/// distinct entries.
///
/// A mix of root-level files and exactly one matched scope is *not*
/// flagged — the single matched scope is still the more accurate choice.
pub fn meta_scope_suggested(files: &[String]) -> bool {
    if files.is_empty() {
        return false;
    }
    let mut candidates: Vec<String> = Vec::new();
    let mut any_root_level = false;
    for file in files {
        match candidate_scope(file) {
            Some(c) => {
                if !candidates.contains(&c) {
                    candidates.push(c);
                }
            }
            None => any_root_level = true,
        }
    }
    if candidates.is_empty() {
        any_root_level
    } else {
        candidates.len() >= 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_when_no_staged_files() {
        assert_eq!(unmatched_scope_dir(&[], &["api".into()]), None);
    }

    #[test]
    fn none_for_root_level_file() {
        let files = vec!["README.md".to_string()];
        assert_eq!(unmatched_scope_dir(&files, &["api".into()]), None);
    }

    #[test]
    fn none_when_top_dir_already_a_resolved_scope() {
        let files = vec!["api/src/main.rs".to_string()];
        assert_eq!(unmatched_scope_dir(&files, &["api".into()]), None);
    }

    #[test]
    fn some_when_top_dir_not_a_resolved_scope() {
        let files = vec!["services/foo/main.rs".to_string()];
        assert_eq!(
            unmatched_scope_dir(&files, &["api".into()]),
            Some("services".to_string())
        );
    }

    #[test]
    fn none_for_known_scope_dir_parents() {
        // "crates" is the parent scanned by discover_scopes, not a scope itself.
        let files = vec!["crates/api/src/main.rs".to_string()];
        assert_eq!(unmatched_scope_dir(&files, &["api".into()]), None);
    }

    #[test]
    fn none_for_dotdirs() {
        let files = vec![".github/workflows/ci.yml".to_string()];
        assert_eq!(unmatched_scope_dir(&files, &[]), None);
    }

    #[test]
    fn none_for_known_non_scope_dirs() {
        let files = vec!["docs/guide.md".to_string()];
        assert_eq!(unmatched_scope_dir(&files, &[]), None);
    }

    #[test]
    fn meta_scope_not_suggested_when_no_files() {
        assert!(!meta_scope_suggested(&[]));
    }

    #[test]
    fn meta_scope_suggested_when_all_root_level() {
        let files = vec!["README.md".to_string(), ".git-std.toml".to_string()];
        assert!(meta_scope_suggested(&files));
    }

    #[test]
    fn meta_scope_suggested_when_spanning_multiple_crates() {
        let files = vec![
            "crates/api/src/main.rs".to_string(),
            "crates/auth/src/lib.rs".to_string(),
        ];
        assert!(meta_scope_suggested(&files));
    }

    #[test]
    fn meta_scope_not_suggested_for_single_matched_scope() {
        let files = vec!["crates/api/src/main.rs".to_string()];
        assert!(!meta_scope_suggested(&files));
    }

    #[test]
    fn meta_scope_not_suggested_for_single_scope_plus_root_file() {
        let files = vec![
            "README.md".to_string(),
            "crates/api/src/main.rs".to_string(),
        ];
        assert!(!meta_scope_suggested(&files));
    }
}
