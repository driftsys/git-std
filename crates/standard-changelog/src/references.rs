//! Reference-only prefixes of conventional commit footer values.

/// Extract a leading list of references, stopping before prose or a blank line.
pub(crate) fn extract_references(value: &str) -> Vec<String> {
    let mut references = Vec::new();
    for line in value.lines() {
        if line.trim().is_empty() {
            break;
        }
        for token in line
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
        {
            let token = if is_url(token) {
                token
            } else {
                token.trim_end_matches(['.', ';'])
            };
            if digits(token) {
                references.push(format!("#{token}"));
            } else if token.strip_prefix('#').is_some_and(digits)
                || is_qualified_reference(token)
                || is_tracker_reference(token)
                || is_url(token)
            {
                references.push(token.to_string());
            } else {
                return references;
            }
        }
    }
    references
}

fn digits(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
}

/// Whether the entire value names an issue in another repository.
pub(crate) fn is_qualified_reference(value: &str) -> bool {
    let Some((repository, number)) = value.rsplit_once('#') else {
        return false;
    };
    repository.contains('/')
        && digits(number)
        && repository.split('/').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        })
}

fn is_tracker_reference(value: &str) -> bool {
    let Some((project, number)) = value.rsplit_once('-') else {
        return false;
    };
    project.starts_with(|c: char| c.is_ascii_alphabetic())
        && project
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        && digits(number)
}

fn is_url(value: &str) -> bool {
    value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
        .is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_targets_keep_their_path_punctuation() {
        assert_eq!(
            extract_references("https://tracker.test/ticket/12; #78"),
            ["https://tracker.test/ticket/12;", "#78"]
        );
        assert_eq!(
            extract_references("https://tracker.test/ticket/12."),
            ["https://tracker.test/ticket/12."]
        );
    }

    #[test]
    fn punctuation_and_invalid_references_have_explicit_boundaries() {
        assert_eq!(
            extract_references("#12, #34.\nPROJ-56;"),
            ["#12", "#34", "PROJ-56"]
        );
        for invalid in [
            "#",
            "#12abc",
            "99problems",
            "owner//#12",
            "PROJ-",
            "https://",
            "café#1",
        ] {
            assert!(extract_references(invalid).is_empty(), "{invalid}");
        }
    }
}
