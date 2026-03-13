use crate::parse::ConventionalCommit;

/// Format a [`ConventionalCommit`] back into a well-formed conventional commit message string.
pub fn format(commit: &ConventionalCommit) -> String {
    let mut msg = String::new();

    // Header: type[(scope)][!]: description
    msg.push_str(&commit.r#type);
    if let Some(scope) = &commit.scope {
        msg.push('(');
        msg.push_str(scope);
        msg.push(')');
    }
    if commit.is_breaking {
        msg.push('!');
    }
    msg.push_str(": ");
    msg.push_str(&commit.description);

    // Body
    if let Some(body) = &commit.body {
        msg.push_str("\n\n");
        msg.push_str(body);
    }

    // Footers
    if !commit.footers.is_empty() {
        msg.push_str("\n\n");
        for (i, footer) in commit.footers.iter().enumerate() {
            if i > 0 {
                msg.push('\n');
            }
            msg.push_str(&footer.token);
            msg.push_str(": ");
            msg.push_str(&footer.value);
        }
    }

    msg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::Footer;

    #[test]
    fn minimal() {
        let commit = ConventionalCommit {
            r#type: "feat".into(),
            scope: None,
            description: "add login".into(),
            body: None,
            footers: vec![],
            is_breaking: false,
        };
        assert_eq!(format(&commit), "feat: add login");
    }

    #[test]
    fn with_scope() {
        let commit = ConventionalCommit {
            r#type: "fix".into(),
            scope: Some("auth".into()),
            description: "handle tokens".into(),
            body: None,
            footers: vec![],
            is_breaking: false,
        };
        assert_eq!(format(&commit), "fix(auth): handle tokens");
    }

    #[test]
    fn breaking_with_bang() {
        let commit = ConventionalCommit {
            r#type: "feat".into(),
            scope: None,
            description: "remove legacy API".into(),
            body: None,
            footers: vec![],
            is_breaking: true,
        };
        assert_eq!(format(&commit), "feat!: remove legacy API");
    }

    #[test]
    fn with_body() {
        let commit = ConventionalCommit {
            r#type: "feat".into(),
            scope: None,
            description: "add PKCE".into(),
            body: Some("Full PKCE flow.".into()),
            footers: vec![],
            is_breaking: false,
        };
        assert_eq!(format(&commit), "feat: add PKCE\n\nFull PKCE flow.");
    }

    #[test]
    fn with_footers() {
        let commit = ConventionalCommit {
            r#type: "fix".into(),
            scope: None,
            description: "fix bug".into(),
            body: None,
            footers: vec![
                Footer {
                    token: "Refs".into(),
                    value: "#42".into(),
                },
                Footer {
                    token: "Reviewed-by".into(),
                    value: "Alice".into(),
                },
            ],
            is_breaking: false,
        };
        assert_eq!(
            format(&commit),
            "fix: fix bug\n\nRefs: #42\nReviewed-by: Alice"
        );
    }

    #[test]
    fn roundtrip() {
        let msg = "feat(auth): add OAuth2 PKCE flow";
        let commit = crate::parse::parse(msg).unwrap();
        assert_eq!(format(&commit), msg);
    }
}
