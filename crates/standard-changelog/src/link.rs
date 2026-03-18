//! Link-building utilities for changelog rendering.
//!
//! Provides reference-style link generation for commit hashes, issue numbers,
//! external tracker references, and version comparison headings.

use crate::{RepoHost, VersionRelease};

/// Collected reference-style link definitions (label, url).
pub(crate) type LinkDefs = Vec<(String, String)>;

/// Add a link definition if not already present.
pub(crate) fn add_link_def(defs: &mut LinkDefs, label: &str, url: &str) {
    if !defs.iter().any(|(l, _)| l == label) {
        defs.push((label.to_string(), url.to_string()));
    }
}

/// Issue URL for a given `#N` reference.
pub(crate) fn issue_url(num: &str, host: &RepoHost) -> Option<String> {
    match host {
        RepoHost::GitHub { url } => Some(format!("{url}/issues/{num}")),
        RepoHost::GitLab { url } => Some(format!("{url}/-/issues/{num}")),
        RepoHost::Unknown => None,
    }
}

/// Replace `#N` issue references in text with reference-style links.
pub(crate) fn linkify_issue_numbers(text: &str, host: &RepoHost, defs: &mut LinkDefs) -> String {
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
pub(crate) fn link_ref(
    r: &str,
    host: &RepoHost,
    bug_url: Option<&str>,
    defs: &mut LinkDefs,
) -> String {
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
pub(crate) fn commit_link(hash: &str, host: &RepoHost, defs: &mut LinkDefs) -> String {
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
pub(crate) fn version_heading(
    release: &VersionRelease,
    host: &RepoHost,
    defs: &mut LinkDefs,
) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RepoHost;

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
}
