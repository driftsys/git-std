# git-std

From commit to release. One tool for [conventional commits][cc], [versioning][semver], [changelog][keep-changelog], and [git hooks][githooks] management.

`git-std` replaces commitizen, commitlint, standard-version, husky, and lefthook with a single binary. Fast, low memory footprint, zero runtime dependencies. Works out of the box with sensible defaults, which can be overridden with a `.git-std.toml`.

Invoked as `git std` via git's subcommand discovery.

## Subcommands

| Command          | Purpose                                 |
| ---------------- | --------------------------------------- |
| `git std commit` | Interactive conventional commit builder |
| `git std check`  | Commit message validation               |
| `git std bump`   | Version bump + changelog + commit + tag |
| `git std hooks`  | Git hooks management (install/run/list) |

## Workspace crates

`git-std` is built on four independent library crates, each published separately on [crates.io](https://crates.io). The libraries implement domain logic only — no CLI, no git operations, no terminal output. `git-std` is the orchestrator that wires them together with I/O, config, and CLI dispatch.

```text
git-std (binary)
├── standard-commit
├── standard-version
│   └── standard-commit
├── standard-githooks
└── standard-changelog
    └── standard-commit
```

### standard-commit

[![crates.io](https://img.shields.io/crates/v/standard-commit)](https://crates.io/crates/standard-commit)

[Conventional Commits][cc] parsing, linting, and formatting. Pure library — strings in, data out.

- **Parse** a raw commit message into type, scope, description, body, footers, and breaking status
- **Lint** against configurable rules (allowed types/scopes, header length, required scope)
- **Format** a structured commit back to a well-formed message string

### standard-version

[![crates.io](https://img.shields.io/crates/v/standard-version)](https://crates.io/crates/standard-version)

[Semantic][semver] and [calendar][calver] version bump calculation. Pure library — computes the next version from parsed conventional commits and bump rules.

### standard-changelog

[![crates.io](https://img.shields.io/crates/v/standard-changelog)](https://crates.io/crates/standard-changelog)

[Changelog][keep-changelog] generation from conventional commits. Groups commits by type, renders markdown sections, and manages `CHANGELOG.md` files.

### standard-githooks

[![crates.io](https://img.shields.io/crates/v/standard-githooks)](https://crates.io/crates/standard-githooks)

[Git hooks][githooks] file format parsing, shim generation, and execution model. Owns the `.githooks/<hook>.hooks` file format — can read/write hook definitions and generate shim scripts.

## Configuration

`git-std` reads `.git-std.toml` for project-level configuration. Hook definitions live in `.githooks/*.hooks` (plain text, one command per line). The CLI reads the config and passes the relevant settings to each library crate's own config types.

## Install

```bash
cargo install git-std
```

## License

MIT

## Specs and references

- [Conventional Commits v1.0.0][cc]
- [Semantic Versioning v2.0.0][semver]
- [Calendar Versioning][calver]
- [Keep a Changelog v1.1.0][keep-changelog]
- [Git hooks documentation][githooks]

[cc]: https://www.conventionalcommits.org/en/v1.0.0/
[semver]: https://semver.org/spec/v2.0.0.html
[calver]: https://calver.org/
[keep-changelog]: https://keepachangelog.com/en/1.1.0/
[githooks]: https://git-scm.com/docs/githooks
