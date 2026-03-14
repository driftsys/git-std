# git-std

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

The script detects your OS and architecture, downloads the
matching binary from the latest GitHub release, and installs
it to `~/.local/bin/git-std`. Set `GIT_STD_INSTALL_DIR` to
override the install location.

**From source:**

```bash
cargo install git-std
```

**Verify:**

```bash
git std --version
```

## Quick start

```bash
# Set up hooks (if your repo has .githooks/*.hooks files)
git std hooks install

# Stage changes and make a conventional commit interactively
git add .
git std commit

# Check commit messages on your branch
git std check --range main..HEAD

# Generate a changelog
git std changelog --stdout

# Bump version, update changelog, commit, and tag
git std bump
git push --follow-tags
```

## Subcommands

| Command               | Purpose                                 |
| --------------------- | --------------------------------------- |
| `git std commit`      | Interactive conventional commit builder |
| `git std check`       | Commit message validation               |
| `git std bump`        | Version bump + changelog + commit + tag |
| `git std changelog`   | Generate or update the changelog        |
| `git std hooks`       | Git hooks management (install/run/list) |
| `git std self-update` | Update git-std to the latest version    |

### `git std commit`

Build a conventional commit message interactively or non-interactively.

```bash
# Interactive — prompts for type, scope, subject, body, breaking change
git std commit

# Non-interactive
git std commit -a --type fix --scope auth -m "fix(auth): handle expired tokens"

# Preview without committing
git std commit --dry-run
```

| Flag              | Description                            |
| ----------------- | -------------------------------------- |
| `--type <type>`   | Pre-fill commit type, skip type prompt |
| `--scope <scope>` | Pre-fill scope, skip scope prompt      |
| `-m <msg>`        | Non-interactive mode with full message |
| `--breaking`      | Add a `BREAKING CHANGE` footer         |
| `--dry-run`       | Print the message without committing   |
| `--amend`         | Amend the previous commit              |
| `--sign` / `-S`   | GPG-sign the commit                    |
| `-a` / `--all`    | Stage all tracked modified files first |

### `git std check`

Validate commit messages against the conventional commit spec.

```bash
# Single message
git std check "feat: add feature"

# From a file (e.g., commit-msg hook)
git std check --file .git/COMMIT_EDITMSG

# All commits on a branch
git std check --range main..HEAD

# Strict mode — reject unknown types/scopes
git std check --strict "feat(auth): add login"
```

| Flag              | Description                                |
| ----------------- | ------------------------------------------ |
| `--file <path>`   | Read message from a file                   |
| `--range <range>` | Validate all commits in a revision range   |
| `--strict`        | Enforce known types and scopes from config |
| `--format <fmt>`  | Output format: `text` (default) or `json`  |

### `git std bump`

Calculate the next version from conventional commits, update
version files, generate changelog, commit, and tag.

```bash
# Preview what would happen
git std bump --dry-run

# Execute the bump
git std bump

# Pre-release
git std bump --prerelease

# Force a specific version
git std bump --release-as 3.0.0
```

### `git std changelog`

Generate or update the changelog from commit history.

```bash
# Preview unreleased changes
git std changelog --stdout

# Regenerate the full changelog
git std changelog --full

# Write to a specific file
git std changelog --output CHANGES.md
```

### `git std hooks`

Manage git hooks defined in `.githooks/*.hooks` files.

```bash
# Install hook shims
git std hooks install

# List configured hooks
git std hooks list

# Run a hook manually (useful for debugging)
git std hooks run pre-commit
```

## Configuration

`git-std` reads `.git-std.toml` from the project root. All
fields are optional — sensible defaults apply when the file
is absent.

```toml
# Versioning scheme: "semver" (default), "calver", or "patch"
scheme = "semver"

# Allowed commit types (defaults to the conventional commit standard set)
types = ["feat", "fix", "docs", "style", "refactor", "perf", "test",
         "chore", "ci", "build"]

# Scope validation: omit for no validation, "auto" to discover from
# workspace layout, or an explicit list
scopes = ["auth", "api", "cli", "deps"]

# Enforce types/scopes without the --strict flag
strict = true

[versioning]
tag_prefix = "v"                  # Git tag prefix (default: "v")
prerelease_tag = "rc"             # Default pre-release identifier
calver_format = "YYYY.MM.PATCH"   # Only used when scheme = "calver"

[changelog]
hidden = ["chore", "ci", "build", "style", "test"]

[changelog.sections]
feat = "Features"
fix = "Bug Fixes"
perf = "Performance"
refactor = "Refactoring"
docs = "Documentation"
```

Hook definitions live in `.githooks/*.hooks` files (plain
text, one command per line). See the
[specification](docs/SPEC.md) for the full hooks file
format.

## Git hooks

Create `.githooks/<hook-name>.hooks` files and run
`git std hooks install` to activate them.

**Example `.githooks/commit-msg.hooks`:**

```sh
! git std check --file {msg}
```

**Example `.githooks/pre-commit.hooks`:**

```sh
# Formatting
dprint check

# Rust
cargo clippy --workspace -- -D warnings *.rs
cargo test --workspace --lib *.rs
```

**Prefixes:** `!` = fail fast (abort on failure),
`?` = advisory (warn only), none = hook default mode.

**Globs** at the end of a line restrict the command to
matching staged files. No match means the command is
skipped.

## CI integration

### GitHub Actions

```yaml
name: Validate commits
on: pull_request

jobs:
  check-commits:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Install git-std
        run: curl -fsSL https://raw.githubusercontent.com/driftsys/git-std/main/install.sh | bash

      - name: Check commit messages
        run: git std check --range ${{ github.event.pull_request.base.sha }}..${{ github.sha }}
```

### GitLab CI

```yaml
lint:commits:
  stage: build
  script:
    - curl -fsSL https://raw.githubusercontent.com/driftsys/git-std/main/install.sh | bash
    - git std check --range $CI_MERGE_REQUEST_DIFF_BASE_SHA..HEAD
```

## Workspace crates

`git-std` is built on four independent library crates, each
published separately on [crates.io][crates-io]. The libraries
implement domain logic only — no CLI, no git operations, no
terminal output. `git-std` is the orchestrator that wires
them together with I/O, config, and CLI dispatch.

```text
git-std (binary)
├── standard-commit      — conventional commit parsing, linting, formatting
├── standard-version     — semantic and calendar version bump calculation
├── standard-changelog   — changelog generation from conventional commits
└── standard-githooks    — hook file format parsing, shim generation
```

## License

MIT

## Specs and references

- [Full specification](docs/SPEC.md)
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
