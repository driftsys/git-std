use anyhow::{Result, ensure};
use std::collections::BTreeSet;

type ReleaseIdentity = Vec<(Option<String>, String)>;

pub(super) fn parse_release_subject(subject: &str, monorepo: bool) -> Result<ReleaseIdentity> {
    let subject = if let Some((start, suffix)) = subject.rsplit_once(" (#") {
        if suffix
            .strip_suffix(')')
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        {
            start
        } else {
            subject
        }
    } else {
        subject
    };
    let payload = subject
        .strip_prefix("chore(release): ")
        .ok_or_else(|| anyhow::anyhow!("target is not a chore(release) commit"))?;
    ensure!(!payload.is_empty(), "release subject has no version");
    if !monorepo {
        return Ok(vec![(None, payload.into())]);
    }
    let mut entries = Vec::new();
    let mut names = BTreeSet::new();
    for (index, entry) in payload.split(", ").enumerate() {
        let (name, version) = if let Some((name, version)) = entry.rsplit_once('@') {
            ensure!(!name.is_empty(), "empty release package");
            (Some(name.to_string()), version)
        } else {
            ensure!(index == 0, "root release must be first");
            (
                None,
                entry
                    .strip_prefix('v')
                    .ok_or_else(|| anyhow::anyhow!("invalid root release entry"))?,
            )
        };
        ensure!(
            !version.is_empty() && !version.chars().any(char::is_whitespace),
            "invalid release version"
        );
        ensure!(names.insert(name.clone()), "duplicate release entry");
        entries.push((name, version.into()));
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_version_and_squash_suffix() {
        assert_eq!(
            parse_release_subject("chore(release): 1.2.3-rc.1+build (#45)", false).unwrap(),
            vec![(None, "1.2.3-rc.1+build".into())]
        );
    }

    #[test]
    fn monorepo_root_and_scoped_packages() {
        assert_eq!(
            parse_release_subject("chore(release): v2.0.0, @scope/core@1.2.3, api@4.0.0", true)
                .unwrap(),
            vec![
                (None, "2.0.0".into()),
                (Some("@scope/core".into()), "1.2.3".into()),
                (Some("api".into()), "4.0.0".into())
            ]
        );
    }

    #[test]
    fn malformed_or_duplicate_identity_is_rejected() {
        for subject in [
            "feat: release 1.0.0",
            "chore(release): ",
            "chore(release): api@1.0.0, api@2.0.0",
            "chore(release): api@1.0.0, v2.0.0",
        ] {
            assert!(parse_release_subject(subject, true).is_err(), "{subject}");
        }
    }
}
