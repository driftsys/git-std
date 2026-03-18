# API Reference

`git-std` is built on five crates. The four library crates
implement domain logic only — no CLI, no git operations, no
terminal output.

| Crate              | Description                                    | docs.rs                                                                                     |
| ------------------ | ---------------------------------------------- | ------------------------------------------------------------------------------------------- |
| git-std            | CLI binary — orchestrates I/O, git, config     | [docs.rs/git-std](https://docs.rs/git-std/latest/git_std/)                                  |
| standard-commit    | Conventional commit parsing, linting           | [docs.rs/standard-commit](https://docs.rs/standard-commit/latest/standard_commit/)          |
| standard-version   | Version bump (semver + calver), file detection | [docs.rs/standard-version](https://docs.rs/standard-version/latest/standard_version/)       |
| standard-changelog | Changelog generation from conventional commits | [docs.rs/standard-changelog](https://docs.rs/standard-changelog/latest/standard_changelog/) |
| standard-githooks  | Hook file format parsing, shim generation      | [docs.rs/standard-githooks](https://docs.rs/standard-githooks/latest/standard_githooks/)    |
