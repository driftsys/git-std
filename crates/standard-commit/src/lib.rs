//! Conventional commit parsing, validation, and formatting.
//!
//! `standard-commit` implements the
//! [Conventional Commits](https://www.conventionalcommits.org/) specification
//! as a pure library — no I/O, no git operations, no terminal output.
//!
//! - **Parsing** — extract type, scope, description, body, footers, and breaking status
//! - **Linting** — validate messages against configurable rules
//! - **Formatting** — render a [`ConventionalCommit`] back to a well-formed message string

mod format;
mod lint;
mod parse;

pub use format::format;
pub use lint::{LintConfig, LintError, lint};
pub use parse::{ConventionalCommit, Footer, ParseError, parse};
