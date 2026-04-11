---
name: std-commit
description: Author a conventional commit for staged changes using git std — use when asked to "commit", "write a commit", or "commit my changes".
---

## Workflow

**Step 1: Verify git std is installed**

- Run `git std --version`
- If it fails:
  - If `./bootstrap` exists at repo root: ask "git std is not installed — run `./bootstrap` to install it?"
  - Otherwise: ask "git std is not installed — install it now?" with `curl -fsSL https://driftsys.github.io/git-std/install.sh | bash`
  - If user declines, stop.

**Step 2: Get project context**

- Run `git std --context`
- If output shows `Not bootstrapped` or `Nothing staged`, print the message and stop.
- Otherwise, extract:
  - Available **Types** from context (e.g., feat, fix, docs, test, chore)
  - Available **Scopes** from context
  - Whether scopes are `(required, strict)` — if so, `--scope` flag is mandatory

**Step 3: Determine commit type and scope**

- Use **only** the types and scopes from context — never invent either.
- For scope: match changed file paths against workspace package names
  - If diff spans multiple scopes, pick the most-changed one
  - If scopes are `(required, strict)` and no scope determined, ask user

**Step 4: Construct the commit message**

- **Subject line** (`--message`):
  - Imperative mood, lowercase, no trailing period
  - **Limit to 50 characters**
  - Example: "add login flow" (not "added login flow" or "Add login flow")

- **Body** (`--body`, optional if extended context needed):
  - Wrap at 72 characters per line
  - Explain _what_ changed and _why_ — not _how_ (the diff shows that)
  - Aim for 2–5 sentences
  - Example: "The cache invalidation routine was checking stale entries after acquiring the lock, creating a window where two threads could invalidate the same entry. Wrap the check-and-clear in a single lock acquisition."

**Step 5: Check for special markers**

- **Breaking change**: If diff contains one, add `--breaking "short description"`
- **Issue reference**:
  - Extract from branch name if it matches `{type}/{issue}-{description}`
  - For `feat` or `fix` type, ask: "Related issue? (e.g. #123 — leave blank to skip)"
  - If issue provided, add `--footer "Closes #N"`

**Step 6: Assemble and approve**

- Show the proposed `git std commit` command with all flags
- Ask: "Proceed with this commit?"
- Do **not** run the command without explicit user approval
