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

Prepare a release, finalize a merged release, or recover interrupted
publication.

Invoke with `/std-bump` in your agent.

The skill:

1. Reads repository policy, command availability, configuration and release state;
   unavailable tooling or unsafe checkout state produces a concrete blocker.
2. Resolves package selection, release process and publication scope, reusing
   authorization already given in the session.
3. Previews the bump or existing-release tag plan and asks only for decisions or
   approval that remain unresolved.
4. Validates the generated or merged release revision before publication, then
   publishes only the selected refs atomically when authorized.
5. Inspects actual refs on recovery and reports local changes, remote refs,
   validation and hosted publication as separate states.

## Release management

See [release workflows](releases.md) for the preferred combined commit-and-tag
process and the supported prepare-then-tag process. Agents use `std-bump` for
both release preparation and finalizing an already merged release.
