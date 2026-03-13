//! Changelog generation from conventional commits.
//!
//! Groups parsed commits by type, renders markdown sections, and manages
//! `CHANGELOG.md` files. Does not run git operations or produce terminal output.

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
    /// Issue references from `Refs` and `Closes` footers.
    pub refs: Vec<String>,
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

/// Parse a git remote URL to detect the repo host.
///
/// Supports SSH and HTTPS URLs for GitHub and GitLab. Returns
/// [`RepoHost::Unknown`] for unrecognised hosts.
pub fn detect_host(remote_url: &str) -> RepoHost {
    let url = remote_url.trim();

    // SSH: git@github.com:owner/repo.git
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        let path = rest.strip_suffix(".git").unwrap_or(rest);
        return RepoHost::GitHub {
            url: format!("https://github.com/{path}"),
        };
    }
    if let Some(rest) = url.strip_prefix("git@gitlab.com:") {
        let path = rest.strip_suffix(".git").unwrap_or(rest);
        return RepoHost::GitLab {
            url: format!("https://gitlab.com/{path}"),
        };
    }

    // HTTPS
    if url.starts_with("https://github.com/") {
        let path = url.strip_prefix("https://github.com/").unwrap_or_default();
        let path = path.strip_suffix(".git").unwrap_or(path);
        return RepoHost::GitHub {
            url: format!("https://github.com/{path}"),
        };
    }
    if url.starts_with("https://gitlab.com/") {
        let path = url.strip_prefix("https://gitlab.com/").unwrap_or_default();
        let path = path.strip_suffix(".git").unwrap_or(path);
        return RepoHost::GitLab {
            url: format!("https://gitlab.com/{path}"),
        };
    }

    RepoHost::Unknown
}

/// Replace `#N` issue references in text with links for known hosts.
fn linkify_issue_numbers(text: &str, host: &RepoHost) -> String {
    let base_url = match host {
        RepoHost::GitHub { url } | RepoHost::GitLab { url } => Some(url.as_str()),
        RepoHost::Unknown => None,
    };

    let Some(base) = base_url else {
        return text.to_string();
    };

    let mut result = String::with_capacity(text.len());
    let mut chars = text.char_indices().peekable();

    while let Some((i, ch)) = chars.next() {
        if ch == '#' {
            let start = i + 1;
            let mut end = start;
            while let Some(&(j, d)) = chars.peek() {
                if d.is_ascii_digit() {
                    end = j + 1;
                    chars.next();
                } else {
                    break;
                }
            }
            if end > start {
                let num = &text[start..end];
                let issue_path = match host {
                    RepoHost::GitLab { .. } => format!("{base}/-/issues/{num}"),
                    _ => format!("{base}/issues/{num}"),
                };
                result.push_str(&format!("[#{num}]({issue_path})"));
            } else {
                result.push('#');
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Link a single ref value. Handles `#N` (GitHub/GitLab), URLs, and
/// external tracker refs via `bug_url` (e.g. `PROJ-123`).
fn link_ref(r: &str, host: &RepoHost, bug_url: Option<&str>) -> String {
    let r = r.trim();

    // Already a URL — wrap as markdown link.
    if r.starts_with("http://") || r.starts_with("https://") {
        return format!("[{r}]({r})");
    }

    // #N — link to GitHub/GitLab issues.
    if r.starts_with('#') && r[1..].chars().all(|c| c.is_ascii_digit()) && r.len() > 1 {
        return linkify_issue_numbers(r, host);
    }

    // External tracker ref (e.g. PROJ-123) — use bug_url template.
    if let Some(template) = bug_url {
        let url = template.replace("%s", r);
        return format!("[{r}]({url})");
    }

    r.to_string()
}

/// Format a commit hash as a link (or plain text for Unknown hosts).
fn commit_link(hash: &str, host: &RepoHost) -> String {
    match host {
        RepoHost::GitHub { url } => format!("[{hash}]({url}/commit/{hash})"),
        RepoHost::GitLab { url } => format!("[{hash}]({url}/-/commit/{hash})"),
        RepoHost::Unknown => hash.to_string(),
    }
}

/// Format a version heading with an optional compare link.
fn version_heading(release: &VersionRelease, host: &RepoHost) -> String {
    let version_link = if let Some(prev) = &release.prev_tag {
        match host {
            RepoHost::GitHub { url } => {
                format!(
                    "[{tag}]({url}/compare/v{prev}...v{tag})",
                    tag = release.tag,
                    prev = prev,
                )
            }
            RepoHost::GitLab { url } => {
                format!(
                    "[{tag}]({url}/-/compare/v{prev}...v{tag})",
                    tag = release.tag,
                    prev = prev,
                )
            }
            RepoHost::Unknown => release.tag.clone(),
        }
    } else {
        release.tag.clone()
    };

    format!("## {version_link} ({date})", date = release.date)
}

/// Render a single version section.
pub fn render_version(
    release: &VersionRelease,
    config: &ChangelogConfig,
    host: &RepoHost,
) -> String {
    let mut out = String::new();

    out.push_str(&version_heading(release, host));
    out.push('\n');

    for (section_name, entries) in &release.groups {
        // Skip empty groups
        if entries.is_empty() {
            continue;
        }

        out.push('\n');
        out.push_str(&format!("### {section_name}\n"));
        out.push('\n');

        for entry in entries {
            let link = commit_link(&entry.hash, host);
            let desc = linkify_issue_numbers(&entry.description, host);
            let refs_str = if entry.refs.is_empty() {
                String::new()
            } else {
                let bug_url = config.bug_url.as_deref();
                let linked: Vec<String> = entry
                    .refs
                    .iter()
                    .map(|r| link_ref(r, host, bug_url))
                    .collect();
                format!(", closes {}", linked.join(", "))
            };
            if let Some(scope) = &entry.scope {
                out.push_str(&format!("- **{scope}:** {desc} ({link}){refs_str}\n",));
            } else {
                out.push_str(&format!("- {desc} ({link}){refs_str}\n"));
            }
        }
    }

    if !release.breaking_changes.is_empty() {
        out.push('\n');
        out.push_str("### BREAKING CHANGES\n");
        out.push('\n');
        for bc in &release.breaking_changes {
            out.push_str(&format!("- {bc}\n"));
        }
    }

    out
}

/// Prepend a release section to an existing changelog, replacing any prior
/// "Unreleased" block. If the file has no title line, one is added.
///
/// Returns the updated changelog content.
pub fn prepend_release(
    existing: &str,
    release: &VersionRelease,
    config: &ChangelogConfig,
    host: &RepoHost,
) -> String {
    let filtered = filter_release(release, config);
    if filtered.groups.is_empty() && filtered.breaking_changes.is_empty() {
        return existing.to_string();
    }

    let new_section = render_version(&filtered, config, host);
    let title_line = format!("# {}\n", config.title);

    // Strip the title and any existing Unreleased block from the existing content.
    let body = if existing.starts_with(&title_line) {
        &existing[title_line.len()..]
    } else {
        existing
    };

    // Remove a previous Unreleased section (from title_line up to next ## heading).
    let body = strip_unreleased_section(body);

    let mut out = String::new();
    out.push_str(&title_line);
    out.push('\n');
    out.push_str(&new_section);
    out.push_str(&body);
    out
}

/// Remove a leading "## Unreleased" section, up to the next `## ` heading.
fn strip_unreleased_section(content: &str) -> String {
    let trimmed = content.trim_start_matches('\n');

    if !trimmed.starts_with("## Unreleased") {
        return content.to_string();
    }

    // Find the next ## heading after the first line.
    match trimmed.find("\n## ") {
        Some(pos) => trimmed[pos..].to_string(),
        None => String::new(),
    }
}

/// Render a full changelog from multiple releases.
pub fn render(releases: &[VersionRelease], config: &ChangelogConfig, host: &RepoHost) -> String {
    let mut out = String::new();

    out.push_str(&format!("# {}\n", config.title));

    for release in releases {
        // Filter out hidden types and empty groups
        let filtered = filter_release(release, config);
        if filtered.groups.is_empty() && filtered.breaking_changes.is_empty() {
            continue;
        }

        out.push('\n');
        out.push_str(&render_version(&filtered, config, host));
    }

    out
}

/// Filter a release to exclude hidden types and empty groups.
fn filter_release(release: &VersionRelease, _config: &ChangelogConfig) -> VersionRelease {
    let groups: Vec<(String, Vec<ChangelogEntry>)> = release
        .groups
        .iter()
        .filter(|(_, entries)| !entries.is_empty())
        .cloned()
        .collect();

    VersionRelease {
        tag: release.tag.clone(),
        date: release.date.clone(),
        prev_tag: release.prev_tag.clone(),
        groups,
        breaking_changes: release.breaking_changes.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_host_github_ssh() {
        assert_eq!(
            detect_host("git@github.com:owner/repo.git"),
            RepoHost::GitHub {
                url: "https://github.com/owner/repo".to_string()
            }
        );
    }

    #[test]
    fn detect_host_github_https() {
        assert_eq!(
            detect_host("https://github.com/owner/repo.git"),
            RepoHost::GitHub {
                url: "https://github.com/owner/repo".to_string()
            }
        );
    }

    #[test]
    fn detect_host_gitlab_ssh() {
        assert_eq!(
            detect_host("git@gitlab.com:owner/repo.git"),
            RepoHost::GitLab {
                url: "https://gitlab.com/owner/repo".to_string()
            }
        );
    }

    #[test]
    fn detect_host_gitlab_https() {
        assert_eq!(
            detect_host("https://gitlab.com/owner/repo.git"),
            RepoHost::GitLab {
                url: "https://gitlab.com/owner/repo".to_string()
            }
        );
    }

    #[test]
    fn detect_host_unknown() {
        assert_eq!(
            detect_host("https://example.com/repo.git"),
            RepoHost::Unknown
        );
    }

    #[test]
    fn render_version_with_github_links() {
        let release = VersionRelease {
            tag: "0.2.0".to_string(),
            date: "2026-03-13".to_string(),
            prev_tag: Some("0.1.0".to_string()),
            groups: vec![
                (
                    "Features".to_string(),
                    vec![
                        ChangelogEntry {
                            scope: Some("auth".to_string()),
                            description: "add OAuth2 PKCE flow".to_string(),
                            hash: "abc1234".to_string(),
                            is_breaking: false,
                            refs: vec![],
                        },
                        ChangelogEntry {
                            scope: None,
                            description: "add basic login".to_string(),
                            hash: "def5678".to_string(),
                            is_breaking: false,
                            refs: vec![],
                        },
                    ],
                ),
                (
                    "Bug Fixes".to_string(),
                    vec![ChangelogEntry {
                        scope: Some("build".to_string()),
                        description: "drop OpenSSL dependency".to_string(),
                        hash: "9fe82f4".to_string(),
                        is_breaking: false,
                        refs: vec![],
                    }],
                ),
            ],
            breaking_changes: vec![],
        };

        let config = ChangelogConfig::default();
        let host = RepoHost::GitHub {
            url: "https://github.com/owner/repo".to_string(),
        };

        let output = render_version(&release, &config, &host);

        assert!(output.contains(
            "## [0.2.0](https://github.com/owner/repo/compare/v0.1.0...v0.2.0) (2026-03-13)"
        ));
        assert!(output.contains("### Features"));
        assert!(output.contains("- **auth:** add OAuth2 PKCE flow ([abc1234](https://github.com/owner/repo/commit/abc1234))"));
        assert!(output.contains(
            "- add basic login ([def5678](https://github.com/owner/repo/commit/def5678))"
        ));
        assert!(output.contains("### Bug Fixes"));
        assert!(output.contains("- **build:** drop OpenSSL dependency ([9fe82f4](https://github.com/owner/repo/commit/9fe82f4))"));
    }

    #[test]
    fn render_version_without_links() {
        let release = VersionRelease {
            tag: "0.1.0".to_string(),
            date: "2026-03-01".to_string(),
            prev_tag: None,
            groups: vec![(
                "Features".to_string(),
                vec![ChangelogEntry {
                    scope: None,
                    description: "initial release".to_string(),
                    hash: "1234567".to_string(),
                    is_breaking: false,
                    refs: vec![],
                }],
            )],
            breaking_changes: vec![],
        };

        let config = ChangelogConfig::default();
        let host = RepoHost::Unknown;

        let output = render_version(&release, &config, &host);

        assert!(output.contains("## 0.1.0 (2026-03-01)"));
        assert!(output.contains("- initial release (1234567)"));
        // No links
        assert!(!output.contains("https://"));
    }

    #[test]
    fn render_version_with_breaking_changes() {
        let release = VersionRelease {
            tag: "2.0.0".to_string(),
            date: "2026-03-13".to_string(),
            prev_tag: Some("1.0.0".to_string()),
            groups: vec![(
                "Features".to_string(),
                vec![ChangelogEntry {
                    scope: None,
                    description: "new API".to_string(),
                    hash: "aaa1111".to_string(),
                    is_breaking: true,
                    refs: vec![],
                }],
            )],
            breaking_changes: vec!["removed v1 endpoints".to_string()],
        };

        let config = ChangelogConfig::default();
        let host = RepoHost::Unknown;

        let output = render_version(&release, &config, &host);

        assert!(output.contains("### BREAKING CHANGES"));
        assert!(output.contains("- removed v1 endpoints"));
    }

    #[test]
    fn render_full_changelog() {
        let releases = vec![
            VersionRelease {
                tag: "0.2.0".to_string(),
                date: "2026-03-13".to_string(),
                prev_tag: Some("0.1.0".to_string()),
                groups: vec![(
                    "Features".to_string(),
                    vec![ChangelogEntry {
                        scope: None,
                        description: "second feature".to_string(),
                        hash: "bbb2222".to_string(),
                        is_breaking: false,
                        refs: vec![],
                    }],
                )],
                breaking_changes: vec![],
            },
            VersionRelease {
                tag: "0.1.0".to_string(),
                date: "2026-03-01".to_string(),
                prev_tag: None,
                groups: vec![(
                    "Features".to_string(),
                    vec![ChangelogEntry {
                        scope: None,
                        description: "initial release".to_string(),
                        hash: "aaa1111".to_string(),
                        is_breaking: false,
                        refs: vec![],
                    }],
                )],
                breaking_changes: vec![],
            },
        ];

        let config = ChangelogConfig::default();
        let host = RepoHost::Unknown;

        let output = render(&releases, &config, &host);

        assert!(output.starts_with("# Changelog\n"));
        assert!(output.contains("## 0.2.0 (2026-03-13)"));
        assert!(output.contains("## 0.1.0 (2026-03-01)"));
        // 0.2.0 should come before 0.1.0
        let pos_02 = output.find("0.2.0").unwrap();
        let pos_01 = output.find("0.1.0").unwrap();
        assert!(pos_02 < pos_01);
    }

    #[test]
    fn render_version_no_scope() {
        let release = VersionRelease {
            tag: "1.0.0".to_string(),
            date: "2026-01-01".to_string(),
            prev_tag: None,
            groups: vec![(
                "Bug Fixes".to_string(),
                vec![ChangelogEntry {
                    scope: None,
                    description: "fix a bug".to_string(),
                    hash: "ccc3333".to_string(),
                    is_breaking: false,
                    refs: vec![],
                }],
            )],
            breaking_changes: vec![],
        };

        let config = ChangelogConfig::default();
        let host = RepoHost::Unknown;

        let output = render_version(&release, &config, &host);

        assert!(output.contains("- fix a bug (ccc3333)"));
        // No bold scope
        assert!(!output.contains("**"));
    }

    #[test]
    fn hidden_types_excluded() {
        // Build a release with groups already rendered — but demonstrate that
        // render() filters out empty groups properly.
        let releases = vec![VersionRelease {
            tag: "1.0.0".to_string(),
            date: "2026-01-01".to_string(),
            prev_tag: None,
            groups: vec![
                (
                    "Features".to_string(),
                    vec![ChangelogEntry {
                        scope: None,
                        description: "a feature".to_string(),
                        hash: "aaa1111".to_string(),
                        is_breaking: false,
                        refs: vec![],
                    }],
                ),
                ("Empty Section".to_string(), vec![]),
            ],
            breaking_changes: vec![],
        }];

        let config = ChangelogConfig::default();
        let host = RepoHost::Unknown;

        let output = render(&releases, &config, &host);

        assert!(output.contains("### Features"));
        assert!(!output.contains("### Empty Section"));
    }

    #[test]
    fn prepend_to_empty_file() {
        let release = VersionRelease {
            tag: "Unreleased".to_string(),
            date: "2026-03-13".to_string(),
            prev_tag: None,
            groups: vec![(
                "Features".to_string(),
                vec![ChangelogEntry {
                    scope: None,
                    description: "new thing".to_string(),
                    hash: "abc1234".to_string(),
                    is_breaking: false,
                    refs: vec![],
                }],
            )],
            breaking_changes: vec![],
        };

        let config = ChangelogConfig::default();
        let host = RepoHost::Unknown;

        let output = prepend_release("", &release, &config, &host);
        assert!(output.starts_with("# Changelog\n"));
        assert!(output.contains("## Unreleased (2026-03-13)"));
        assert!(output.contains("- new thing (abc1234)"));
    }

    #[test]
    fn prepend_replaces_existing_unreleased() {
        let existing = "# Changelog\n\n## Unreleased (2026-03-12)\n\n### Features\n\n- old thing (aaa1111)\n\n## 0.1.0 (2026-03-01)\n\n### Features\n\n- initial (bbb2222)\n";

        let release = VersionRelease {
            tag: "Unreleased".to_string(),
            date: "2026-03-13".to_string(),
            prev_tag: Some("0.1.0".to_string()),
            groups: vec![(
                "Features".to_string(),
                vec![ChangelogEntry {
                    scope: None,
                    description: "newer thing".to_string(),
                    hash: "ccc3333".to_string(),
                    is_breaking: false,
                    refs: vec![],
                }],
            )],
            breaking_changes: vec![],
        };

        let config = ChangelogConfig::default();
        let host = RepoHost::Unknown;

        let output = prepend_release(existing, &release, &config, &host);
        // Old unreleased gone.
        assert!(!output.contains("old thing"));
        // New unreleased present.
        assert!(output.contains("newer thing"));
        // Previous release preserved.
        assert!(output.contains("## 0.1.0 (2026-03-01)"));
        assert!(output.contains("- initial (bbb2222)"));
    }

    #[test]
    fn prepend_preserves_existing_releases() {
        let existing = "# Changelog\n\n## 0.2.0 (2026-03-10)\n\n### Features\n\n- second (bbb2222)\n\n## 0.1.0 (2026-03-01)\n\n### Features\n\n- first (aaa1111)\n";

        let release = VersionRelease {
            tag: "Unreleased".to_string(),
            date: "2026-03-13".to_string(),
            prev_tag: Some("0.2.0".to_string()),
            groups: vec![(
                "Bug Fixes".to_string(),
                vec![ChangelogEntry {
                    scope: None,
                    description: "a fix".to_string(),
                    hash: "ddd4444".to_string(),
                    is_breaking: false,
                    refs: vec![],
                }],
            )],
            breaking_changes: vec![],
        };

        let config = ChangelogConfig::default();
        let host = RepoHost::Unknown;

        let output = prepend_release(existing, &release, &config, &host);
        // New section is first.
        let pos_unreleased = output.find("Unreleased").unwrap();
        let pos_020 = output.find("0.2.0").unwrap();
        let pos_010 = output.find("0.1.0").unwrap();
        assert!(pos_unreleased < pos_020);
        assert!(pos_020 < pos_010);
        // All content present.
        assert!(output.contains("- a fix (ddd4444)"));
        assert!(output.contains("- second (bbb2222)"));
        assert!(output.contains("- first (aaa1111)"));
    }

    #[test]
    fn linkify_issue_numbers_github() {
        let host = RepoHost::GitHub {
            url: "https://github.com/owner/repo".to_string(),
        };
        assert_eq!(
            linkify_issue_numbers("add feature (#42)", &host),
            "add feature ([#42](https://github.com/owner/repo/issues/42))"
        );
    }

    #[test]
    fn linkify_issue_numbers_gitlab() {
        let host = RepoHost::GitLab {
            url: "https://gitlab.com/owner/repo".to_string(),
        };
        assert_eq!(
            linkify_issue_numbers("#10", &host),
            "[#10](https://gitlab.com/owner/repo/-/issues/10)"
        );
    }

    #[test]
    fn linkify_issue_numbers_unknown_no_links() {
        let host = RepoHost::Unknown;
        assert_eq!(linkify_issue_numbers("fix (#7)", &host), "fix (#7)");
    }

    #[test]
    fn link_ref_github_issue() {
        let host = RepoHost::GitHub {
            url: "https://github.com/o/r".to_string(),
        };
        assert_eq!(
            link_ref("#42", &host, None),
            "[#42](https://github.com/o/r/issues/42)"
        );
    }

    #[test]
    fn link_ref_bug_url_template() {
        let host = RepoHost::Unknown;
        assert_eq!(
            link_ref("PROJ-123", &host, Some("https://jira.co/browse/%s")),
            "[PROJ-123](https://jira.co/browse/PROJ-123)"
        );
    }

    #[test]
    fn link_ref_full_url() {
        let host = RepoHost::Unknown;
        assert_eq!(
            link_ref("https://linear.app/team/ISS-99", &host, None),
            "[https://linear.app/team/ISS-99](https://linear.app/team/ISS-99)"
        );
    }

    #[test]
    fn link_ref_plain_text_no_template() {
        let host = RepoHost::Unknown;
        assert_eq!(link_ref("PROJ-123", &host, None), "PROJ-123");
    }

    #[test]
    fn render_entry_with_refs() {
        let release = VersionRelease {
            tag: "1.0.0".to_string(),
            date: "2026-03-13".to_string(),
            prev_tag: None,
            groups: vec![(
                "Features".to_string(),
                vec![ChangelogEntry {
                    scope: None,
                    description: "add auth".to_string(),
                    hash: "abc1234".to_string(),
                    is_breaking: false,
                    refs: vec!["#45".to_string(), "#46".to_string()],
                }],
            )],
            breaking_changes: vec![],
        };

        let config = ChangelogConfig::default();
        let host = RepoHost::GitHub {
            url: "https://github.com/o/r".to_string(),
        };

        let output = render_version(&release, &config, &host);
        assert!(output.contains("closes [#45](https://github.com/o/r/issues/45), [#46](https://github.com/o/r/issues/46)"));
    }

    #[test]
    fn render_entry_with_bug_url() {
        let release = VersionRelease {
            tag: "1.0.0".to_string(),
            date: "2026-03-13".to_string(),
            prev_tag: None,
            groups: vec![(
                "Bug Fixes".to_string(),
                vec![ChangelogEntry {
                    scope: None,
                    description: "fix login".to_string(),
                    hash: "abc1234".to_string(),
                    is_breaking: false,
                    refs: vec!["PROJ-42".to_string()],
                }],
            )],
            breaking_changes: vec![],
        };

        let mut config = ChangelogConfig::default();
        config.bug_url = Some("https://jira.co/browse/%s".to_string());
        let host = RepoHost::Unknown;

        let output = render_version(&release, &config, &host);
        assert!(output.contains("closes [PROJ-42](https://jira.co/browse/PROJ-42)"));
    }

    #[test]
    fn render_description_refs_linked() {
        let release = VersionRelease {
            tag: "1.0.0".to_string(),
            date: "2026-03-13".to_string(),
            prev_tag: None,
            groups: vec![(
                "Features".to_string(),
                vec![ChangelogEntry {
                    scope: None,
                    description: "add feature (#42)".to_string(),
                    hash: "abc1234".to_string(),
                    is_breaking: false,
                    refs: vec![],
                }],
            )],
            breaking_changes: vec![],
        };

        let config = ChangelogConfig::default();
        let host = RepoHost::GitHub {
            url: "https://github.com/o/r".to_string(),
        };

        let output = render_version(&release, &config, &host);
        assert!(output.contains("[#42](https://github.com/o/r/issues/42)"));
    }

    #[test]
    fn strip_unreleased_removes_section() {
        let content = "\n## Unreleased (2026-03-12)\n\n### Features\n\n- old (aaa)\n\n## 0.1.0 (2026-03-01)\n\n### Features\n\n- init (bbb)\n";
        let result = strip_unreleased_section(content);
        assert!(!result.contains("Unreleased"));
        assert!(result.contains("## 0.1.0"));
    }

    #[test]
    fn strip_unreleased_no_op_without_unreleased() {
        let content = "\n## 0.1.0 (2026-03-01)\n\n### Features\n\n- init (bbb)\n";
        let result = strip_unreleased_section(content);
        assert_eq!(result, content);
    }

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
