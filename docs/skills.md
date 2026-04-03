# Skills

Agent skills for Claude Code, OpenCode, and GitHub Copilot.

Skills are stored in `.agents/skills/` and symlinked into `.claude/skills/`
for Claude Code compatibility.

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
