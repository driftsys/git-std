---
name: std-commit
description: Author a conventional commit for staged changes using git std — use when asked to "commit", "write a commit", or "commit my changes".
---

Run `git std --context`, then author a `git std commit` invocation for the staged changes.

## Rules

- If `git std --version` fails:
  - If `./bootstrap` exists at repo root, ask: "git std is not installed —
    run `./bootstrap` to install it?" If confirmed, run it.
  - Otherwise ask: "git std is not installed — install it now?" If confirmed,
    run `curl -fsSL https://driftsys.github.io/git-std/install.sh | bash`
- Use only the **Types** and **Scopes** listed in the context — never invent either.
- If scopes are `(required, strict)`, `--scope` is mandatory.
- If the output signals `Not bootstrapped` or `Nothing staged`, print the message and stop.
- Match changed file paths against workspace package names to determine `--scope`.
  If the diff spans multiple scopes, pick the most-changed one.
- `--message`: imperative mood, lowercase, no trailing period.
- If the diff contains a clear breaking change, add `--breaking "short description"`.
- For issue refs:
  - If context specifies that refs are required for this commit type, ask for the
    ref and do not proceed without one.
  - Otherwise, if `--type` is `feat` or `fix`, ask:
    "Related issue? (e.g. #123 — leave blank to skip)"
  - If the branch name follows `{type}/{issue}-{description}`, extract the issue
    number and pre-fill it as the default.
  - If provided, append `--footer "Closes #N"`.

Do not run the command without the user's approval.
