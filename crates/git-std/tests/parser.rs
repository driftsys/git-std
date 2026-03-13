use git_std::commit::{self, ParseError};

// ── Valid messages ────────────────────────────────────────────────

#[test]
fn minimal_type_and_description() {
    let c = commit::parse("feat: add login").unwrap();
    assert_eq!(c.r#type, "feat");
    assert_eq!(c.scope, None);
    assert_eq!(c.description, "add login");
    assert_eq!(c.body, None);
    assert!(c.footers.is_empty());
    assert!(!c.is_breaking);
}

#[test]
fn with_scope() {
    let c = commit::parse("fix(auth): handle expired tokens").unwrap();
    assert_eq!(c.r#type, "fix");
    assert_eq!(c.scope.as_deref(), Some("auth"));
    assert_eq!(c.description, "handle expired tokens");
}

#[test]
fn breaking_bang_no_scope() {
    let c = commit::parse("feat!: remove legacy API").unwrap();
    assert!(c.is_breaking);
    assert_eq!(c.r#type, "feat");
}

#[test]
fn breaking_bang_with_scope() {
    let c = commit::parse("refactor(runtime)!: drop Python 2 support").unwrap();
    assert!(c.is_breaking);
    assert_eq!(c.scope.as_deref(), Some("runtime"));
}

#[test]
fn with_body() {
    let msg = "feat: add OAuth2 PKCE flow\n\nImplements the full PKCE authorization code flow\nwith S256 challenge method.";
    let c = commit::parse(msg).unwrap();
    assert_eq!(c.r#type, "feat");
    assert_eq!(c.description, "add OAuth2 PKCE flow");
    assert!(c.body.is_some());
    assert!(c.body.as_deref().unwrap().contains("PKCE authorization"));
}

#[test]
fn with_breaking_change_footer() {
    let msg = "feat: change auth flow\n\nBREAKING CHANGE: token format changed from JWT to opaque";
    let c = commit::parse(msg).unwrap();
    assert!(c.is_breaking);
    assert!(c.footers.iter().any(|f| f.token == "BREAKING CHANGE"));
}

#[test]
fn with_multiple_footers() {
    let msg = "fix(api): correct rate limiting\n\nRefs: #123\nReviewed-by: Alice";
    let c = commit::parse(msg).unwrap();
    assert_eq!(c.footers.len(), 2);
}

#[test]
fn breaking_bang_and_footer_combined() {
    let msg = "feat(api)!: redesign endpoints\n\nBREAKING CHANGE: all v1 endpoints removed";
    let c = commit::parse(msg).unwrap();
    assert!(c.is_breaking);
}

#[test]
fn various_types() {
    for ty in &[
        "feat", "fix", "docs", "style", "refactor", "perf", "test", "chore", "ci", "build",
    ] {
        let msg = format!("{ty}: do something");
        let c = commit::parse(&msg).unwrap();
        assert_eq!(c.r#type, *ty);
    }
}

#[test]
fn scope_with_slash() {
    let c = commit::parse("fix(api/v2): correct endpoint").unwrap();
    assert_eq!(c.scope.as_deref(), Some("api/v2"));
}

#[test]
fn scope_with_dots() {
    let c = commit::parse("chore(deps.dev): bump test framework").unwrap();
    assert_eq!(c.scope.as_deref(), Some("deps.dev"));
}

#[test]
fn scope_with_hyphen() {
    let c = commit::parse("feat(my-scope): add feature").unwrap();
    assert_eq!(c.scope.as_deref(), Some("my-scope"));
}

#[test]
fn body_with_blank_line_separation() {
    let msg = "docs: update readme\n\nAdded installation instructions\nand usage examples.";
    let c = commit::parse(msg).unwrap();
    assert!(c.body.is_some());
    assert!(c.body.as_deref().unwrap().contains("installation"));
}

#[test]
fn body_and_footer() {
    let msg =
        "feat(parser): add footer support\n\nParse git trailers from commit messages.\n\nRefs: #42";
    let c = commit::parse(msg).unwrap();
    assert!(c.body.is_some());
    assert_eq!(c.footers.len(), 1);
    assert_eq!(c.footers[0].token, "Refs");
    assert_eq!(c.footers[0].value, "#42");
}

// ── Invalid messages ─────────────────────────────────────────────

#[test]
fn empty_message() {
    assert!(matches!(commit::parse(""), Err(ParseError::EmptyMessage)));
}

#[test]
fn whitespace_only() {
    assert!(matches!(
        commit::parse("   \n\n  "),
        Err(ParseError::EmptyMessage)
    ));
}

#[test]
fn no_type_prefix() {
    assert!(commit::parse("added new feature").is_err());
}

#[test]
fn missing_colon_separator() {
    assert!(commit::parse("feat add login").is_err());
}

#[test]
fn empty_description() {
    assert!(commit::parse("feat: ").is_err());
}

#[test]
fn uppercase_type() {
    // Conventional commits spec requires lowercase type
    assert!(commit::parse("FEAT: add login").is_err());
}
