/// A commit entry ready for changelog rendering.
#[derive(Debug, Clone)]
pub struct ChangelogEntry {
    /// The optional scope (e.g. `auth`).
    pub scope: Option<String>,
    /// The commit description (subject line).
    pub description: String,
    /// Short commit hash.
    pub hash: String,
    /// Whether this commit is a breaking change.
    pub is_breaking: bool,
    /// Issue references from footers, as `(token, value)` pairs
    /// (e.g. `("closes", "#45")`, `("refs", "#46")`).
    pub refs: Vec<(String, String)>,
}

/// A version release with grouped commits.
#[derive(Debug, Clone)]
pub struct VersionRelease {
    /// The version tag (e.g. `"0.2.0"`).
    pub tag: String,
    /// The release date (e.g. `"2026-03-13"`).
    pub date: String,
    /// The previous tag for generating compare URLs.
    pub prev_tag: Option<String>,
    /// Grouped entries: `(section_name, entries)`.
    pub groups: Vec<(String, Vec<ChangelogEntry>)>,
    /// `BREAKING CHANGE` footer values.
    pub breaking_changes: Vec<String>,
}

/// Repo host for generating links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoHost {
    /// GitHub-hosted repository.
    GitHub {
        /// Full URL, e.g. `"https://github.com/owner/repo"`.
        url: String,
    },
    /// GitLab-hosted repository.
    GitLab {
        /// Full URL, e.g. `"https://gitlab.com/owner/repo"`.
        url: String,
    },
    /// Unknown host — no links generated.
    Unknown,
}

/// Configuration for changelog rendering.
#[derive(Debug, Clone)]
pub struct ChangelogConfig {
    /// The changelog title (default `"Changelog"`).
    pub title: String,
    /// Mapping of commit type to section title.
    pub sections: Vec<(String, String)>,
    /// Commit types to exclude from the changelog.
    pub hidden: Vec<String>,
    /// Optional bug tracker URL template (e.g. `"https://jira.company.com/browse/%s"`).
    pub bug_url: Option<String>,
}

impl Default for ChangelogConfig {
    fn default() -> Self {
        Self {
            title: "Changelog".to_string(),
            sections: vec![
                ("feat".to_string(), "Features".to_string()),
                ("fix".to_string(), "Bug Fixes".to_string()),
                ("perf".to_string(), "Performance".to_string()),
                ("revert".to_string(), "Reverts".to_string()),
                ("refactor".to_string(), "Refactoring".to_string()),
                ("docs".to_string(), "Documentation".to_string()),
            ],
            hidden: vec![
                "chore".to_string(),
                "ci".to_string(),
                "build".to_string(),
                "style".to_string(),
                "test".to_string(),
            ],
            bug_url: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = ChangelogConfig::default();

        assert_eq!(config.title, "Changelog");
        assert_eq!(
            config.sections,
            vec![
                ("feat".to_string(), "Features".to_string()),
                ("fix".to_string(), "Bug Fixes".to_string()),
                ("perf".to_string(), "Performance".to_string()),
                ("revert".to_string(), "Reverts".to_string()),
                ("refactor".to_string(), "Refactoring".to_string()),
                ("docs".to_string(), "Documentation".to_string()),
            ]
        );
        assert_eq!(
            config.hidden,
            vec![
                "chore".to_string(),
                "ci".to_string(),
                "build".to_string(),
                "style".to_string(),
                "test".to_string(),
            ]
        );
        assert!(config.bug_url.is_none());
    }
}
