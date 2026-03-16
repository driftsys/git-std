use crate::parse::{self, ConventionalCommit};

/// Configuration for linting conventional commit messages.
#[derive(Debug, Clone)]
pub struct LintConfig {
    /// Allowed commit types. `None` means any lowercase type is accepted.
    pub types: Option<Vec<String>>,
    /// Allowed scopes. `None` means any scope is accepted.
    pub scopes: Option<Vec<String>>,
    /// Maximum header line length. Default: 100.
    pub max_header_length: usize,
    /// Whether a scope is required. Default: false.
    pub require_scope: bool,
}

impl Default for LintConfig {
    fn default() -> Self {
        Self {
            types: None,
            scopes: None,
            max_header_length: 100,
            require_scope: false,
        }
    }
}

/// A lint error found in a commit message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct LintError {
    /// Human-readable description of the error.
    pub message: String,
}

/// Lint a commit message against the given configuration.
///
/// First parses the message, then applies additional rules from the config.
/// Returns an empty vec if the message is valid.
pub fn lint(message: &str, config: &LintConfig) -> Vec<LintError> {
    let mut errors = Vec::new();

    let commit = match parse::parse(message) {
        Ok(c) => c,
        Err(e) => {
            errors.push(LintError {
                message: e.to_string(),
            });
            return errors;
        }
    };

    check_header_length(message, config.max_header_length, &mut errors);
    check_type(&commit, &config.types, &mut errors);
    check_scope(&commit, &config.scopes, config.require_scope, &mut errors);

    errors
}

fn check_header_length(message: &str, max: usize, errors: &mut Vec<LintError>) {
    if let Some(header) = message.lines().next()
        && header.len() > max
    {
        errors.push(LintError {
            message: format!(
                "header is {} characters, exceeds maximum of {max}",
                header.len()
            ),
        });
    }
}

fn check_type(
    commit: &ConventionalCommit,
    types: &Option<Vec<String>>,
    errors: &mut Vec<LintError>,
) {
    if let Some(allowed) = types
        && !allowed.iter().any(|t| t == &commit.r#type)
    {
        errors.push(LintError {
            message: format!(
                "type '{}' is not in the allowed list: {}",
                commit.r#type,
                allowed.join(", ")
            ),
        });
    }
}

fn check_scope(
    commit: &ConventionalCommit,
    scopes: &Option<Vec<String>>,
    require_scope: bool,
    errors: &mut Vec<LintError>,
) {
    if require_scope && commit.scope.is_none() {
        errors.push(LintError {
            message: "scope is required".to_string(),
        });
    }

    if let (Some(allowed), Some(scope)) = (scopes, &commit.scope)
        && !allowed.iter().any(|s| s == scope)
    {
        errors.push(LintError {
            message: format!(
                "scope '{scope}' is not in the allowed list: {}",
                allowed.join(", ")
            ),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_with_default_config() {
        let errors = lint("feat: add login", &LintConfig::default());
        assert!(errors.is_empty());
    }

    #[test]
    fn invalid_message_returns_parse_error() {
        let errors = lint("bad message", &LintConfig::default());
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn header_too_long() {
        let long_desc = "x".repeat(100);
        let msg = format!("feat: {long_desc}");
        let errors = lint(&msg, &LintConfig::default());
        assert!(errors.iter().any(|e| e.message.contains("exceeds maximum")));
    }

    #[test]
    fn type_not_in_allowed_list() {
        let config = LintConfig {
            types: Some(vec!["feat".into(), "fix".into()]),
            ..Default::default()
        };
        let errors = lint("docs: update readme", &config);
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("not in the allowed list"))
        );
    }

    #[test]
    fn type_in_allowed_list() {
        let config = LintConfig {
            types: Some(vec!["feat".into(), "fix".into()]),
            ..Default::default()
        };
        let errors = lint("feat: add feature", &config);
        assert!(errors.is_empty());
    }

    #[test]
    fn scope_required_but_missing() {
        let config = LintConfig {
            require_scope: true,
            ..Default::default()
        };
        let errors = lint("feat: add feature", &config);
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("scope is required"))
        );
    }

    #[test]
    fn scope_not_in_allowed_list() {
        let config = LintConfig {
            scopes: Some(vec!["auth".into(), "api".into()]),
            ..Default::default()
        };
        let errors = lint("feat(unknown): add feature", &config);
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("not in the allowed list"))
        );
    }

    #[test]
    fn scope_in_allowed_list() {
        let config = LintConfig {
            scopes: Some(vec!["auth".into(), "api".into()]),
            ..Default::default()
        };
        let errors = lint("feat(auth): add feature", &config);
        assert!(errors.is_empty());
    }
}
