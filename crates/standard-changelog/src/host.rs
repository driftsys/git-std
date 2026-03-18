use crate::RepoHost;

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
}
