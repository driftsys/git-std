//! Conventional commit parsing, validation, and formatting.
//!
//! Implements the [Conventional Commits](https://www.conventionalcommits.org/)
//! specification as a pure library — no I/O, no git operations, no terminal
//! output.
//!
//! # Main entry points
//!
//! - [`parse`] — parse a commit message into a [`ConventionalCommit`]
//! - [`lint`] — validate a message against a [`LintConfig`]
//! - [`format`] — render a [`ConventionalCommit`] back to a well-formed string
//!
//! # Example
//!
//! ```
//! use standard_commit::{parse, format, lint, LintConfig};
//!
//! let commit = parse("feat(auth): add OAuth2 PKCE flow").unwrap();
//! assert_eq!(commit.r#type, "feat");
//! assert_eq!(commit.scope.as_deref(), Some("auth"));
//!
//! // Round-trip: format back to string
//! assert_eq!(format(&commit), "feat(auth): add OAuth2 PKCE flow");
//!
//! // Lint with default rules
//! let errors = lint("feat: add login", &LintConfig::default());
//! assert!(errors.is_empty());
//! ```

mod format;
mod lint;
mod parse;

pub use format::format;
pub use lint::{LintConfig, LintError, lint};
pub use parse::{ConventionalCommit, Footer, ParseError, parse};
