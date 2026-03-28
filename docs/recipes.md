# Recipes

## Git aliases

Shorten common commands with git aliases:

```bash
git config --global alias.sc "std commit"
git config --global alias.sb "std bump"
git config --global alias.sl "std changelog"
git config --global alias.sk "std check"
```

Then use:

```bash
git sc           # interactive commit
git sc -am "fix(auth): handle expired tokens"
git sk --range main..HEAD
git sl --stdout  # preview changelog
git sb           # bump + changelog + tag
```

## Semver workflow

Semver is the default scheme — no configuration needed.

Bump rules are inferred from conventional commits:

- `BREAKING CHANGE` or `!` suffix → **major**
- `feat` → **minor**
- Everything else → **patch**

```bash
git std bump               # auto-detect bump type
git std bump --dry-run     # preview without writing
git std bump --prerelease  # e.g. 2.0.0-rc.1
git std bump --release-as 3.0.0  # force a specific version
```

## Calver workflow

Set the scheme in `.git-std.toml`:

```toml
scheme = "calver"

[versioning]
calver_format = "YYYY.0M.PATCH"
```

Commit types are ignored — the version is derived from the
current date. The patch counter increments within the same
period and resets when the period changes.

```bash
# March 2026, first release of the month
git std bump          # → 2026.03.0

# March 2026, second release
git std bump          # → 2026.03.1

# April 2026, first release
git std bump          # → 2026.04.0
```

Common formats:

| Format             | Example       | Use case         |
| ------------------ | ------------- | ---------------- |
| `YYYY.MM.PATCH`    | `2026.3.0`    | Monthly releases |
| `YYYY.0M.PATCH`    | `2026.03.0`   | Zero-padded      |
| `YY.WW.DP`         | `26.12.30`    | Weekly + day     |
| `YYYY.MM.DD.PATCH` | `2026.3.18.0` | Daily releases   |

## Stable branch

Use `--stable` to create a patch-only maintenance branch
from the current version:

```bash
# On main at v2.3.0
git std bump --stable
```

This creates a `stable/v2.3` branch where only patch bumps
are allowed. Breaking changes are rejected unless `--force`
is used.

```bash
# On stable/v2.3 — cherry-pick a fix, then:
git std bump          # → 2.3.1
git std bump          # → 2.3.2
```

Back on main, use `--minor` to advance without a major bump:

```bash
# On main after stable branch was cut
git std bump --minor  # → 2.4.0 (instead of 3.0.0)
```

## Custom version files

By default, `git std bump` auto-detects and updates
`Cargo.toml` version fields. For other files, add
`[[version_files]]` entries to `.git-std.toml`. Each entry
needs a file path and a regex whose first capture group
matches the version string.

**Plain text version file:**

```toml
[[version_files]]
path = "VERSION"
regex = '(.+)'
```

**Java (pom.xml):**

```toml
[[version_files]]
path = "pom.xml"
regex = '<version>([^<]+)</version>'
```

**Helm chart:**

```toml
[[version_files]]
path = "Chart.yaml"
regex = 'version:\s*(.+)'
```

**Python (pyproject.toml):**

```toml
[[version_files]]
path = "pyproject.toml"
regex = 'version\s*=\s*"([^"]+)"'
```

**Multiple files at once:**

```toml
[[version_files]]
path = "package.json"
regex = '"version":\s*"([^"]+)"'

[[version_files]]
path = "src/version.h"
regex = '#define\s+VERSION\s+"([^"]+)"'
```

These are updated alongside auto-detected files during
`git std bump`. Use `--dry-run` to preview which files
would be updated.

## Monorepo workflow

Enable per-package versioning for multi-package repositories:

```toml
monorepo = true
scheme = "semver"
scopes = "auto"

[versioning]
tag_template = "{name}@{version}"

# Optional — auto-discovered from workspace manifests if omitted
# [[packages]]
# name = "core"
# path = "crates/core"
```

Packages are auto-discovered from `Cargo.toml` workspace
members, `package.json` workspaces, or `deno.json` workspace.

### Bump all packages

```bash
git std bump                   # bumps all packages with changes
git std bump --dry-run         # preview what would change
```

### Bump specific packages

```bash
git std bump -p core           # bump only core
git std bump -p core -p cli    # bump core and cli
```

The `-p` flag skips dependency cascade — only the named
packages are bumped.

### Per-package scheme override

Mix versioning schemes in the same monorepo:

```toml
monorepo = true
scheme = "semver"              # default for all packages

[versioning]
tag_template = "{name}@{version}"
calver_format = "YYYY.0M.PATCH"

[[packages]]
name = "core"
path = "crates/core"

[[packages]]
name = "api"
path = "crates/api"
scheme = "calver"              # this package uses calver
```

### Per-package changelog config

Override changelog settings per package:

```toml
[[packages]]
name = "core"
path = "crates/core"

[packages.changelog]
hidden = ["chore"]             # show more commit types for core
```

Each package gets its own `CHANGELOG.md` in its root
directory (e.g. `crates/core/CHANGELOG.md`). The root
`CHANGELOG.md` includes all commits from all packages.

## Git hooks

### Setting up hooks

```bash
git std hooks install
```

This writes `.githooks/*.hooks` template files and shim
scripts, sets `core.hooksPath`, and prompts which hooks to
enable.

### Hook types

| Hook                 | When it runs                     | Typical use                       |
| -------------------- | -------------------------------- | --------------------------------- |
| `pre-commit`         | Before a commit is created       | Lint, format, run fast tests      |
| `commit-msg`         | After the message is written     | Validate conventional commit      |
| `prepare-commit-msg` | Before the editor opens          | Pre-fill commit template          |
| `post-commit`        | After the commit is created      | Notifications, stats              |
| `pre-push`           | Before push sends data to remote | Full test suite, build validation |
| `post-merge`         | After a merge completes          | Reinstall deps, rebuild           |

### Command prefixes

Each `.hooks` file contains one command per line with a
prefix that controls behaviour:

| Prefix | Name     | Behaviour                           |
| ------ | -------- | ----------------------------------- |
| `!`    | check    | Run command, block on failure       |
| `~`    | fix      | Isolate staged files, run, re-stage |
| `?`    | advisory | Run command, never block            |

The `~` prefix safely isolates staged content, runs the
formatter, and re-stages the result.
`$@` is populated with the list of staged files.

### pre-commit: lint only

Check formatting and lint without modifying files. The
commit is blocked if any check fails.

`.githooks/pre-commit.hooks`:

```text
! cargo fmt --check -- $@
! cargo clippy --workspace -- -D warnings
```

### pre-commit: auto-format

Automatically format staged files and re-stage the result.
The `~` prefix handles the stash dance so unstaged changes
are preserved.

`.githooks/pre-commit.hooks`:

```text
~ cargo fmt -- $@
! cargo clippy --workspace -- -D warnings
? cargo test --workspace
```

This formats first, then lints (blocking), then runs tests
(advisory — never blocks the commit).

### commit-msg: validate message

Reject commits that don't follow conventional commits.

`.githooks/commit-msg.hooks`:

```text
! git std check --file {msg}
```

### pre-push: PR readiness check

Run the full validation suite before pushing. Catches
issues before CI does.

`.githooks/pre-push.hooks`:

```text
! cargo test --workspace
! cargo clippy --workspace -- -D warnings
! git std check --range origin/main..HEAD
```

### Managing hooks

```bash
git std hooks enable pre-push
git std hooks disable post-commit
git std hooks list              # see status of all hooks
```
