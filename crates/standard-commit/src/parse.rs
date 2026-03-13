/// A parsed conventional commit message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalCommit {
    /// The commit type (e.g. `feat`, `fix`).
    pub r#type: String,
    /// The optional scope (e.g. `auth`).
    pub scope: Option<String>,
    /// The commit description (subject line after `type(scope): `).
    pub description: String,
    /// The optional body, separated from the header by a blank line.
    pub body: Option<String>,
    /// Trailer footers.
    pub footers: Vec<Footer>,
    /// Whether this is a breaking change (`!` suffix or `BREAKING CHANGE` footer).
    pub is_breaking: bool,
}

/// A commit message footer (trailer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Footer {
    /// The footer token (e.g. `BREAKING CHANGE`, `Refs`).
    pub token: String,
    /// The footer value.
    pub value: String,
}

/// Errors that can occur when parsing a conventional commit message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The message is empty.
    EmptyMessage,
    /// The message does not conform to the conventional commit format.
    InvalidFormat(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::EmptyMessage => write!(f, "commit message is empty"),
            ParseError::InvalidFormat(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse a commit message string into a [`ConventionalCommit`].
///
/// Validates that the message conforms to the
/// [Conventional Commits](https://www.conventionalcommits.org/) specification:
/// `<type>[(<scope>)][!]: <description>`, with optional body and footers.
///
/// The type must be lowercase ASCII (`[a-z]+`).
pub fn parse(message: &str) -> Result<ConventionalCommit, ParseError> {
    let message = message.trim();
    if message.is_empty() {
        return Err(ParseError::EmptyMessage);
    }

    let commit = git_conventional::Commit::parse(message)
        .map_err(|e| ParseError::InvalidFormat(e.to_string()))?;

    let ty = commit.type_().as_str();
    if !ty.bytes().all(|b| b.is_ascii_lowercase()) {
        return Err(ParseError::InvalidFormat(format!(
            "type must be lowercase: '{ty}'"
        )));
    }

    let footers = commit
        .footers()
        .iter()
        .map(|f| Footer {
            token: f.token().to_string(),
            value: f.value().to_string(),
        })
        .collect();

    Ok(ConventionalCommit {
        r#type: commit.type_().to_string(),
        scope: commit.scope().map(|s| s.to_string()),
        description: commit.description().to_string(),
        body: commit.body().map(|b| b.to_string()),
        footers,
        is_breaking: commit.breaking(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_message() {
        assert_eq!(parse(""), Err(ParseError::EmptyMessage));
        assert_eq!(parse("   "), Err(ParseError::EmptyMessage));
    }

    #[test]
    fn type_only_no_colon() {
        assert!(parse("feat").is_err());
    }

    #[test]
    fn missing_space_after_colon() {
        let result = parse("feat:no space");
        if let Ok(commit) = result {
            assert_eq!(commit.r#type, "feat");
        }
    }

    #[test]
    fn minimal_commit() {
        let commit = parse("feat: add login").unwrap();
        assert_eq!(commit.r#type, "feat");
        assert_eq!(commit.scope, None);
        assert_eq!(commit.description, "add login");
        assert_eq!(commit.body, None);
        assert!(commit.footers.is_empty());
        assert!(!commit.is_breaking);
    }

    #[test]
    fn with_scope() {
        let commit = parse("fix(auth): handle expired tokens").unwrap();
        assert_eq!(commit.r#type, "fix");
        assert_eq!(commit.scope.as_deref(), Some("auth"));
        assert_eq!(commit.description, "handle expired tokens");
        assert!(!commit.is_breaking);
    }

    #[test]
    fn breaking_with_bang() {
        let commit = parse("feat!: remove legacy API").unwrap();
        assert_eq!(commit.r#type, "feat");
        assert!(commit.is_breaking);
    }

    #[test]
    fn breaking_with_scope_and_bang() {
        let commit = parse("refactor(runtime)!: drop Python 2 support").unwrap();
        assert_eq!(commit.r#type, "refactor");
        assert_eq!(commit.scope.as_deref(), Some("runtime"));
        assert!(commit.is_breaking);
    }

    #[test]
    fn uppercase_type_rejected() {
        assert!(parse("FEAT: add login").is_err());
    }
}
