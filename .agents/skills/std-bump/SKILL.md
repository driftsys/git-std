---
name: std-bump
description: Bump the project version using git std — use when asked to "bump", "release", "cut a release", or "tag a version".
---

## Workflow

**Step 1: Verify git std is installed**

- Run `git std --version`
- If it fails:
  - If `./bootstrap` exists at repo root: ask "git std is not installed. Run
    `./bootstrap` to install it?"
  - Otherwise: ask "git std is not installed. Install it now?"
  - If user declines, stop.

**Step 2: Get project context**

- Run `git std --context`
- If output shows `Not bootstrapped`, print and stop.
- Extract:
  - **Stable status**: Is `Stable: true` or `Stable: false`?
  - **Current branch**: Which branch are we on?
  - **Scheme**: What versioning scheme (semver, calver, etc.)?
  - **Tag prefix**: What's the tag prefix (v, release-, etc.)?

**Step 3: Ensure on release branch**

- If `Stable: true` (already on release branch):
  - Proceed to Step 4
- If `Stable: false` (not on release branch):
  - Ask: "You need to be on the release branch (main/master) to bump. Switch to
    main and pull latest?"
  - If user says "No": stop — do not bump outside release branch
  - If user says "Yes":
    - Run `git checkout main`
    - Run `git fetch origin`
    - Run `git pull origin main`
    - Re-run `git std --context` (get updated context)
    - Show `git log --oneline -5`

**Step 4: Check for sync issues**

- Run `git rev-list HEAD..origin/main --count`
- If count > 0 (we're behind):
  - Ask: "⚠ Local branch is behind origin. Pull first?"
  - If "Yes": run `git pull origin main`
  - If "No": ask "Continue bumping anyway?" (confirm user knows)
- If count = 0: proceed

**Step 5: Identify bump type**

Ask user: "What type of bump?" with options:

- `--prerelease` (alpha, beta, rc releases)
- Regular release (standard semantic version bump)
- `--first-release` (if no tags exist yet)

**Step 6: Select packages (if multi-package workspace)**

- Run `git std bump --dry-run` to determine available packages
- If workspace has multiple packages:
  - Ask: "Which packages to bump?" with options:
    - "All packages"
    - Each package name individually (multi-select)
  - If individual: collect list of selected package names
- If single package workspace: skip this step

**Step 7: Show dry-run and get approval**

- Run `git std bump --dry-run` with all flags determined so far
- Display the **full dry-run output** to user
- If the output shows **"no bump-worthy commits found"**:
  - Only `feat`/`fix`/`perf`/`revert`/breaking changes trigger a bump —
    `docs`/`style`/`refactor`/`test`/`chore`/`ci`/`build` never do (matches
    the Conventional Commits spec; not configurable).
  - Ask: "No bump-worthy commits since the last tag. Force a release anyway
    (e.g. for a docs-only change)?" with options:
    - "No, don't bump" (stop here)
    - "Yes, force patch" → add `--release-as patch`
    - "Yes, force minor" → add `--release-as minor`
    - "Yes, force major" → add `--release-as major`
  - Re-run `git std bump --dry-run` with the chosen `--release-as` flag and
    show the updated plan
- If the command instead exits non-zero with a message mentioning
  `--first-major-release` (the dry-run plan would promote the project from
  `0.x` to `1.0.0`):
  - This is a deliberate API-stability commitment, not a routine bump. Ask
    the user explicitly: "This bump would promote the project from 0.x to
    1.0.0 — a stability commitment for the public API. What's changed that
    justifies declaring 1.0 now? Should we proceed, or hold in 0.x for now?"
  - If the user confirms: add `--first-major-release` and re-run
    `git std bump --dry-run` to show the updated plan
  - If the user is unsure or declines: stop here — do not add the flag
- Otherwise, show what will be bumped, new versions, and tags that will be created
- Ask: "Proceed with this version bump?" (Yes/No)
- **Do not proceed without explicit approval**

**Step 8: Confirm push strategy**

- Ask: "Push commit and tags to origin after bumping?" (Yes/No)
- Note: If "No", user must push manually later

**Step 9: Execute the bump**

- Run `git std bump` with all confirmed flags:
  - `--prerelease` if user selected prerelease
  - `--first-release` if applicable
  - `--release-as <level>` if the user chose to force a bump in Step 7
  - `--first-major-release` if the user confirmed the 0.x → 1.0 promotion in Step 7
  - `--package <name>` for each selected package
  - `--push` if user confirmed push in Step 8
- Display the result (commit hash, new version, tags created)
- Show `git log --oneline -3` to confirm
