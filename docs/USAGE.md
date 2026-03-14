# git-std — Usage

> `git std` — standard git workflow from commit to release.

## Synopsis

```bash
git std <command> [options]
```

## Commands

### `git std check`

Validate commit messages against the [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) specification.

**Input modes:**

```bash
git std check "feat: add login"                    # inline message
git std check --file .git/COMMIT_EDITMSG           # from file (strips # comments)
git std check --range main..HEAD                   # all commits in a range
```

**Flags:**

| Flag              | Description                                  |
| ----------------- | -------------------------------------------- |
| `--file <path>`   | Read message from file                       |
| `--range <range>` | Validate all commits in a git revision range |
| `--strict`        | Enforce types/scopes from `.git-std.toml`    |

**Exit codes:** `0` = valid, `1` = invalid, `2` = I/O or usage error.

**Examples:**

```bash
# Validate a single message
git std check "feat(auth): add OAuth2 PKCE flow"

# Validate all commits on a branch
git std check --range main..HEAD

# Strict mode — reject unknown types and scopes
git std check --strict --range main..HEAD

# As a commit-msg hook
git std check --file "$1"
```

### `git std commit`

Interactive conventional commit builder. Prompts for type, scope, description, body, and breaking change, then runs `git commit`.

**Flags:**

| Flag              | Description                      |
| ----------------- | -------------------------------- |
| `--type <type>`   | Pre-fill type, skip prompt       |
| `--scope <scope>` | Pre-fill scope, skip prompt      |
| `--message <msg>` | Non-interactive mode             |
| `--breaking`      | Add `BREAKING CHANGE` footer     |
| `--dry-run`       | Print message without committing |
| `--amend`         | Pass `--amend` to `git commit`   |
| `--sign` / `-S`   | GPG-sign the commit              |
| `--all` / `-a`    | Stage tracked changes            |

**Exit codes:** `0` = committed, `1` = validation/git error, `2` = usage error.

### `git std bump`

Calculate the next version from conventional commits, update version files, generate changelog, commit, and tag.

**Flags:**

| Flag                 | Description                              |
| -------------------- | ---------------------------------------- |
| `--dry-run`          | Print plan without writing               |
| `--prerelease [tag]` | Bump as pre-release (e.g., `2.0.0-rc.1`) |
| `--release-as <ver>` | Force a specific version                 |
| `--first-release`    | Initial changelog, no bump               |
| `--no-tag`           | Skip tag creation                        |
| `--no-commit`        | Update files only                        |
| `--sign`             | GPG-sign commit and tag                  |
| `--skip-changelog`   | Bump without changelog                   |

**Exit codes:** `0` = success, `1` = error.

### `git std changelog`

Generate or update the changelog from git history.

**Flags:**

| Flag              | Description                           |
| ----------------- | ------------------------------------- |
| `--full`          | Regenerate entire changelog           |
| `--stdout`        | Print to stdout instead of file       |
| `--output <file>` | Write to file (default: CHANGELOG.md) |

### `git std hooks`

Manage git hooks defined in `.githooks/*.hooks` files.

```bash
git std hooks install    # set up hooks directory and shim scripts
git std hooks run <hook> # execute a hook manually
git std hooks list       # display configured hooks
```

### `git std self-update`

Fetch the latest release and replace the current binary.

## Global Flags

| Flag               | Description                         |
| ------------------ | ----------------------------------- |
| `--help` / `-h`    | Print help                          |
| `--version` / `-V` | Print version                       |
| `--color <when>`   | `auto` (default), `always`, `never` |
| `--quiet` / `-q`   | Suppress non-error output           |

---

## Configuration

`git-std` reads `.git-std.toml` in the project root. All fields are optional — sensible defaults are used when the file is absent or a field is omitted.

### Full schema

```toml
# ── Project ───────────────────────────────────────────────────────
scheme = "semver"                              # semver | calver | patch
types = ["feat", "fix", "docs", "style",
         "refactor", "perf", "test",
         "chore", "ci", "build"]
scopes = ["auth", "api", "ci", "deps"]         # "auto" | string[] | omit
strict = true                                  # enforce types/scopes without --strict flag

# ── Versioning ────────────────────────────────────────────────────
[versioning]
tag_prefix = "v"                               # git tag prefix
prerelease_tag = "rc"                          # default pre-release id
calver_format = "YYYY.MM.PATCH"                # only when scheme = "calver"

# ── Changelog ─────────────────────────────────────────────────────
[changelog]
hidden = ["chore", "ci", "build", "style", "test"]

[changelog.sections]
feat = "Features"
fix = "Bug Fixes"
perf = "Performance"
refactor = "Refactoring"
docs = "Documentation"
```

### Fields

#### Top-level

| Field    | Type                 | Default           | Description                                             |
| -------- | -------------------- | ----------------- | ------------------------------------------------------- |
| `scheme` | string               | `"semver"`        | Versioning scheme: `semver`, `calver`, or `patch`       |
| `types`  | string[]             | 10 standard types | Allowed conventional commit types                       |
| `scopes` | `"auto"` or string[] | None              | Scope discovery or explicit allowlist                   |
| `strict` | bool                 | `false`           | Enforce types/scopes validation without `--strict` flag |

Default types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`, `ci`, `build`.

**Scopes behavior:**

- **Not set** (default) — no scope validation, any scope accepted
- **`scopes = "auto"`** — discover scopes from workspace layout (`crates/*`, `packages/*`, `modules/*`)
- **`scopes = ["auth", "api"]`** — explicit allowlist

When scopes is set (either `"auto"` or an array) and `--strict` is used, a scope is required and must be in the resolved list. For `git std commit`, the resolved scopes populate the interactive scope prompt.

#### `[versioning]`

| Field            | Type   | Default           | Description                                             |
| ---------------- | ------ | ----------------- | ------------------------------------------------------- |
| `tag_prefix`     | string | `"v"`             | Git tag prefix (e.g., `v1.0.0`)                         |
| `prerelease_tag` | string | `"rc"`            | Default pre-release identifier                          |
| `calver_format`  | string | `"YYYY.MM.PATCH"` | Calendar version format (only when `scheme = "calver"`) |

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

Common formats: `YYYY.MM.PATCH` (monthly releases), `YYYY.0M.PATCH` (zero-padded month), `YY.WW.DP` (weekly with day-of-week), `YYYY.MM.DD.PATCH` (daily releases).

Bump rules are inferred from the scheme — not configurable. For semver: `BREAKING CHANGE` or `!` triggers major, `feat` triggers minor, everything else triggers patch.

#### `[changelog]`

| Field    | Type     | Default                                     | Description                   |
| -------- | -------- | ------------------------------------------- | ----------------------------- |
| `hidden` | string[] | `["chore", "ci", "build", "style", "test"]` | Types excluded from changelog |

#### `[changelog.sections]`

Maps commit types to changelog section headings. Types not listed here use the type name as the heading.

| Key        | Default           |
| ---------- | ----------------- |
| `feat`     | `"Features"`      |
| `fix`      | `"Bug Fixes"`     |
| `perf`     | `"Performance"`   |
| `refactor` | `"Refactoring"`   |
| `docs`     | `"Documentation"` |

### Inferred settings

These are not configurable — git-std resolves them automatically:

| Concern          | Resolution                                     |
| ---------------- | ---------------------------------------------- |
| Bump rules       | Inferred from `scheme`                         |
| Version files    | Auto-detected (Cargo.toml)                     |
| URLs             | Inferred from `git remote get-url origin`      |
| Changelog output | Always `CHANGELOG.md`                          |
| Release commit   | Always `chore(release): <version>`             |

### Minimal examples

**No config needed** — git-std works with zero configuration using conventional defaults.

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

---

## CI Integration

```yaml
# GitHub Actions
- name: Validate commits
  run: git std check --range ${{ github.event.pull_request.base.sha }}..${{ github.sha }}
```

```yaml
# GitLab CI
lint:commits:
  script:
    - git std check --range $CI_MERGE_REQUEST_DIFF_BASE_SHA..HEAD
```

## Hooks Integration

Create `.githooks/commit-msg.hooks`:

```text
! git std check --file {msg}
```

Then install:

```bash
git std hooks install
```

Every commit message will be validated automatically.
