# git-std

From commit to release. One tool for
[conventional commits](https://www.conventionalcommits.org/en/v1.0.0/),
[versioning](https://semver.org/),
[changelog](https://keepachangelog.com/en/1.1.0/), and
[git hooks](https://git-scm.com/docs/githooks) management.

Stop juggling five tools to enforce commit standards, bump
versions, and manage hooks. `git-std` replaces commitizen,
commitlint, standard-version, husky, and lefthook with a
single, fast binary — zero runtime dependencies.

## Features

- **Structured commits** — interactive prompt for type,
  scope, and description, or non-interactive with `--message`
- **Commit validation** — lint messages inline, from file,
  or across a revision range
- **Version bumping** — semver, calver, and patch-only
  schemes, calculated automatically from commit history
- **Changelog generation** — incremental or full, built
  from conventional commits
- **Git hooks** — install, enable/disable, and auto-format
  staged files safely before commit
- **Lock file sync** — automatically update Cargo, npm,
  yarn, pnpm, deno, uv, and poetry lock files after bump
- **Custom version files** — update any file during bump
  using regex patterns
- **Configuration** — `.git-std.toml` with sensible
  defaults, inspectable via `config list`/`get`
- **Shell completions** — bash, zsh, fish
- **AI agent skills** — install commit and bump skills for your AI coding agent
  with `npx skills add driftsys/git-std`
- **CI-ready** — JSON output, non-zero exit codes, no
  interactive prompts in pipelines

## Quick start

```bash
git std hook install                # set up hooks
git std commit                       # interactive commit
git std bump                         # bump + changelog + tag
git push --follow-tags
```

Invoked as `git std` via git's subcommand discovery.
