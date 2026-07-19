//! Detect staged paths that don't match any resolved commit scope.
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
}
