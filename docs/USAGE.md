# Commands

```bash
git std <command> [options]
```

## Global Flags

| Flag                    | Description                          |
| ----------------------- | ------------------------------------ |
| `--help` / `-h`         | Print help                           |
| `--version` / `-V`      | Print version                        |
| `--color <when>`        | `auto` (default), `always`, `never`  |
| `--completions <shell>` | Generate shell completions to stdout |
| `--update`              | Update git-std to the latest release |

## `git std lint`

Validate commit messages against the
[Conventional Commits][conv-commits] specification.

**Input modes:**

```bash
git std lint "feat: add login"                    # inline message
git std lint --file .git/COMMIT_EDITMSG           # from file (strips # comments)
git std lint --range main..HEAD                   # all commits in a range
```

**Flags:**

| Flag              | Description                                  |
| ----------------- | -------------------------------------------- |
| `--file <path>`   | Read message from file                       |
| `--range <range>` | Validate all commits in a git revision range |
| `--strict`        | Enforce types/scopes from `.git-std.toml`    |
| `--format <fmt>`  | Output format: `text` (default) or `json`    |

**Exit codes:** `0` = valid, `1` = invalid, `2` = I/O or usage error.

**Examples:**

```bash
# Validate a single message
git std lint "feat(auth): add OAuth2 PKCE flow"

# Validate all commits on a branch
git std lint --range main..HEAD

# Strict mode — reject unknown types and scopes
git std lint --strict --range main..HEAD

# As a commit-msg hook
git std lint --file "$1"
```

## `git std commit`

Interactive conventional commit builder. Prompts for type,
scope, description, body, and breaking change, then runs
`git commit`.

**Flags:**

| Flag               | Description                       |
| ------------------ | --------------------------------- |
| `--type <type>`    | Pre-fill type, skip prompt        |
| `--scope <scope>`  | Pre-fill scope, skip prompt       |
| `--message <msg>`  | Non-interactive mode              |
| `--body <text>`    | Commit body paragraph             |
| `--breaking <msg>` | Add `BREAKING CHANGE` footer      |
| `--footer <text>`  | Add a trailer footer (repeatable) |
| `--signoff` / `-s` | Add `Signed-off-by` trailer       |
| `--dry-run`        | Print message without committing  |
| `--amend`          | Pass `--amend` to `git commit`    |
| `--sign` / `-S`    | GPG-sign the commit               |
| `--all` / `-a`     | Stage tracked changes             |

**Exit codes:** `0` = committed, `1` = validation/git error, `2` = usage error.

## `git std bump`

Calculate the next version from conventional commits,
update version files, generate changelog, commit, and tag.

**Flags:**

| Flag                 | Description                                                        |
| -------------------- | ------------------------------------------------------------------ |
| `--dry-run`          | Print plan without writing                                         |
| `--prerelease [tag]` | Bump as pre-release (e.g. `2.0.0-rc.1`)                            |
| `--release-as <ver>` | Force a specific version                                           |
| `--first-release`    | Initial changelog, no bump                                         |
| `--no-tag`           | Skip tag creation                                                  |
| `--no-commit`        | Update files only                                                  |
| `--sign` / `-S`      | GPG-sign commit and tag                                            |
| `--skip-changelog`   | Bump without changelog                                             |
| `--force`            | Allow breaking changes in patch-only scheme                        |
| `--stable [branch]`  | Create a stable branch for patch-only releases                     |
| `--minor`            | Use minor bump (instead of major) when advancing main after stable |
| `--format <fmt>`     | Output format: `text` (default) or `json`                          |
| `--package <name>`   | Filter bump to specific package(s) (monorepo only, repeatable)     |
| `--push [remote]`    | Push commit and tags after release (default remote: `origin`)      |
| `--yes` / `-y`       | Skip branch confirmation prompt                                    |

**Exit codes:** `0` = success, `1` = error.

### Monorepo bump

When `monorepo = true`, each package is versioned
independently based on commits touching its path.

```bash
# Bump all packages with changes
git std bump

# Preview monorepo bump
git std bump --dry-run

# Bump specific package(s)
git std bump -p core
git std bump -p core -p cli

# JSON output for CI
git std bump --dry-run --format json
```

Dependency cascade: when package A bumps and package B
depends on A, B gets at least a patch bump. Use `-p` to
skip cascade and bump only the named packages.

## `git std changelog`

Generate or update the changelog from git history.

**Flags:**

| Flag                   | Description                              |
| ---------------------- | ---------------------------------------- |
| `--full`               | Regenerate entire changelog              |
| `--range <range>`      | Generate for a tag range (e.g. `v1..v2`) |
| `-w`, `--write [path]` | Write to file (default: CHANGELOG.md)    |

Output goes to stdout by default. Pass `-w` / `--write` to
write to `CHANGELOG.md`, or `-w <path>` for a custom path.
`--full` and `--range` are mutually exclusive. Without
either, generates an incremental changelog from unreleased
commits since the last tag.

## `git std init`

Scaffold hooks, bootstrap script, and README section in one step.
Consolidates hook setup and bootstrap scaffolding for maintainers.

```bash
git std init            # scaffold everything interactively
git std init --force    # overwrite existing files
```

**What it does:**

1. Creates `.githooks/` directory.
2. Sets `core.hooksPath` to `.githooks`.
3. Writes `.hooks` templates (`pre-commit`, `commit-msg`, `pre-push`, etc.).
4. Prompts which hooks to enable, writes shims.
5. Generates `./bootstrap` script.
6. Generates `.githooks/bootstrap.hooks`.
7. Creates `.git-std.toml` with taplo schema directive (if absent).
8. Appends post-clone section to `README.md` and `AGENTS.md` (if found).
9. Stages all created files.

**Flags:**

| Flag      | Description              |
| --------- | ------------------------ |
| `--force` | Overwrite existing files |

**Exit codes:** `0` = success, `1` = error.

## `git std hook`

Manage git hooks defined in `.githooks/*.hooks` files.

```bash
git std hook install          # set up hooks directory and shim scripts
git std hook run <hook>       # execute a hook manually
git std hook list             # display configured hooks
git std hook enable <hook>    # activate a hook (rename .off → shim)
git std hook disable <hook>   # deactivate a hook (rename shim → .off)
```

**Subcommands:**

| Subcommand       | Description                                    |
| ---------------- | ---------------------------------------------- |
| `install`        | Write shim scripts and `.hooks` templates      |
| `run <hook>`     | Execute a hook manually                        |
| `list`           | Display all hooks with enabled/disabled status |
| `enable <hook>`  | Activate a disabled hook                       |
| `disable <hook>` | Deactivate an enabled hook                     |

**Known hook types:** `pre-commit`, `commit-msg`, `pre-push`,
`post-commit`, `prepare-commit-msg`, `post-merge`.

**Flags (run and list):**

| Flag             | Description                             |
| ---------------- | --------------------------------------- |
| `--format <fmt>` | Output format: `text` (default), `json` |

## `git std bootstrap`

Post-clone environment setup. Detects convention files and
configures the local environment.

```bash
git std bootstrap              # run built-in checks + bootstrap.hooks
git std bootstrap --dry-run    # print what would be done
git std bootstrap install      # scaffold bootstrap files for contributors
```

**Built-in checks:**

| Convention file          | Action                                                   |
| ------------------------ | -------------------------------------------------------- |
| `.githooks/`             | `git config core.hooksPath .githooks`                    |
| `.gitattributes`         | `git lfs install` + `git lfs pull` (if `filter=lfs`)     |
| `.git-blame-ignore-revs` | `git config blame.ignoreRevsFile .git-blame-ignore-revs` |

After built-in checks, runs `.githooks/bootstrap.hooks` if present.

**`bootstrap install` flags:**

| Flag      | Description              |
| --------- | ------------------------ |
| `--force` | Overwrite existing files |

**`bootstrap` flags:**

| Flag        | Description                             |
| ----------- | --------------------------------------- |
| `--dry-run` | Print what would be done without acting |

## `git std doctor`

Show everything about your local git-std setup in one command.
Three sections: **Status**, **Hooks**, **Configuration**.
Problems appear as hints at the bottom.

```bash
git std doctor              # show all sections, exit 0 (no problems) or 1
git std doctor --format json  # machine-readable JSON on stdout
```

**Sections:**

| Section         | Contents                                                                                                    |
| --------------- | ----------------------------------------------------------------------------------------------------------- |
| `Status`        | Tool versions: `git`, `git-lfs` (if `.gitattributes` needs it), `git-std` with update notice if available   |
| `Hooks`         | All `.githooks/*.hooks` files with commands and sigils (`!` required, `?` advisory), enabled/disabled state |
| `Configuration` | All `.git-std.toml` keys; explicit values in bold, defaults plain/dim                                       |

**Example output:**

```text
  Status
    git 2.43.0
    git-lfs 3.4.1
    git-std 0.11.3 (update available: 0.12.0)

  Hooks
    commit-msg
      !  git std lint -f
    pre-commit
      !  cargo fmt --check
      !  cargo clippy
    pre-push (disabled)
      !  just verify

  Configuration
    scheme           semver
    strict           true
    ...

  hint: git-lfs not found — required by .gitattributes
  hint: .git-std.toml invalid: expected `=` at line 3
```

**Flags:**

| Flag             | Description                             |
| ---------------- | --------------------------------------- |
| `--format <fmt>` | Output format: `text` (default), `json` |

**Exit codes:** `0` = no problems, `1` = one or more hints surfaced,
`2` = not a git repository.

## `git std version`

Lightweight, scriptable version queries.

```bash
git std version                  # 0.11.3
git std version --describe       # 0.11.3-dev.7+g3a2b1c.dirty
git std version --next           # 0.12.0
git std version --label          # minor
git std version --code           # 10299
git std version --format json    # all fields as JSON
```

Output goes to stdout. No `v` prefix.

**Flags:**

| Flag             | Description                                                           |
| ---------------- | --------------------------------------------------------------------- |
| `--describe`     | Cargo-style describe: `-dev.N` pre-release + `+hash[.dirty]` metadata |
| `--next`         | Next version from conventional commits since the last tag             |
| `--label`        | Bump label (`major`/`minor`/`patch`/`none`), accounting for pre-1.0   |
| `--code`         | Integer version code                                                  |
| `--format <fmt>` | Output format: `text` (default), `json`                               |

**Exit codes:** `0` = success, `1` = error.

## `--completions <shell>`

Generate shell completion scripts to stdout. The output includes wrappers
that enable completion for both `git-std` and `git std` invocations.
Works without a subcommand — usable regardless of install method.

```bash
git std --completions bash   # Bash
git std --completions zsh    # Zsh
git std --completions fish   # Fish
```

Add to your shell profile:

```bash
# Bash (~/.bashrc)
eval "$(git-std --completions bash)"

# Zsh (~/.zshrc)
eval "$(git-std --completions zsh)"

# Fish (~/.config/fish/config.fish)
git-std --completions fish | source
```

## `git std config`

Inspect effective configuration loaded from `.git-std.toml`.

```bash
git std config list              # print all settings with source annotations
git std config list --format json  # machine-readable JSON on stdout
git std config get <key>         # print a single value to stdout
git std config get <key> --format json  # value as JSON
```

**Subcommands:**

| Subcommand  | Description                                   |
| ----------- | --------------------------------------------- |
| `list`      | Print all effective config grouped by section |
| `get <key>` | Print a single dot-separated key value        |

**Flags (list and get):**

| Flag             | Description                             |
| ---------------- | --------------------------------------- |
| `--format <fmt>` | Output format: `text` (default), `json` |

**Supported keys for `get`:**

`scheme`, `strict`, `types`, `scopes`,
`versioning.tag_prefix`, `versioning.prerelease_tag`, `versioning.calver_format`,
`changelog.title`, `changelog.hidden`, `changelog.sections`, `changelog.bug_url`

**Exit codes:** `0` = success, `1` = unknown key or error.

**Example output:**

```text
$ git std config list
  scheme = semver                            (default)
  strict = false                             (default)
  types = [feat, fix, docs, ...]             (default)
  scopes = none                              (default)

  [versioning]
  tag_prefix = v                             (default)
  ...
```

## Update Check

git-std periodically checks for newer releases in the background and
prints a hint after command output when an update is available.

- Non-blocking — a detached background process fetches the latest
  release once every 24 hours.
- Adapts to install method (cargo, install.sh, nix).
- Opt-out: `GIT_STD_NO_UPDATE_CHECK=1`.

[conv-commits]: https://www.conventionalcommits.org/en/v1.0.0/
