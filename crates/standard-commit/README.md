# standard-commit

[![crates.io](https://img.shields.io/crates/v/standard-commit.svg)](https://crates.io/crates/standard-commit)
[![docs.rs](https://docs.rs/standard-commit/badge.svg)](https://docs.rs/standard-commit)

Conventional commit parsing, validation, and formatting.

Implements the [Conventional Commits][cc] specification as a
pure library — no I/O, no git operations, no terminal output.

## Entry points

- `parse` — parse a commit message into a `ConventionalCommit`
- `lint` — validate a message against a `LintConfig`
- `format` — render a `ConventionalCommit` back to a string

## Example

```rust
use standard_commit::{parse, format, lint, LintConfig};

let commit = parse("feat(auth): add OAuth2 PKCE flow").unwrap();
assert_eq!(commit.r#type, "feat");
assert_eq!(commit.scope.as_deref(), Some("auth"));

// Round-trip: format back to string
assert_eq!(format(&commit), "feat(auth): add OAuth2 PKCE flow");

// Lint with default rules
let errors = lint("feat: add login", &LintConfig::default());
assert!(errors.is_empty());
```

## Part of git-std

This crate is one of four libraries powering [git-std][git-std],
a single binary for conventional commits, versioning, changelog,
and git hooks.

## License

MIT

[cc]: https://www.conventionalcommits.org/en/v1.0.0/
[git-std]: https://github.com/driftsys/git-std
