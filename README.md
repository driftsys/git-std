# git-std

[![CI](https://github.com/driftsys/git-std/actions/workflows/ci.yml/badge.svg)](https://github.com/driftsys/git-std/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/git-std.svg)](https://crates.io/crates/git-std)
[![user guide](https://img.shields.io/badge/docs-user%20guide-blue)](https://driftsys.github.io/git-std/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

From commit to release. One tool for
[conventional commits][cc], [versioning][semver],
[changelog][keep-changelog], and [git hooks][githooks]
management.

`git-std` replaces commitizen, commitlint, standard-version,
husky, and lefthook with a single binary. Fast, low memory
footprint, zero runtime dependencies. Works out of the box
with sensible defaults, which can be overridden with a
`.git-std.toml`.

Invoked as `git std` via git's subcommand discovery.

## Install

**Install script (recommended):**

```bash
curl -fsSL https://raw.githubusercontent.com/driftsys/git-std/main/install.sh | bash
```

**From source:**

```bash
cargo install git-std
```

## Quick start

```bash
git std hooks install                # set up hooks
git add .
git std commit                       # interactive commit
git std check --range main..HEAD     # validate commits
git std changelog --stdout           # preview changelog
git std bump                         # bump + changelog + tag
git push --follow-tags
```

**Shell completions:**

```bash
eval "$(git-std completions bash)"   # or zsh, fish
```

See the [user guide](https://driftsys.github.io/git-std/)
for commands, configuration, and recipes.

## Workspace crates

`git-std` is built on four independent library crates, each
published separately on [crates.io][crates-io]. The libraries
implement domain logic only — no CLI, no git operations, no
terminal output.

| Crate                | Description                                      |
| -------------------- | ------------------------------------------------ |
| [standard-commit]    | Conventional commit parsing, linting, formatting |
| [standard-version]   | Version bump (semver + calver), file detection   |
| [standard-changelog] | Changelog generation from conventional commits   |
| [standard-githooks]  | Hook file format parsing, shim generation        |

## License

MIT

## Documentation

- [User guide](https://driftsys.github.io/git-std/) (mdbook)
- [Specification](docs/SPEC.md)
- [API docs](https://docs.rs/git-std) (docs.rs)

## References

- [Conventional Commits v1.0.0][cc]
- [Semantic Versioning v2.0.0][semver]
- [Calendar Versioning][calver]
- [Keep a Changelog v1.1.0][keep-changelog]
- [Git hooks documentation][githooks]

[cc]: https://www.conventionalcommits.org/en/v1.0.0/
[semver]: https://semver.org/spec/v2.0.0.html
[calver]: https://calver.org/
[keep-changelog]: https://keepachangelog.com/en/1.1.0/
[crates-io]: https://crates.io
[githooks]: https://git-scm.com/docs/githooks
[standard-commit]: crates/standard-commit/
[standard-version]: crates/standard-version/
[standard-changelog]: crates/standard-changelog/
[standard-githooks]: crates/standard-githooks/
