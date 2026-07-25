# Getting started

## Install

**Install script (recommended):**

```bash
curl -fsSL https://raw.githubusercontent.com/driftsys/git-std/main/install.sh | bash
```

**From source:**

```bash
cargo install git-std
```

**Shell completions:**

```bash
# Bash (~/.bashrc)
eval "$(git-std --completions bash)"

# Zsh (~/.zshrc)
eval "$(git-std --completions zsh)"

# Fish (~/.config/fish/config.fish)
git-std --completions fish | source
```

## Set up git-std in your repo

**Maintainer, one-time setup:**

```bash
git std init
```

This scaffolds `.githooks/` (hook templates + shims, prompting which hooks
to enable — default `pre-commit` and `commit-msg`), generates a
`./bootstrap` script for contributors, creates `.git-std.toml`, and appends
a post-clone reminder to `README.md`/`AGENTS.md`. Review the staged changes
and commit them.

**Contributor, after `git clone` or `git worktree add`:**

```bash
./bootstrap
```

This runs the environment checks and any project-specific bootstrap
commands (via `git std bootstrap` under the hood).

To set up only hooks without the rest of `init` — e.g. in a repo that
already has a `.git-std.toml` and bootstrap script — run
`git std hook install` directly.

## AI agent skills

Install the `std-commit` and `std-bump` skills for your AI coding agent with
[`upskill`](https://github.com/driftsys/upskill) (`cargo install upskill` or
see its install script):

```bash
upskill add driftsys/git-std
```

## Make a commit

Stage your changes and run the interactive builder:

```bash
git add .
git std commit
```

Or use non-interactive mode:

```bash
git std commit -m "feat(auth): add OAuth2 PKCE flow"
```

## Validate commits

```bash
git std lint "feat: add login"
git std lint --range main..HEAD
```

Use `--strict` to enforce types and scopes from
`.git-std.toml`.

## Preview changelog

```bash
git std changelog --stdout
```

## Bump, changelog, and tag

```bash
git std bump
git push --follow-tags
```

This analyses commits since the last tag, calculates the
next version, updates version files, generates the
changelog, creates a release commit, and tags it.

Use `--dry-run` to preview without writing anything.
