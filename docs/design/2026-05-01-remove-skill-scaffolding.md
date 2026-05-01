# Design: Remove skill scaffolding from `git std init`

**Date:** 2026-05-01
**Issue:** #488 (superseded — closing as won't-implement)
**Epic:** #439

## Context

`git std init` currently scaffolds AI agent skill files into three locations:

- `skills/*.md` — canonical source files committed to the repo
- `.agents/skills/<name>/SKILL.md` — symlinks (or text pointers on Windows) to
  the source files
- `.claude/skills/<name>` — symlinks pointing to `.agents/skills/<name>`

This coupling is problematic:

- Symlinks are fragile cross-platform (broken on Windows, invisible to
  `git archive`)
- `init` conflates project bootstrap with agent tooling lifecycle
- A dedicated `skill add` copy-based command (the original #488 proposal) moves
  the problem without removing the responsibility

## Decision

Remove all skill scaffolding from `git std init`. Skill lifecycle is delegated
to the `skills` CLI (agentskills.io format, `npx skills add`) in the short term
and a future dedicated `upskill` tool long term. Both consume skills from
`skills/` in the repo. `git std` is completely silent on the topic — no
commands, no hints, no documentation until `upskill` is ready.

## What changes

### `skills/` — restructure to agentskills.io format

The `skills` CLI (`npx skills add driftsys/git-std`) expects each skill to be a
subdirectory containing `SKILL.md`:

```text
skills/
  std-commit/
    SKILL.md
  std-bump/
    SKILL.md
```

Current layout uses flat files (`skills/std-commit.md`). Rename:

- `skills/std-commit.md` → `skills/std-commit/SKILL.md`
- `skills/std-bump.md` → `skills/std-bump/SKILL.md`

Optional subdirectories `scripts/` and `references/` may be added per-skill in
the future but are out of scope here.

### `crates/git-std/src/cli/init/scaffold.rs`

Remove:

- `write_skill_source()` — created `.agents/skills/<name>/SKILL.md`
- `write_skill_symlink()` — created `.claude/skills/<name>` symlink
- `skill_definitions()` — returned the list of skills to scaffold
- `#[cfg(unix)] use std::os::unix::fs::symlink` import (only used by the above)
- Module-doc reference to skill files

Keep and update:

- The three skill _content_ unit tests (`std_commit_skill_has_frontmatter`,
  `std_commit_skill_includes_message_guidelines`, `std_bump_skill_has_frontmatter`)
  — these verify the source files are well-formed for the `skills` CLI consumer.
  Update `include_str!` paths from `"../../../../../skills/std-commit.md"` to
  `"../../../../../skills/std-commit/SKILL.md"` (and same for `std-bump`).

### `crates/git-std/src/cli/init/mod.rs`

Remove:

- Constants `AGENTS_SKILL_COMMIT_DIR`, `AGENTS_SKILL_BUMP_DIR`,
  `CLAUDE_SKILL_COMMIT`, `CLAUDE_SKILL_BUMP`
- Imports of `write_skill_source`, `write_skill_symlink`, `skill_definitions`
  from `scaffold`
- Step 8 ("scaffold agent skills") in `init()`
- Skill scaffolding loop in `refresh()`

## What does not change

| Item                                              | Why                                                      |
| ------------------------------------------------- | -------------------------------------------------------- |
| `.agents/skills/`, `.claude/skills/` in this repo | Already committed; left as-is until `upskill` takes over |
| Hook, config, bootstrap scaffolding in `init`     | Unrelated — unchanged                                    |
| `git std init` output and UX                      | Skill lines simply disappear from output                 |

## Compatibility note

`.agents/skills/std-commit/SKILL.md` already matches the agentskills.io layout,
so `npx skills add driftsys/git-std` will naturally deploy skills into the same
directory structure that the repo uses today.

## What closes

- **#488** — the `skill add` command proposal is superseded by this decision.
  Close as won't-implement with a note pointing to the `skills` CLI and
  `upskill`.

## Out of scope

- Removing `.agents/skills/` and `.claude/skills/` from this repo's git history
  (left for when `upskill` is operational)
- Adding `scripts/` or `references/` subdirectories to skills
- Any `upskill` implementation or interface definition
- Documentation updates (deferred until `upskill` is ready)
