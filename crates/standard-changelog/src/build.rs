use crate::{ChangelogConfig, ChangelogEntry, VersionRelease};

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
