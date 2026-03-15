//! Glob matching for staged/tracked file paths.
//!
//! Provides gitignore-style glob matching to determine whether a hook command
//! should run based on the files that changed. Patterns follow the same syntax
//! as `.gitignore` entries (e.g., `*.rs`, `modules/**/*.kt`).
//!
//! This module is pure — it receives pre-collected file paths and performs no
//! I/O or git operations.

use std::path::Path;

use globset::GlobBuilder;

/// Returns `true` if at least one path in `files` matches the given glob
/// `pattern`.
///
/// The pattern uses gitignore-style syntax:
///
/// - `*.rs` matches any `.rs` file (at any depth, since the slash-less form
///   is matched against the basename only)
/// - `src/**/*.rs` matches `.rs` files anywhere under `src/`
/// - `modules/*.kt` matches `.kt` files directly inside `modules/`
///
/// # Examples
///
/// ```
/// use standard_githooks::matches_any;
///
/// let files = vec!["src/main.rs", "src/lib.rs", "README.md"];
/// assert!(matches_any("*.rs", &files));
/// assert!(!matches_any("*.kt", &files));
/// ```
pub fn matches_any(pattern: &str, files: &[&str]) -> bool {
    let is_basename = !pattern.contains('/');
    let glob = match GlobBuilder::new(pattern).literal_separator(true).build() {
        Ok(g) => g,
        Err(_) => return false,
    };
    let matcher = glob.compile_matcher();

    if is_basename {
        // Basename-only pattern: match against the file name component,
        // mirroring gitignore behaviour where `*.rs` matches `src/lib.rs`.
        files.iter().any(|f| {
            Path::new(f)
                .file_name()
                .is_some_and(|name| matcher.is_match(name))
        })
    } else {
        // Path pattern: match against the full relative path.
        files.iter().any(|f| matcher.is_match(f))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Basic extension matching ──────────────────────────────────

    #[test]
    fn matches_rust_files() {
        let files = vec!["src/main.rs", "src/lib.rs", "README.md"];
        assert!(matches_any("*.rs", &files));
    }

    #[test]
    fn no_match_returns_false() {
        let files = vec!["src/main.rs", "README.md"];
        assert!(!matches_any("*.kt", &files));
    }

    #[test]
    fn empty_file_list_returns_false() {
        let files: Vec<&str> = vec![];
        assert!(!matches_any("*.rs", &files));
    }

    // ── Nested paths ──────────────────────────────────────────────

    #[test]
    fn star_pattern_matches_nested_files() {
        // `*.rs` without a slash is a basename pattern — matches at any depth
        let files = vec!["crates/core/src/lib.rs"];
        assert!(matches_any("*.rs", &files));
    }

    #[test]
    fn double_star_glob() {
        let files = vec![
            "modules/auth/Login.kt",
            "modules/core/Base.kt",
            "build.gradle",
        ];
        assert!(matches_any("modules/**/*.kt", &files));
        assert!(!matches_any("modules/**/*.rs", &files));
    }

    #[test]
    fn single_directory_glob() {
        let files = vec!["src/main.rs", "tests/integration.rs"];
        assert!(matches_any("src/*.rs", &files));
        assert!(!matches_any("lib/*.rs", &files));
    }

    // ── Exact file matching ───────────────────────────────────────

    #[test]
    fn exact_filename() {
        let files = vec!["Cargo.toml", "src/main.rs"];
        assert!(matches_any("Cargo.toml", &files));
        assert!(!matches_any("package.json", &files));
    }

    // ── Edge cases ────────────────────────────────────────────────

    #[test]
    fn pattern_with_leading_dot() {
        let files = vec![".github/workflows/ci.yml", "src/main.rs"];
        assert!(matches_any(".github/**/*.yml", &files));
    }

    #[test]
    fn multiple_extensions() {
        let files = vec!["app.ts", "utils.tsx", "style.css"];
        assert!(matches_any("*.ts", &files));
        // *.ts should NOT match .tsx (literal separator matching)
        assert!(matches_any("*.tsx", &files));
    }

    #[test]
    fn deeply_nested_match() {
        let files = vec!["a/b/c/d/e/file.py"];
        assert!(matches_any("*.py", &files));
        assert!(matches_any("a/**/*.py", &files));
    }

    #[test]
    fn slash_in_pattern_restricts_to_path() {
        // With a slash, the pattern is anchored — `src/*.rs` should not match
        // files in subdirectories of `src/`.
        let files = vec!["src/sub/deep.rs"];
        assert!(!matches_any("src/*.rs", &files));
        // But double-star should match
        assert!(matches_any("src/**/*.rs", &files));
    }
}
