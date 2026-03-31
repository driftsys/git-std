//! Semantic version bump calculation from conventional commits.
//!
//! Computes the next version from a list of parsed conventional commits and
//! bump rules.
//!
//! # Main entry points
//!
//! - [`determine_bump`] -- analyse commits and return the bump level
//! - [`apply_bump`] -- apply a bump level to a semver version
//! - [`apply_prerelease`] -- bump with a pre-release tag (e.g. `rc.0`)
//! - [`summarise`] -- count commits by category for display

use standard_commit::ConventionalCommit;

/// The level of version bump to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BumpLevel {
    /// Bug fix -- increment the patch component.
    Patch,
    /// New feature -- increment the minor component.
    Minor,
    /// Breaking change -- increment the major component.
    Major,
}

/// Analyse a list of conventional commits and return the highest applicable
/// bump level.
///
/// Bump rules follow the [Conventional Commits](https://www.conventionalcommits.org/)
/// specification:
/// - `feat` → [`BumpLevel::Minor`]
/// - `fix` or `perf` → [`BumpLevel::Patch`]
/// - `BREAKING CHANGE` footer or `!` suffix → [`BumpLevel::Major`]
///
/// Returns `None` when no bump-worthy commits exist (e.g. only `chore`,
/// `docs`, `refactor`).
pub fn determine_bump(commits: &[ConventionalCommit]) -> Option<BumpLevel> {
    let mut level: Option<BumpLevel> = None;

    for commit in commits {
        let bump = commit_bump(commit);
        if let Some(b) = bump {
            level = Some(match level {
                Some(current) => current.max(b),
                None => b,
            });
        }
    }

    level
}

/// Determine the bump level for a single commit.
fn commit_bump(commit: &ConventionalCommit) -> Option<BumpLevel> {
    // Breaking change (footer or `!` suffix) always yields Major.
    if commit.is_breaking {
        return Some(BumpLevel::Major);
    }
    for footer in &commit.footers {
        if footer.token == "BREAKING CHANGE" || footer.token == "BREAKING-CHANGE" {
            return Some(BumpLevel::Major);
        }
    }

    match commit.r#type.as_str() {
        "feat" => Some(BumpLevel::Minor),
        "fix" | "perf" | "revert" => Some(BumpLevel::Patch),
        _ => None,
    }
}

/// Apply a bump level to a semver version, returning the new version.
///
/// Resets lower components to zero (e.g. minor bump `1.2.3` → `1.3.0`).
/// For versions `< 1.0.0`, major bumps still increment the major component.
pub fn apply_bump(current: &semver::Version, level: BumpLevel) -> semver::Version {
    let mut next = current.clone();
    // Clear any pre-release or build metadata.
    next.pre = semver::Prerelease::EMPTY;
    next.build = semver::BuildMetadata::EMPTY;

    match level {
        BumpLevel::Major => {
            next.major += 1;
            next.minor = 0;
            next.patch = 0;
        }
        BumpLevel::Minor => {
            next.minor += 1;
            next.patch = 0;
        }
        BumpLevel::Patch => {
            next.patch += 1;
        }
    }

    next
}

/// Apply a pre-release bump. If the current version already has a pre-release
/// tag matching `tag`, the numeric suffix is incremented. Otherwise, `.0` is
/// appended to the bumped version.
///
/// Example: `1.0.0` + Minor + tag `"rc"` → `1.1.0-rc.0`
/// Example: `1.1.0-rc.0` + tag `"rc"` → `1.1.0-rc.1`
pub fn apply_prerelease(current: &semver::Version, level: BumpLevel, tag: &str) -> semver::Version {
    // If already a pre-release with the same tag prefix, just bump the counter.
    if !current.pre.is_empty() {
        let pre_str = current.pre.as_str();
        if let Some(rest) = pre_str.strip_prefix(tag)
            && let Some(num_str) = rest.strip_prefix('.')
            && let Ok(n) = num_str.parse::<u64>()
        {
            let mut next = current.clone();
            next.pre = semver::Prerelease::new(&format!("{tag}.{}", n + 1)).unwrap_or_default();
            next.build = semver::BuildMetadata::EMPTY;
            return next;
        }
    }

    // Otherwise, bump normally then append the pre-release tag.
    let mut next = apply_bump(current, level);
    next.pre = semver::Prerelease::new(&format!("{tag}.0")).unwrap_or_default();
    next
}

/// Summary of analysed commits for display purposes.
///
/// Counts commits by category. A single commit may increment both
/// `breaking_count` and its type count (e.g. a breaking `feat` increments
/// both `feat_count` and `breaking_count`).
#[derive(Debug, Default)]
pub struct BumpSummary {
    /// Count of `feat` commits.
    pub feat_count: usize,
    /// Count of `fix` commits.
    pub fix_count: usize,
    /// Count of commits with breaking changes.
    pub breaking_count: usize,
    /// Count of other conventional commits (perf, refactor, etc.).
    pub other_count: usize,
}

/// Summarise a list of conventional commits for display purposes.
pub fn summarise(commits: &[ConventionalCommit]) -> BumpSummary {
    let mut summary = BumpSummary::default();
    for commit in commits {
        let is_breaking = commit.is_breaking
            || commit
                .footers
                .iter()
                .any(|f| f.token == "BREAKING CHANGE" || f.token == "BREAKING-CHANGE");
        if is_breaking {
            summary.breaking_count += 1;
        }
        match commit.r#type.as_str() {
            "feat" => summary.feat_count += 1,
            "fix" => summary.fix_count += 1,
            _ => summary.other_count += 1,
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use standard_commit::Footer;

    fn commit(typ: &str, breaking: bool) -> ConventionalCommit {
        ConventionalCommit {
            r#type: typ.to_string(),
            scope: None,
            description: "test".to_string(),
            body: None,
            footers: vec![],
            is_breaking: breaking,
        }
    }

    fn commit_with_footer(typ: &str, footer_token: &str) -> ConventionalCommit {
        ConventionalCommit {
            r#type: typ.to_string(),
            scope: None,
            description: "test".to_string(),
            body: None,
            footers: vec![Footer {
                token: footer_token.to_string(),
                value: "some breaking change".to_string(),
            }],
            is_breaking: false,
        }
    }

    #[test]
    fn no_commits_returns_none() {
        assert_eq!(determine_bump(&[]), None);
    }

    #[test]
    fn non_bump_commits_return_none() {
        let commits = vec![commit("chore", false), commit("docs", false)];
        assert_eq!(determine_bump(&commits), None);
    }

    #[test]
    fn fix_yields_patch() {
        let commits = vec![commit("fix", false)];
        assert_eq!(determine_bump(&commits), Some(BumpLevel::Patch));
    }

    #[test]
    fn perf_yields_patch() {
        let commits = vec![commit("perf", false)];
        assert_eq!(determine_bump(&commits), Some(BumpLevel::Patch));
    }

    #[test]
    fn feat_yields_minor() {
        let commits = vec![commit("feat", false)];
        assert_eq!(determine_bump(&commits), Some(BumpLevel::Minor));
    }

    #[test]
    fn breaking_bang_yields_major() {
        let commits = vec![commit("feat", true)];
        assert_eq!(determine_bump(&commits), Some(BumpLevel::Major));
    }

    #[test]
    fn breaking_footer_yields_major() {
        let commits = vec![commit_with_footer("fix", "BREAKING CHANGE")];
        assert_eq!(determine_bump(&commits), Some(BumpLevel::Major));
    }

    #[test]
    fn breaking_change_hyphenated_footer() {
        let commits = vec![commit_with_footer("fix", "BREAKING-CHANGE")];
        assert_eq!(determine_bump(&commits), Some(BumpLevel::Major));
    }

    #[test]
    fn highest_bump_wins() {
        let commits = vec![commit("fix", false), commit("feat", false)];
        assert_eq!(determine_bump(&commits), Some(BumpLevel::Minor));
    }

    #[test]
    fn breaking_beats_all() {
        let commits = vec![
            commit("fix", false),
            commit("feat", false),
            commit("chore", true),
        ];
        assert_eq!(determine_bump(&commits), Some(BumpLevel::Major));
    }

    #[test]
    fn apply_bump_patch() {
        let v = semver::Version::new(1, 2, 3);
        assert_eq!(
            apply_bump(&v, BumpLevel::Patch),
            semver::Version::new(1, 2, 4)
        );
    }

    #[test]
    fn apply_bump_minor() {
        let v = semver::Version::new(1, 2, 3);
        assert_eq!(
            apply_bump(&v, BumpLevel::Minor),
            semver::Version::new(1, 3, 0)
        );
    }

    #[test]
    fn apply_bump_major() {
        let v = semver::Version::new(1, 2, 3);
        assert_eq!(
            apply_bump(&v, BumpLevel::Major),
            semver::Version::new(2, 0, 0)
        );
    }

    #[test]
    fn apply_bump_clears_prerelease() {
        let v = semver::Version::parse("1.2.3-rc.1").unwrap();
        assert_eq!(
            apply_bump(&v, BumpLevel::Patch),
            semver::Version::new(1, 2, 4)
        );
    }

    #[test]
    fn apply_prerelease_new() {
        let v = semver::Version::new(1, 0, 0);
        let next = apply_prerelease(&v, BumpLevel::Minor, "rc");
        assert_eq!(next, semver::Version::parse("1.1.0-rc.0").unwrap());
    }

    #[test]
    fn apply_prerelease_increment() {
        let v = semver::Version::parse("1.1.0-rc.0").unwrap();
        let next = apply_prerelease(&v, BumpLevel::Minor, "rc");
        assert_eq!(next, semver::Version::parse("1.1.0-rc.1").unwrap());
    }

    #[test]
    fn apply_prerelease_different_tag() {
        let v = semver::Version::parse("1.1.0-alpha.2").unwrap();
        let next = apply_prerelease(&v, BumpLevel::Minor, "rc");
        // Different tag → bump normally and start at 0.
        assert_eq!(next, semver::Version::parse("1.2.0-rc.0").unwrap());
    }

    #[test]
    fn summarise_counts() {
        let commits = vec![
            commit("feat", false),
            commit("feat", false),
            commit("fix", false),
            commit("chore", true),
            commit("refactor", false),
        ];
        let s = summarise(&commits);
        assert_eq!(s.feat_count, 2);
        assert_eq!(s.fix_count, 1);
        assert_eq!(s.breaking_count, 1);
        assert_eq!(s.other_count, 2); // chore + refactor
    }

    #[test]
    fn bump_level_ordering() {
        assert!(BumpLevel::Major > BumpLevel::Minor);
        assert!(BumpLevel::Minor > BumpLevel::Patch);
    }
}
