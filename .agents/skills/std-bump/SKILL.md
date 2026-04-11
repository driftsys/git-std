---
name: std-bump
description: Bump the project version using git std — use when asked to "bump", "release", "cut a release", or "tag a version".
---

## Workflow

**Step 1: Verify git std is installed**

- Run `git std --version`
- If it fails:
  - If `./bootstrap` exists at repo root: ask "git std is not installed — run `./bootstrap` to install it?"
  - Otherwise: ask "git std is not installed — install it now?" with `curl -fsSL https://driftsys.github.io/git-std/install.sh | bash`
  - If user declines, stop.

**Step 2: Assess project state and branch**

- Run `git std --context`
- If output shows `Not bootstrapped`, print the message and stop.
- Check `Stable:` status:
  - If `Stable: true` — already on release branch, proceed to Step 3
  - If `Stable: false` — need to switch to main/master

**Step 3: Sync with main (if needed)**

Only if `Stable: false` from Step 2:

- Run `git fetch origin`
- If current branch is NOT main or master:
  - Ask: "You're not on main — switch to main and pull origin/main first?"
  - If yes: run `git checkout main && git pull origin main`, then re-run `git std --context` and show `git status` + `git log --oneline -5`
  - If no: stop — do not bump outside release branch
- If already on main or master:
  - Check sync: run `git rev-list HEAD..origin/main --count`
  - If behind origin: ask "⚠ local main is N commits behind origin/main — pull before bumping?"
  - If yes: run `git pull origin main`

**Step 4: Plan the bump**

- Run `git std bump --dry-run` and show the full output
- Ask: "Proceed with this bump?" (must confirm before continuing)
- If `--first-release` is needed (no tags yet), note this will be used

**Step 5: Select packages (if multi-package workspace)**

Only if workspace has multiple packages:

- Ask: "Bump all packages or specific ones? (leave blank for all, or list e.g. git-std, standard-commit)"
- If specific packages: add `--package <name>` for each

**Step 6: Confirm push strategy**

- Ask: "Push commit and tags after? (--push)"
- Note: if user says no, they'll need to push manually

**Step 7: Execute the bump**

- Run `git std bump [flags]` with:
  - `--prerelease` if needed
  - `--first-release` if needed
  - `--package <name>` for each specified package
  - `--push` if user confirmed in Step 6
- Do **not** run without explicit user approval from Step 4
