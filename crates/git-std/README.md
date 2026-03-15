# git-std

From commit to release. One tool for
[conventional commits][cc], [versioning][semver],
[changelog][keep-changelog], and [git hooks][githooks]
management.

Replaces commitizen, commitlint, standard-version, husky, and
lefthook with a single binary. Fast, zero runtime dependencies.
Works out of the box with sensible defaults.

Invoked as `git std` via git's subcommand discovery.

## Install

```bash
cargo install git-std
```

Or via install script:

```bash
curl -fsSL https://raw.githubusercontent.com/driftsys/git-std/main/install.sh | bash
```

## Quick start

```bash
git add .
git std commit                       # interactive commit
git std check --range main..HEAD     # validate commits
git std changelog --stdout           # preview changelog
git std bump                         # bump + changelog + tag
git push --follow-tags
```

## Subcommands

| Command             | Purpose                          |
| ------------------- | -------------------------------- |
| `git std commit`    | Interactive conventional commit  |
| `git std check`     | Commit message validation        |
| `git std bump`      | Version bump + changelog + tag   |
| `git std changelog` | Generate or update the changelog |
| `git std hooks`     | Git hooks management             |

## Configuration

Optional `.git-std.toml` in the project root:

```toml
types = ["feat", "fix", "docs", "chore"]
scopes = ["auth", "api"]

[versioning]
tag_prefix = "v"

[changelog]
hidden = ["chore", "ci"]
```

All fields are optional — sensible defaults apply when the
file is absent.

See the full [documentation][docs] for details.

## License

MIT

[cc]: https://www.conventionalcommits.org/en/v1.0.0/
[semver]: https://semver.org/spec/v2.0.0.html
[keep-changelog]: https://keepachangelog.com/en/1.1.0/
[githooks]: https://git-scm.com/docs/githooks
[docs]: https://github.com/driftsys/git-std/blob/main/docs/USAGE.md
