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
  - If context shows `Stable: true`, we are on the release branch — proceed.
  - If context shows `Stable: false` and the current branch is not main or master,
    run `git fetch origin`, then ask: "You're not on main — switch to main and
    pull origin/main first?"
    If confirmed, run `git checkout main && git pull origin main`, then re-run
    `git std --context` to refresh state, and show `git status` and
    `git log --oneline -5` so the user can review before proceeding.
    If declined, stop — do not run bump outside the release branch.
  - If context shows `Stable: false` and already on main or master (prerelease tag
    on the release branch), run `git fetch origin`, check sync with
    `git rev-list HEAD..origin/main --count` — if local is behind origin,
    ask: "⚠ local main is N commits behind origin/main — pull before bumping?"
    If confirmed, run `git pull origin main`. Then proceed.
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
