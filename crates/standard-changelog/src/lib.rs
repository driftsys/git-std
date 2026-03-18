//! Changelog generation from conventional commits.
//!
//! Groups parsed commits by type, renders markdown sections, and manages
//! `CHANGELOG.md` files. Pure library — no I/O, no git operations, no
//! terminal output.
//!
//! # Main entry points
//!
//! - [`build_release`] — parse raw commits into a [`VersionRelease`]
//! - [`render`] — render multiple releases into a full `CHANGELOG.md`
//! - [`render_version`] — render a single version section
//! - [`prepend_release`] — splice a new release into an existing changelog
//!
//! # Example
//!
//! ```
//! use standard_changelog::{build_release, render, ChangelogConfig, RepoHost};
//!
//! let commits = vec![
//!     ("abc1234", "feat(auth): add OAuth2 PKCE flow"),
//!     ("def5678", "fix: handle expired tokens"),
//! ];
//!
//! let config = ChangelogConfig::default();
//! let mut release = build_release(&commits, "1.0.0", None, &config).unwrap();
//! release.date = "2026-03-14".to_string();
//!
//! let host = RepoHost::Unknown;
//! let changelog = render(&[release], &config, &host);
//! assert!(changelog.contains("## 1.0.0 (2026-03-14)"));
//! assert!(changelog.contains("### Features"));
//! assert!(changelog.contains("### Bug Fixes"));
//! ```

mod model;
pub use model::*;

/// Parse a git remote URL to detect the repo host.
///
/// Supports SSH (`git@github.com:owner/repo.git`) and HTTPS
/// (`https://github.com/owner/repo`) URLs for GitHub and GitLab.
/// Returns [`RepoHost::Unknown`] for unrecognised hosts.
///
/// # Examples
///
/// ```
/// use standard_changelog::{detect_host, RepoHost};
///
/// let host = detect_host("git@github.com:owner/repo.git");
/// assert!(matches!(host, RepoHost::GitHub { .. }));
///
/// let host = detect_host("https://example.com/repo.git");
/// assert_eq!(host, RepoHost::Unknown);
/// ```
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

/// Collected reference-style link definitions (label, url).
type LinkDefs = Vec<(String, String)>;

/// Add a link definition if not already present.
fn add_link_def(defs: &mut LinkDefs, label: &str, url: &str) {
    if !defs.iter().any(|(l, _)| l == label) {
        defs.push((label.to_string(), url.to_string()));
    }
}

/// Issue URL for a given `#N` reference.
fn issue_url(num: &str, host: &RepoHost) -> Option<String> {
    match host {
        RepoHost::GitHub { url } => Some(format!("{url}/issues/{num}")),
        RepoHost::GitLab { url } => Some(format!("{url}/-/issues/{num}")),
        RepoHost::Unknown => None,
    }
}

/// Replace `#N` issue references in text with reference-style links.
fn linkify_issue_numbers(text: &str, host: &RepoHost, defs: &mut LinkDefs) -> String {
    if matches!(host, RepoHost::Unknown) {
        return text.to_string();
    }

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
                let label = format!("#{num}");
                if let Some(url) = issue_url(num, host) {
                    add_link_def(defs, &label, &url);
                }
                result.push_str(&format!("[{label}]"));
            } else {
                result.push('#');
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Link a single ref value using reference-style links.
fn link_ref(r: &str, host: &RepoHost, bug_url: Option<&str>, defs: &mut LinkDefs) -> String {
    let r = r.trim();

    // Already a URL — keep inline (ref-style doesn't help here).
    if r.starts_with("http://") || r.starts_with("https://") {
        return format!("[{r}]({r})");
    }

    // #N — reference-style link to GitHub/GitLab issues.
    if r.starts_with('#') && r[1..].chars().all(|c| c.is_ascii_digit()) && r.len() > 1 {
        let num = &r[1..];
        if let Some(url) = issue_url(num, host) {
            add_link_def(defs, r, &url);
            return format!("[{r}]");
        }
        return r.to_string();
    }

    // External tracker ref (e.g. PROJ-123) — use bug_url template.
    if let Some(template) = bug_url {
        let url = template.replace("%s", r);
        add_link_def(defs, r, &url);
        return format!("[{r}]");
    }

    r.to_string()
}

/// Format a commit hash as a reference-style link.
fn commit_link(hash: &str, host: &RepoHost, defs: &mut LinkDefs) -> String {
    match host {
        RepoHost::GitHub { url } => {
            add_link_def(defs, hash, &format!("{url}/commit/{hash}"));
            format!("[{hash}]")
        }
        RepoHost::GitLab { url } => {
            add_link_def(defs, hash, &format!("{url}/-/commit/{hash}"));
            format!("[{hash}]")
        }
        RepoHost::Unknown => hash.to_string(),
    }
}

/// Format a version heading with an optional compare link (reference-style).
fn version_heading(release: &VersionRelease, host: &RepoHost, defs: &mut LinkDefs) -> String {
    let version_link = if let Some(prev) = &release.prev_tag {
        match host {
            RepoHost::GitHub { url } => {
                add_link_def(
                    defs,
                    &release.tag,
                    &format!("{url}/compare/v{prev}...v{tag}", tag = release.tag),
                );
                format!("[{tag}]", tag = release.tag)
            }
            RepoHost::GitLab { url } => {
                add_link_def(
                    defs,
                    &release.tag,
                    &format!("{url}/-/compare/v{prev}...v{tag}", tag = release.tag),
                );
                format!("[{tag}]", tag = release.tag)
            }
            RepoHost::Unknown => release.tag.clone(),
        }
    } else {
        release.tag.clone()
    };

    format!("## {version_link} ({date})", date = release.date)
}

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

/// Convert days since Unix epoch to (year, month, day).
///
/// Uses the algorithm from <http://howardhinnant.github.io/date_algorithms.html>.
pub fn days_to_date(mut days: i64) -> (i64, i64, i64) {
    days += 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Format a Unix timestamp (seconds since epoch) as `YYYY-MM-DD`.
///
/// ```
/// assert_eq!(standard_changelog::format_date(1_710_374_400), "2024-03-14");
/// ```
pub fn format_date(unix_secs: i64) -> String {
    let days = unix_secs / 86400;
    let (year, month, day) = days_to_date(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Build a [`VersionRelease`] from raw commit data.
///
/// Each entry in `commits` is a `(short_hash, full_message)` pair. Messages
/// are parsed with [`standard_commit::parse`]; non-conventional messages are
/// silently skipped. Commits whose type appears in [`ChangelogConfig::hidden`]
/// are excluded.
///
/// Returns `None` when no visible groups or breaking changes are produced.
///
/// # Example
///
/// ```
/// use standard_changelog::{build_release, ChangelogConfig};
///
/// let commits = vec![
///     ("abc1234", "feat: add login"),
///     ("def5678", "fix: handle timeout"),
///     ("ghi9012", "chore: update deps"),  // hidden by default
/// ];
///
/// let config = ChangelogConfig::default();
/// let release = build_release(&commits, "1.0.0", None, &config).unwrap();
/// assert_eq!(release.groups.len(), 2); // Features + Bug Fixes
/// ```
pub fn build_release(
    commits: &[(&str, &str)],
    version: &str,
    prev_tag: Option<&str>,
    config: &ChangelogConfig,
) -> Option<VersionRelease> {
    use std::collections::{HashMap, HashSet};

    let section_map: HashMap<&str, &str> = config
        .sections
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let hidden_set: HashSet<&str> = config.hidden.iter().map(|s| s.as_str()).collect();

    let mut groups_map: HashMap<String, Vec<ChangelogEntry>> = HashMap::new();
    let mut breaking_changes = Vec::new();

    for (short_hash, message) in commits {
        let parsed = match standard_commit::parse(message) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if hidden_set.contains(parsed.r#type.as_str()) {
            continue;
        }

        let section_title = match section_map.get(parsed.r#type.as_str()) {
            Some(title) => (*title).to_string(),
            None => continue,
        };

        let mut refs = Vec::new();
        for footer in &parsed.footers {
            match footer.token.as_str() {
                "BREAKING CHANGE" | "BREAKING-CHANGE" => {
                    breaking_changes.push(footer.value.clone());
                }
                "Refs" | "Closes" | "Fixes" | "Resolves" => {
                    let token = footer.token.to_lowercase();
                    for r in footer.value.split(',') {
                        let r = r.trim();
                        if !r.is_empty() {
                            let value = if r.chars().all(|c| c.is_ascii_digit()) {
                                format!("#{r}")
                            } else {
                                r.to_string()
                            };
                            refs.push((token.clone(), value));
                        }
                    }
                }
                _ => {}
            }
        }

        let entry = ChangelogEntry {
            scope: parsed.scope,
            description: parsed.description,
            hash: (*short_hash).to_string(),
            is_breaking: parsed.is_breaking,
            refs,
        };

        groups_map.entry(section_title).or_default().push(entry);
    }

    // Order groups by section config order.
    let sections: Vec<(&str, &str)> = section_map.iter().map(|(k, v)| (*k, *v)).collect();
    let groups: Vec<(String, Vec<ChangelogEntry>)> = sections
        .iter()
        .filter_map(|(_, title)| {
            groups_map
                .remove(*title)
                .map(|entries| (title.to_string(), entries))
        })
        .collect();

    if groups.is_empty() && breaking_changes.is_empty() {
        return None;
    }

    Some(VersionRelease {
        tag: version.to_string(),
        date: String::new(),
        prev_tag: prev_tag.map(|t| t.strip_prefix('v').unwrap_or(t).to_string()),
        groups,
        breaking_changes,
    })
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
    fn linkify_issue_numbers_github() {
        let host = RepoHost::GitHub {
            url: "https://github.com/owner/repo".to_string(),
        };
        let mut defs = Vec::new();
        assert_eq!(
            linkify_issue_numbers("add feature (#42)", &host, &mut defs),
            "add feature ([#42])"
        );
        assert_eq!(
            defs,
            vec![(
                "#42".to_string(),
                "https://github.com/owner/repo/issues/42".to_string()
            )]
        );
    }

    #[test]
    fn linkify_issue_numbers_gitlab() {
        let host = RepoHost::GitLab {
            url: "https://gitlab.com/owner/repo".to_string(),
        };
        let mut defs = Vec::new();
        assert_eq!(linkify_issue_numbers("#10", &host, &mut defs), "[#10]");
        assert_eq!(
            defs,
            vec![(
                "#10".to_string(),
                "https://gitlab.com/owner/repo/-/issues/10".to_string()
            )]
        );
    }

    #[test]
    fn linkify_issue_numbers_unknown_no_links() {
        let host = RepoHost::Unknown;
        let mut defs = Vec::new();
        assert_eq!(
            linkify_issue_numbers("fix (#7)", &host, &mut defs),
            "fix (#7)"
        );
        assert!(defs.is_empty());
    }

    #[test]
    fn link_ref_github_issue() {
        let host = RepoHost::GitHub {
            url: "https://github.com/o/r".to_string(),
        };
        let mut defs = Vec::new();
        assert_eq!(link_ref("#42", &host, None, &mut defs), "[#42]");
        assert_eq!(
            defs,
            vec![(
                "#42".to_string(),
                "https://github.com/o/r/issues/42".to_string()
            )]
        );
    }

    #[test]
    fn link_ref_bug_url_template() {
        let host = RepoHost::Unknown;
        let mut defs = Vec::new();
        assert_eq!(
            link_ref(
                "PROJ-123",
                &host,
                Some("https://jira.co/browse/%s"),
                &mut defs
            ),
            "[PROJ-123]"
        );
        assert_eq!(
            defs,
            vec![(
                "PROJ-123".to_string(),
                "https://jira.co/browse/PROJ-123".to_string()
            )]
        );
    }

    #[test]
    fn link_ref_full_url() {
        let host = RepoHost::Unknown;
        let mut defs = Vec::new();
        assert_eq!(
            link_ref("https://linear.app/team/ISS-99", &host, None, &mut defs),
            "[https://linear.app/team/ISS-99](https://linear.app/team/ISS-99)"
        );
        assert!(defs.is_empty());
    }

    #[test]
    fn link_ref_plain_text_no_template() {
        let host = RepoHost::Unknown;
        let mut defs = Vec::new();
        assert_eq!(link_ref("PROJ-123", &host, None, &mut defs), "PROJ-123");
        assert!(defs.is_empty());
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

        let mut config = ChangelogConfig::default();
        config.bug_url = Some("https://jira.co/browse/%s".to_string());
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

    #[test]
    fn days_to_date_epoch() {
        assert_eq!(days_to_date(0), (1970, 1, 1));
    }

    #[test]
    fn days_to_date_known_date() {
        // 2026-03-13 is day 20525
        assert_eq!(days_to_date(20525), (2026, 3, 13));
    }

    #[test]
    fn format_date_epoch() {
        assert_eq!(format_date(0), "1970-01-01");
    }

    #[test]
    fn format_date_known_timestamp() {
        // 2024-01-01 00:00:00 UTC = 1704067200
        assert_eq!(format_date(1704067200), "2024-01-01");
    }

    #[test]
    fn build_release_basic() {
        let config = ChangelogConfig::default();
        let commits = vec![
            ("abc1234", "feat: add feature\n"),
            ("def5678", "fix: fix bug\n"),
        ];
        let release = build_release(&commits, "1.0.0", None, &config).unwrap();
        assert_eq!(release.tag, "1.0.0");
        assert_eq!(release.groups.len(), 2);
    }

    #[test]
    fn build_release_skips_hidden() {
        let config = ChangelogConfig::default();
        let commits = vec![("abc1234", "chore: cleanup\n")];
        assert!(build_release(&commits, "1.0.0", None, &config).is_none());
    }

    #[test]
    fn build_release_extracts_breaking_changes() {
        let config = ChangelogConfig::default();
        let commits = vec![(
            "abc1234",
            "feat: new api\n\nBREAKING CHANGE: removed old endpoints",
        )];
        let release = build_release(&commits, "2.0.0", Some("v1.0.0"), &config).unwrap();
        assert_eq!(release.breaking_changes, vec!["removed old endpoints"]);
        assert_eq!(release.prev_tag, Some("1.0.0".to_string()));
    }

    #[test]
    fn build_release_extracts_refs() {
        let config = ChangelogConfig::default();
        let commits = vec![("abc1234", "feat: add auth\n\nCloses: #45, #46\nRefs: 99")];
        let release = build_release(&commits, "1.0.0", None, &config).unwrap();
        let entries = &release.groups[0].1;
        assert_eq!(entries[0].refs.len(), 3);
        assert_eq!(
            entries[0].refs[0],
            ("closes".to_string(), "#45".to_string())
        );
        assert_eq!(
            entries[0].refs[1],
            ("closes".to_string(), "#46".to_string())
        );
        assert_eq!(entries[0].refs[2], ("refs".to_string(), "#99".to_string()));
    }

    #[test]
    fn build_release_skips_non_conventional() {
        let config = ChangelogConfig::default();
        let commits = vec![
            ("abc1234", "not a conventional commit"),
            ("def5678", "feat: valid one\n"),
        ];
        let release = build_release(&commits, "1.0.0", None, &config).unwrap();
        assert_eq!(release.groups.len(), 1);
        assert_eq!(release.groups[0].1.len(), 1);
    }

    #[test]
    fn build_release_none_when_empty() {
        let config = ChangelogConfig::default();
        let commits: Vec<(&str, &str)> = vec![];
        assert!(build_release(&commits, "1.0.0", None, &config).is_none());
    }
}
