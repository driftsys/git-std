---
name: std-bump
description: Bump the project version using git std — use when asked to "bump", "release", "cut a release", or "tag a version".
---

Orchestrate a version bump using `git std bump`.

## Rules

- If `git std --version` fails:
  - If `./bootstrap` exists at repo root, ask: "git std is not installed —
    run `./bootstrap` to install it?" If confirmed, run it.
  - Otherwise ask: "git std is not installed — install it now?" If confirmed,
    run `curl -fsSL https://driftsys.github.io/git-std/install.sh | bash`
- Run `git std --context` to assess project state:
  - If `Not bootstrapped`, stop and print the message.
  - If not on a stable branch (main/master), suggest `--prerelease` unless
    the user explicitly asks for a stable release.
  - If context shows no tag yet, use `--first-release`.
- Run `git std bump --dry-run` and show the output. Ask: "Proceed with this bump?"
  Do not continue without confirmation.
- If the workspace has multiple packages, ask: "Bump all packages or specific
  ones? (leave blank for all, or list e.g. git-std, standard-commit)"
  Add `--package` flags if specific packages are named.
- Ask: "Push commit and tags after? (--push)" before running.
- Run `git std bump [--prerelease] [--first-release] [--package ...] [--push]`
  with the confirmed flags.

Do not run any bump command without the user's approval.
