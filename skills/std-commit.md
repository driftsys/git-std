---
name: std-commit
description: Author a conventional commit for staged changes using git std — use when asked to "commit", "write a commit", or "commit my changes".
---

## Workflow

**Step 1: Verify git std is installed**

- Run `git std --version`
- If it fails:
  - If `./bootstrap` exists at repo root: ask "git std is not installed — run
    `./bootstrap` to install it?"
  - Otherwise: ask "git std is not installed — install it now?" with
    `curl -fsSL https://driftsys.github.io/git-std/install.sh | bash`
  - If user declines, stop.

**Step 2: Get project context**

- Run `git std --context`
- If output shows `Not bootstrapped` or `Nothing staged`, print the message and
  stop.
- Otherwise, extract:
  - Available **Types** from context (e.g., feat, fix, docs, test, chore)
  - Available **Scopes** from context
  - Whether scopes are `(required, strict)` — if so, `--scope` flag is mandatory

**Step 3: Determine commit type and scope**

- Use **only** the types and scopes from context — never invent either.
- **Type selection**:
  - If context shows `Suggested type`, present it: "Type appears to be
    [suggestion] — confirm or pick another?"
  - If `Suggested type` is a shortlist like "feat or fix", ask user to choose
    by reading the diff
  - Otherwise ask user to select from available types
- If scopes are configured, ask user to select or skip
- For scope: match changed file paths against workspace package names if
  possible
  - If diff spans multiple scopes, pick the most-changed one
  - If scopes are `(required, strict)` and no scope determined, require user
    selection

**Step 4: Construct the commit message**

- **Subject line** (`--message`):
  - Ask user: "Commit subject (imperative mood, lowercase, max 50 chars)?"
  - Validate: length ≤ 50 characters
  - Example: "add login flow" (not "added login flow" or "Add login flow")

- **Body** (`--body`, optional):
  - Ask user: "Add a body? (yes/no)"
  - If yes, ask: "Body (wrap at 72 characters per line; enter empty line to finish)"
  - Explain _what_ changed and _why_ — not _how_ (the diff shows that)
  - Aim for 2–5 sentences

**Step 5: Check for special markers**

- **Breaking change**:
  - Ask: "Is this a breaking change? (yes/no)"
  - If yes, ask: "Describe the breaking change"
  - Will add `--breaking "description"` flag

- **Issue reference**:
  - Ask: "Related issue? (e.g., #123 — leave blank to skip)"
  - If provided, will add `--footer "Closes #N"` flag

**Step 6: Assemble and approve**

- Construct the full `git std commit` command with all flags
- Show the proposed commit to the user:
  ```
  Type:     <type>
  Scope:    <scope> (if applicable)
  Subject:  <subject>
  Body:     <body preview> (if applicable)
  Breaking: <breaking> (if applicable)
  Issue:    <issue> (if applicable)

  Command:
    git std commit <all flags>
  ```
- Ask: "Proceed with this commit? (yes/no)"
- **Do not run the command without explicit user approval**

**Step 7: Execute**

- If user approves, run the assembled `git std commit` command
- Display the created commit: show `git log -1 --oneline` output
- If user cancels, stop without committing
