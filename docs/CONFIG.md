# git-std — Configuration

`git-std` reads `.git-std.toml` in the project root. All
fields are optional — sensible defaults are used when the
file is absent or a field is omitted.

## Editor schema

A [JSON Schema](schema/v1/git-std.toml.json) is available
for validation and autocomplete in any JSON Schema-aware
TOML editor.

Add the `$schema` key to your `.git-std.toml`:

```toml
"$schema" = "https://driftsys.github.io/git-std/schema/v1/git-std.toml.json"
```

Or use a [taplo](https://taplo.tamasfe.dev) inline directive
(does not modify the file):

```toml
#:schema https://driftsys.github.io/git-std/schema/v1/git-std.toml.json
```

## Full schema

```toml
# ── Project ───────────────────────────────────────────────────────
scheme = "semver"                              # semver | calver | patch
types = ["feat", "fix", "docs", "style",
         "refactor", "perf", "test",
         "chore", "ci", "build", "revert"]
scopes = ["auth", "api", "ci", "deps"]         # "auto" | string[] | omit
strict = true                         # enforce types/scopes
monorepo = false                      # per-package versioning

# ── Versioning ────────────────────────────────────────────────────
[versioning]
tag_prefix = "v"                               # git tag prefix
prerelease_tag = "rc"                          # default pre-release id
calver_format = "YYYY.MM.PATCH"                # only when scheme = "calver"
tag_template = "{name}@{version}"              # per-package tag format

# ── Changelog ─────────────────────────────────────────────────────
[changelog]
title = "Release Notes"                        # optional, custom heading
hidden = ["chore", "ci", "build", "style", "test"]
bug_url = "https://github.com/org/repo/issues" # optional, issue link base

[changelog.sections]
feat = "Features"
fix = "Bug Fixes"
perf = "Performance"
refactor = "Refactoring"
docs = "Documentation"

# ── Version files ────────────────────────────────────────────
[[version_files]]
path = "pom.xml"
regex = '<version>([^<]+)</version>'

# ── Packages (monorepo) ─────────────────────────────────────
[[packages]]
name = "core"
path = "crates/core"
# scheme = "patch"                             # optional override
```

## Fields

### Top-level

| Field      | Type                 | Default           | Description                                             |
| ---------- | -------------------- | ----------------- | ------------------------------------------------------- |
| `scheme`   | string               | `"semver"`        | Versioning scheme (see below)                           |
| `types`    | string[]             | 11 standard types | Allowed conventional commit types                       |
| `scopes`   | `"auto"` or string[] | None              | Scope discovery or explicit allowlist                   |
| `strict`   | bool                 | `false`           | Enforce types/scopes validation without `--strict` flag |
| `monorepo` | bool                 | `false`           | Enable per-package versioning                           |

Default types: `feat`, `fix`, `docs`, `style`, `refactor`,
`perf`, `test`, `chore`, `ci`, `build`, `revert`.

**Versioning schemes:**

- **`semver`** — `BREAKING CHANGE` or `!` → major, `feat` →
  minor, everything else → patch. Resets lower components
  (e.g. `1.2.3` → `1.3.0`). Supports `--prerelease`.
- **`calver`** — date-based, ignores commit types. Uses
  `calver_format` (default `YYYY.MM.PATCH`). Patch increments
  within the same period, resets on period change. No
  `--prerelease`.
- **`patch`** — always increments patch only, never touches
  major/minor. Breaking changes rejected unless `--force` is
  used. Intended for maintenance/LTS branches.

**Scopes behavior:**

- **Not set** (default) — no scope validation, any scope accepted
- **`scopes = "auto"`** — discover scopes from workspace
  layout (`crates/*`, `packages/*`, `modules/*`)
- **`scopes = ["auth", "api"]`** — explicit allowlist

When scopes is set (either `"auto"` or an array) and
`--strict` is used, a scope is required and must be in the
resolved list. For `git std commit`, the resolved scopes
populate the interactive scope prompt.

### `[versioning]`

| Field            | Type   | Default              | Description                                             |
| ---------------- | ------ | -------------------- | ------------------------------------------------------- |
| `tag_prefix`     | string | `"v"`                | Git tag prefix (e.g., `v1.0.0`)                         |
| `prerelease_tag` | string | `"rc"`               | Default pre-release identifier                          |
| `calver_format`  | string | `"YYYY.MM.PATCH"`    | Calendar version format (only when `scheme = "calver"`) |
| `tag_template`   | string | `"{name}@{version}"` | Per-package tag format (only when `monorepo = true`)    |

**Calendar version format tokens:**

| Token   | Description                                               | Example          |
| ------- | --------------------------------------------------------- | ---------------- |
| `YYYY`  | Full year                                                 | `2026`           |
| `YY`    | Short year                                                | `26`             |
| `0M`    | Zero-padded month                                         | `03`             |
| `MM`    | Month (no padding)                                        | `3`              |
| `WW`    | ISO week number                                           | `11`             |
| `DD`    | Day of month                                              | `13`             |
| `PATCH` | Auto-incrementing patch counter, resets each period       | `0`, `1`, `2`    |
| `DP`    | Day of week (1=Mon–7=Sun) concatenated with patch counter | `30`, `31`, `32` |

Common formats: `YYYY.MM.PATCH` (monthly releases),
`YYYY.0M.PATCH` (zero-padded month), `YY.WW.DP` (weekly
with day-of-week), `YYYY.MM.DD.PATCH` (daily releases).

Bump rules are inferred from the scheme — not
configurable. For semver: `BREAKING CHANGE` or `!` triggers
major, `feat` triggers minor, everything else triggers
patch.

### `[changelog]`

| Field     | Type     | Default                                     | Description                      |
| --------- | -------- | ------------------------------------------- | -------------------------------- |
| `title`   | string   | _(none)_                                    | Custom changelog title           |
| `hidden`  | string[] | `["chore", "ci", "build", "style", "test"]` | Types excluded from changelog    |
| `bug_url` | string   | _(none)_                                    | URL template for bug/issue links |

### `[changelog.sections]`

Maps commit types to changelog section headings. Types not
listed here use the type name as the heading.

| Key        | Default           |
| ---------- | ----------------- |
| `feat`     | `"Features"`      |
| `fix`      | `"Bug Fixes"`     |
| `perf`     | `"Performance"`   |
| `refactor` | `"Refactoring"`   |
| `docs`     | `"Documentation"` |

### `[[version_files]]`

Optional array of custom version files to update during bump.
Each entry specifies a file path and a regex whose first
capture group contains the version string.

```toml
[[version_files]]
path = "pom.xml"
regex = '<version>([^<]+)</version>'

[[version_files]]
path = "Chart.yaml"
regex = 'version:\s*(.+)'
```

| Field   | Type   | Description                                 |
| ------- | ------ | ------------------------------------------- |
| `path`  | string | File path relative to repo root             |
| `regex` | string | Regex with capture group containing version |

Entries with missing `path` or `regex` are silently skipped.
These are in addition to auto-detected version files
(e.g. `Cargo.toml`).

### `[[packages]]`

Explicit package definitions for monorepo workspaces. When
`monorepo = true` and no packages are listed, git-std
auto-discovers packages from workspace manifests (Cargo,
npm, Deno) or subdirectories with version files.

```toml
[[packages]]
name = "core"
path = "crates/core"
scheme = "patch"                   # optional: override global scheme

[[packages.version_files]]         # optional: override version files
path = "version.txt"
regex = '(\d+\.\d+\.\d+)'

[packages.changelog]               # optional: override changelog config
title = "Core Changelog"
hidden = ["chore"]
```

| Field           | Type   | Description                                |
| --------------- | ------ | ------------------------------------------ |
| `name`          | string | Package name (used in tags and changelogs) |
| `path`          | string | Package root relative to repo root         |
| `scheme`        | string | Optional versioning scheme override        |
| `version_files` | array  | Optional version files override            |
| `changelog`     | table  | Optional changelog config override         |

Entries with missing `name` or `path` are silently skipped.

## Inferred settings

These are not configurable — git-std resolves them automatically:

| Concern              | Resolution                                                |
| -------------------- | --------------------------------------------------------- |
| Bump rules           | Inferred from `scheme`                                    |
| Version files        | Auto-detected (Cargo.toml)                                |
| URLs                 | Inferred from `git remote get-url origin`                 |
| Changelog output     | `CHANGELOG.md` at root; `{path}/CHANGELOG.md` per package |
| Release commit       | `chore(release): <version>` (includes packages)           |
| Package dependencies | Resolved from workspace manifests (runtime only)          |

## Minimal examples

**No config needed** — git-std works with zero
configuration using conventional defaults.

**Types and scopes only:**

```toml
types = ["feat", "fix", "chore"]
scopes = ["auth", "api"]
```

**Calver project:**

```toml
scheme = "calver"

[versioning]
calver_format = "YYYY.0M.PATCH"
```

**Custom changelog sections:**

```toml
[changelog]
hidden = ["chore", "ci"]

[changelog.sections]
feat = "New Features"
fix = "Bug Fixes"
perf = "Performance Improvements"
```

**Monorepo with per-package versioning:**

```toml
monorepo = true
scheme = "semver"
scopes = "auto"

[versioning]
tag_template = "{name}@{version}"

[[packages]]
name = "core"
path = "crates/core"

[[packages]]
name = "cli"
path = "crates/cli"
```

When `monorepo = true`, each package is bumped independently
based on commits touching its path. Packages are
auto-discovered from workspace manifests if `[[packages]]`
is omitted.

**Dependency cascade:** if package A bumps and package B
depends on A (runtime dependency in Cargo.toml or
package.json), B receives at least a patch bump. Use
`-p` to skip cascade.

**Per-package changelogs:** each bumped package gets a
`CHANGELOG.md` in its root directory. The root
`CHANGELOG.md` includes all commits.

**Mixed versioning schemes:**

```toml
monorepo = true
scheme = "semver"

[versioning]
tag_template = "{name}@{version}"
calver_format = "YYYY.0M.PATCH"

[[packages]]
name = "core"
path = "crates/core"

[[packages]]
name = "api"
path = "crates/api"
scheme = "calver"              # override: date-based versioning
```
