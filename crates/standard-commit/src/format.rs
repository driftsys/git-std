use crate::parse::ConventionalCommit;

/// Maximum header (subject) line length.
const MAX_HEADER_LENGTH: usize = 100;
/// Maximum body/footer line width (git convention).
const BODY_LINE_WIDTH: usize = 72;

/// Format a [`ConventionalCommit`] back into a well-formed conventional commit message string.
///
/// Applies line width rules:
/// - Header is truncated to [`MAX_HEADER_LENGTH`] characters
/// - Body lines are word-wrapped at [`BODY_LINE_WIDTH`] characters
/// - Footer values are word-wrapped at [`BODY_LINE_WIDTH`] characters
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

    // Truncate header to max length
    if msg.len() > MAX_HEADER_LENGTH {
        msg.truncate(MAX_HEADER_LENGTH);
    }

    // Body — word-wrapped
    if let Some(body) = &commit.body {
        msg.push_str("\n\n");
        msg.push_str(&wrap_text(body, BODY_LINE_WIDTH));
    }

    // Footers — values word-wrapped
    if !commit.footers.is_empty() {
        msg.push_str("\n\n");
        for (i, footer) in commit.footers.iter().enumerate() {
            if i > 0 {
                msg.push('\n');
            }
            let prefix = format!("{}: ", footer.token);
            let indent_width = BODY_LINE_WIDTH.saturating_sub(prefix.len());
            if indent_width > 0 && prefix.len() + footer.value.len() > BODY_LINE_WIDTH {
                msg.push_str(&prefix);
                msg.push_str(&wrap_text(&footer.value, indent_width));
            } else {
                msg.push_str(&prefix);
                msg.push_str(&footer.value);
            }
        }
    }

    msg
}

/// Word-wrap text to the given width, preserving paragraph breaks (`\n\n`).
fn wrap_text(text: &str, width: usize) -> String {
    text.split("\n\n")
        .map(|paragraph| wrap_paragraph(paragraph, width))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn wrap_paragraph(paragraph: &str, width: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut current_line = String::new();

    for word in paragraph.split_whitespace() {
        if current_line.is_empty() {
            current_line.push_str(word);
        } else if current_line.len() + 1 + word.len() > width {
            lines.push(current_line);
            current_line = word.to_string();
        } else {
            current_line.push(' ');
            current_line.push_str(word);
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    lines.join("\n")
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

    #[test]
    fn body_wraps_at_72() {
        let long_body = "This is a long body line that should be wrapped because it exceeds the maximum line width of seventy-two characters per line";
        let commit = ConventionalCommit {
            r#type: "feat".into(),
            scope: None,
            description: "test".into(),
            body: Some(long_body.into()),
            footers: vec![],
            is_breaking: false,
        };
        let msg = format(&commit);
        // Skip header line, check body lines
        for line in msg.lines().skip(2) {
            assert!(
                line.len() <= 72,
                "body line too long ({}): {}",
                line.len(),
                line
            );
        }
    }

    #[test]
    fn body_preserves_paragraphs() {
        let commit = ConventionalCommit {
            r#type: "feat".into(),
            scope: None,
            description: "test".into(),
            body: Some("First paragraph.\n\nSecond paragraph.".into()),
            footers: vec![],
            is_breaking: false,
        };
        let msg = format(&commit);
        assert!(msg.contains("First paragraph.\n\nSecond paragraph."));
    }

    #[test]
    fn header_truncated_at_100() {
        let long_desc = "a".repeat(200);
        let commit = ConventionalCommit {
            r#type: "feat".into(),
            scope: None,
            description: long_desc,
            body: None,
            footers: vec![],
            is_breaking: false,
        };
        let msg = format(&commit);
        let header = msg.lines().next().unwrap();
        assert_eq!(header.len(), 100);
    }
}
