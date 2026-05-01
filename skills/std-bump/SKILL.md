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
- Show what will be bumped, new versions, and tags that will be created
- Ask: "Proceed with this version bump?" (Yes/No)
- **Do not proceed without explicit approval**

**Step 8: Confirm push strategy**

- Ask: "Push commit and tags to origin after bumping?" (Yes/No)
- Note: If "No", user must push manually later

**Step 9: Execute the bump**

- Run `git std bump` with all confirmed flags:
  - `--prerelease` if user selected prerelease
  - `--first-release` if applicable
  - `--package <name>` for each selected package
  - `--push` if user confirmed push in Step 8
- Display the result (commit hash, new version, tags created)
- Show `git log --oneline -3` to confirm
