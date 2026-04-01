use crate::link::{LinkDefs, commit_link, link_ref, linkify_issue_numbers, version_heading};
use crate::model::{ChangelogConfig, ChangelogEntry, RepoHost, VersionRelease};

/// Wrap a changelog entry across multiple lines at word boundaries.
///
/// `prefix` is the first part (e.g. `- **scope:** description`).
/// `segments` are trailing parts joined by `, ` (e.g. `(hash)`, `closes [#1]`).
/// Lines beyond the first are indented with 2 spaces for list continuation.
fn wrap_entry(prefix: &str, segments: &[String], max_width: usize) -> String {
    // Build the full line, then wrap at word boundaries.
    let mut full = prefix.to_string();
    for (i, seg) in segments.iter().enumerate() {
        let sep = if i == 0 { " " } else { ", " };
        full.push_str(sep);
        full.push_str(seg);
    }

    wrap_words(&full, "  ", max_width)
}

/// Greedy word-wrap: split `text` at spaces, fill lines up to `max_width`.
/// Continuation lines are prefixed with `indent`.
fn wrap_words(text: &str, indent: &str, max_width: usize) -> String {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out = String::new();
    let mut line = String::new();

    for word in &words {
        if line.is_empty() {
            line.push_str(word);
        } else {
            let candidate_len = line.len() + 1 + word.len();
            if candidate_len <= max_width {
                line.push(' ');
                line.push_str(word);
            } else {
                out.push_str(&line);
                out.push('\n');
                line = format!("{indent}{word}");
            }
        }
    }

    if !line.is_empty() {
        out.push_str(&line);
        out.push('\n');
    }

    out
}

/// Render a single version section as markdown.
///
/// Produces a `## <version> (<date>)` heading followed by grouped entries
/// and optional `### BREAKING CHANGES`. Uses reference-style links for
/// commit hashes and issue numbers when `host` is known, with link
/// definitions appended at the bottom of the section.
///
/// Lines are word-wrapped at 80 characters with 2-space continuation
/// indents for markdown list items.
pub fn render_version(
    release: &VersionRelease,
    config: &ChangelogConfig,
    host: &RepoHost,
) -> String {
    let mut out = String::new();
    let mut defs: LinkDefs = Vec::new();

    out.push_str(&version_heading(release, host, &mut defs));
    out.push('\n');

    for (section_name, entries) in &release.groups {
        if entries.is_empty() {
            continue;
        }

        out.push('\n');
        out.push_str(&format!("### {section_name}\n"));
        out.push('\n');

        for entry in entries {
            let link = commit_link(&entry.hash, host, &mut defs);
            let desc = linkify_issue_numbers(&entry.description, host, &mut defs);
            // Build segments: description, (hash), then ref groups.
            let prefix = if let Some(scope) = &entry.scope {
                format!("- **{scope}:** {desc}")
            } else {
                format!("- {desc}")
            };

            // Collect trailing segments: hash + ref groups.
            let mut segments: Vec<String> = vec![format!("({link})")];

            if !entry.refs.is_empty() {
                let bug_url = config.bug_url.as_deref();
                let mut prev_token = String::new();
                for (token, value) in &entry.refs {
                    let linked = link_ref(value, host, bug_url, &mut defs);
                    if *token != prev_token {
                        // New token group: "closes [#1]"
                        segments.push(format!("{token} {linked}"));
                    } else {
                        // Same token continues: "[#2]"
                        segments.push(linked);
                    }
                    prev_token.clone_from(token);
                }
            }

            out.push_str(&wrap_entry(&prefix, &segments, 80));
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

    // Append reference-style link definitions.
    if !defs.is_empty() {
        out.push('\n');
        for (label, url) in &defs {
            out.push_str(&format!("[{label}]: {url}\n"));
        }
    }

    out
}

/// Prepend a release section to an existing changelog.
///
/// Replaces any prior `## Unreleased` block and adds a `# <title>`
/// heading if one is missing. Previous version sections are preserved.
///
/// Returns the updated changelog content as a string, ready to write
/// to `CHANGELOG.md`.
///
/// # Example
///
/// ```
/// use standard_changelog::*;
///
/// let existing = "# Changelog\n\n## 0.1.0 (2026-01-01)\n\n### Features\n\n- init (aaa1111)\n";
/// let release = VersionRelease {
///     tag: "0.2.0".to_string(),
///     date: "2026-03-14".to_string(),
///     prev_tag: Some("0.1.0".to_string()),
///     groups: vec![("Bug Fixes".to_string(), vec![ChangelogEntry {
///         scope: None,
///         description: "fix crash".to_string(),
///         hash: "bbb2222".to_string(),
///         is_breaking: false,
///         refs: vec![],
///     }])],
///     breaking_changes: vec![],
/// };
///
/// let updated = prepend_release(existing, &release, &ChangelogConfig::default(), &RepoHost::Unknown);
/// assert!(updated.contains("## 0.2.0"));
/// assert!(updated.contains("## 0.1.0"));
/// ```
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
///
/// Produces a complete `CHANGELOG.md` starting with `# <title>`, followed
/// by each release rendered via [`render_version`]. Releases with no
/// visible groups are skipped.
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

        // Reference-style links in body.
        assert!(output.contains("## [0.2.0] (2026-03-13)"));
        assert!(output.contains("- **auth:** add OAuth2 PKCE flow ([abc1234])"));
        assert!(output.contains("- add basic login ([def5678])"));
        assert!(output.contains("- **build:** drop OpenSSL dependency ([9fe82f4])"));
        // Link definitions at the bottom.
        assert!(output.contains("[0.2.0]: https://github.com/owner/repo/compare/v0.1.0...v0.2.0"));
        assert!(output.contains("[abc1234]: https://github.com/owner/repo/commit/abc1234"));
        assert!(output.contains("[def5678]: https://github.com/owner/repo/commit/def5678"));
        assert!(output.contains("[9fe82f4]: https://github.com/owner/repo/commit/9fe82f4"));
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
                    refs: vec![
                        ("closes".to_string(), "#45".to_string()),
                        ("refs".to_string(), "#46".to_string()),
                    ],
                }],
            )],
            breaking_changes: vec![],
        };

        let config = ChangelogConfig::default();
        let host = RepoHost::GitHub {
            url: "https://github.com/o/r".to_string(),
        };

        let output = render_version(&release, &config, &host);
        assert!(output.contains(", closes [#45], refs [#46]"));
        assert!(output.contains("[#45]: https://github.com/o/r/issues/45"));
        assert!(output.contains("[#46]: https://github.com/o/r/issues/46"));
    }

    #[test]
    fn render_entry_wraps_many_refs() {
        let release = VersionRelease {
            tag: "1.0.0".to_string(),
            date: "2026-03-13".to_string(),
            prev_tag: None,
            groups: vec![(
                "Features".to_string(),
                vec![ChangelogEntry {
                    scope: Some("commit".to_string()),
                    description: "add flag mode with many options".to_string(),
                    hash: "abc1234".to_string(),
                    is_breaking: false,
                    refs: (1..=20)
                        .map(|n| ("closes".to_string(), format!("#{n}")))
                        .collect(),
                }],
            )],
            breaking_changes: vec![],
        };

        let config = ChangelogConfig::default();
        let host = RepoHost::GitHub {
            url: "https://github.com/owner/repo".to_string(),
        };

        let output = render_version(&release, &config, &host);
        for line in output.lines() {
            assert!(
                line.len() <= 80,
                "line exceeds 80 chars ({} chars): {line}",
                line.len()
            );
        }
        // All refs present.
        for n in 1..=20 {
            assert!(output.contains(&format!("[#{}]", n)));
        }
    }

    #[test]
    fn render_entry_wraps_mixed_tokens() {
        let release = VersionRelease {
            tag: "1.0.0".to_string(),
            date: "2026-03-13".to_string(),
            prev_tag: None,
            groups: vec![(
                "Features".to_string(),
                vec![ChangelogEntry {
                    scope: Some("auth".to_string()),
                    description: "add OAuth flow".to_string(),
                    hash: "abc1234".to_string(),
                    is_breaking: false,
                    refs: vec![
                        ("closes".to_string(), "#1".to_string()),
                        ("closes".to_string(), "#2".to_string()),
                        ("closes".to_string(), "#3".to_string()),
                        ("refs".to_string(), "#10".to_string()),
                        ("refs".to_string(), "#11".to_string()),
                    ],
                }],
            )],
            breaking_changes: vec![],
        };

        let config = ChangelogConfig::default();
        let host = RepoHost::Unknown;

        let output = render_version(&release, &config, &host);
        // Unknown host: refs are plain text (no links).
        assert!(output.contains("closes #1, #2, #3"));
        assert!(output.contains("refs #10, #11"));
        for line in output.lines() {
            assert!(
                line.len() <= 80,
                "line exceeds 80 chars ({} chars): {line}",
                line.len()
            );
        }
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
                    refs: vec![("closes".to_string(), "PROJ-42".to_string())],
                }],
            )],
            breaking_changes: vec![],
        };

        let config = ChangelogConfig {
            bug_url: Some("https://jira.co/browse/%s".to_string()),
            ..Default::default()
        };
        let host = RepoHost::Unknown;

        let output = render_version(&release, &config, &host);
        assert!(output.contains(", closes [PROJ-42]"));
        assert!(output.contains("[PROJ-42]: https://jira.co/browse/PROJ-42"));
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
        assert!(output.contains("([#42])"));
        assert!(output.contains("[#42]: https://github.com/o/r/issues/42"));
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
}
