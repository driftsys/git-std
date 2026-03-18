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
eval "$(git-std completions bash)"

# Zsh (~/.zshrc)
eval "$(git-std completions zsh)"

# Fish (~/.config/fish/config.fish)
git-std completions fish | source
```

## Set up hooks

```bash
git std hooks install
```

This sets `core.hooksPath`, writes shim scripts, and
prompts which hooks to enable. Default: `pre-commit` and
`commit-msg`.

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
git std check "feat: add login"
git std check --range main..HEAD
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
