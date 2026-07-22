# Skills

Agent skills for Claude Code, OpenCode, and GitHub Copilot.

Skills are authored under `skills/` — the
[`upskill`](https://github.com/driftsys/upskill) source-registry root — and
generated into `.claude/skills/` and `.agents/skills/` via:

```bash
upskill add ./ --claude --opencode
```

(or `just skills`). Regenerate after any change to `skills/` and commit the
generated output alongside the SSOT change.

## /std-commit

Author a conventional commit for staged changes.

Invoke with `/std-commit` in your agent.

The skill:

1. Checks `git std` is installed — offers to install via `./bootstrap` or
   the install script if not.
2. Runs `git std --context` to read project config, valid types, scopes,
   and the staged diff.
3. Proposes a `git std commit --type X [--scope Y] --message Z` command.
4. For `feat` and `fix` commits, asks for a related issue number and
   pre-fills it from the branch name when available
   (e.g. `feat/123-my-feature` → `#123`).
5. Requires your approval before running.

## /std-bump

Orchestrate a version bump.

Invoke with `/std-bump` in your agent.

The skill:

1. Checks `git std` is installed — offers to install if not.
2. Runs `git std --context` to assess stability, branch, and tag state.
3. Runs `git std bump --dry-run` and shows the full plan.
4. Asks for confirmation, package selection (monorepo), and whether to push.
5. Requires your approval before running.
