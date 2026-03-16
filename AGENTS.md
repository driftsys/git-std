# AGENTS.md

Instructions for AI coding agents working in this repository.

## Project

git-std is a single Rust CLI binary that consolidates
conventional commits, version bumping, changelog generation,
and git hooks management. It replaces commitizen, commitlint,
standard-version, git-cliff (CLI), husky, and lefthook with
one statically-linked tool and zero runtime dependencies.

Invoked as `git std` via git's subcommand discovery
(binary name `git-std`).

## Build commands

```bash
cargo test <test_name>       # Run a single test
just assemble                # Compile
just test                    # Run all tests
just lint                    # Lint + format check
just audit                   # Audit dependencies
just check                   # Run all checks (test + lint + audit)
just build                   # Assemble + check
just verify                 # Commitlint + build — run before PR
just fmt                     # Format Rust + Markdown
```

## Architecture

The full specification lives in `docs/SPEC.md`.

**Workspace structure — five crates:**

| Crate                | Role                                                                 |
| -------------------- | -------------------------------------------------------------------- |
| `git-std`            | CLI binary — orchestrates I/O, git, config, dispatch                 |
| `standard-commit`    | Conventional commit parsing, linting, formatting                     |
| `standard-version`   | Semantic version bump calculation, version file detection and update |
| `standard-changelog` | Changelog generation from conventional commits                       |
| `standard-githooks`  | Hook file format parsing, shim generation                            |

Library crates are pure — no git2, no I/O, no terminal
output — except `standard-version`, which performs file
I/O for version file detection and updates.

**Four subcommands**, each a separate concern:

| Subcommand       | Purpose                                 |
| ---------------- | --------------------------------------- |
| `git std commit` | Interactive conventional commit builder |
| `git std check`  | Commit message validation               |
| `git std bump`   | Version bump + changelog + commit + tag |
| `git std hooks`  | Git hooks management (install/run/list) |

**Key design decisions:**

- Config is `.git-std.toml`. Hooks config is
  `.githooks/*.hooks` (plain text, one command per line).
- Uses `git2` (libgit2) for git operations — no shelling
  out to `git` (except for GPG signing).
- Target binary size: ~5-8 MB
  (`lto = true`, `strip = true`, `codegen-units = 1`).
- Static linking with `musl` on Linux.

**Key dependencies:** `clap` (CLI), `inquire` (prompts),
`git2` (git ops), `semver` (version parsing), `toml`
(config), `yansi` (colours).

## Workflow

- **Ask first.** Before implementing, propose an approach
  and wait for approval. Read `docs/SPEC.md` to understand
  the intended behavior.
- **Follow the story plan.** Implementation tasks come from
  GitHub issues. Read the issue and its acceptance criteria
  before starting.
- **ATDD + TDD.** Write acceptance tests first from the
  story's acceptance criteria, then TDD the unit tests and
  implementation to make them pass. Acceptance tests go in
  `tests/`, unit tests go inline in `#[cfg(test)]` modules.
- **Single PR = code + tests + docs.** Every pull request
  ships implementation, tests, and updated documentation
  together. Before opening a PR, update rustdoc comments,
  `README.md`, and `docs/` pages (mdbook) to stay consistent
  with the code changes.
- **Single commit per PR.** Each PR should contain one
  conventional commit per story/task. Fixup or squash
  cleanup commits before pushing so the branch has a
  clean history.
- **Merge commit.** PRs are merged with `gh pr merge --merge`
  (not squash). This preserves the PR reference in the
  merge commit for traceability in `git log`.
- **Work in isolation.** Always work in a git worktree
  (not the main working directory). This prevents conflicts
  with other agents running in parallel and protects the
  user's in-progress work.
- **Before PR.** Run `just verify` (commitlint + build) —
  all must pass. Then ask for approval before creating the
  pull request.
- **Review before merge.** After CI passes, review every PR
  before merging. Post findings as a PR review via
  `gh api repos/{owner}/{repo}/pulls/{n}/reviews` with
  `event=COMMENT`. Classify findings by severity (K0/K1/K2).
  Fix K0 items directly in the PR. Open debt issues for
  K1 and K2 items so they are tracked and not lost.

## Issue model

### Hierarchy

```text
Initiative (label only — initiative:git-workflow)
  └── Epic (issue + epic + epic:<name> labels)
        ├── Story  — user-facing requirement
        ├── Task   — technical requirement
        └── Debt   — refactoring / review findings
```

### Issue types

| Type  | Label   | Purpose                                          |
| ----- | ------- | ------------------------------------------------ |
| Epic  | `epic`  | Tracking issue grouping stories/tasks/debt       |
| Story | `story` | User-facing requirement from the spec            |
| Task  | `task`  | Technical requirement (not user-visible)         |
| Debt  | `debt`  | Refactoring, should-fix or nice-to-have findings |
| Bug   | `bug`   | Defect. K0 bugs are must-fix immediately         |

### Severity

| Label | Meaning      |
| ----- | ------------ |
| `K0`  | Must-have    |
| `K1`  | Should-fix   |
| `K2`  | Nice-to-have |

### Effort

| Label | Meaning                          |
| ----- | -------------------------------- |
| `XS`  | Trivial — typo, one-liner        |
| `S`   | Small — single file, < 1 hour    |
| `M`   | Medium — a few files, half a day |
| `L`   | Large — cross-cutting, full day  |
| `XL`  | Extra large — multi-day          |

### Priority lookup

|    | XS | S  | M  | L    | XL   |
| -- | -- | -- | -- | ---- | ---- |
| K0 | P0 | P0 | P0 | P1   | P1   |
| K1 | P0 | P1 | P1 | P2   | drop |
| K2 | P1 | P2 | P2 | drop | drop |

- **P0:** do now. **P1:** do next. **P2:** do when
  convenient. **Drop:** not worth the effort — close
  as won't-fix.
- K0 never drops — must-haves always get done.

### Review findings flow

- **K0** → fix directly in the PR (no issue needed),
  or open a `bug` issue if it requires separate work.
- **K1 / K2** → open a `debt` issue with severity,
  effort, and priority labels.

### Rules for agents

1. Every story/task/debt must have an `Epic:` line as
   the first non-blank line of the body. Use
   `Epic: #<number>` for repo-local epics or
   `Epic: <org>/<repo>#<number>` for org-level epics.
2. Every story/task/debt carries exactly one
   `epic:<name>` label plus its type label (`story`,
   `task`, or `debt`).
3. No domain labels — the domain is implicit from the
   title and epic.
4. When creating a story/task/debt, update the parent
   epic's task list to include the new issue.
5. Epics live at org level (`driftsys/.github`) for
   cross-repo concerns or at repo level for
   repo-specific work (e.g. #96).
6. Epics are created by humans. Agents create stories,
   tasks, and debt issues.

### Templates

**Epic:**

```markdown
## Goal

<1-2 sentences.>

## Spec reference

<Link to docs/SPEC.md section.>

## Stories

- [ ] #N — title

## Tasks

- [ ] #N — title

## Debt

- [ ] #N — title
```

Labels: `epic`, `epic:<name>`, `initiative:git-workflow`

**Story** (user-facing requirement):

```markdown
Epic: #<epic-number>

## Context

<Why this story exists.>

## Acceptance criteria

- <criterion>

## Spec reference

<Link to docs/SPEC.md section.>
```

Labels: `story`, `epic:<name>`, severity, effort

**Task** (technical requirement):

```markdown
Epic: #<epic-number>

## Context

<Why this task exists.>

## Acceptance criteria

- <criterion>
```

Labels: `task`, `epic:<name>`, severity, effort

**Debt** (refactoring / review findings):

```markdown
Epic: #<epic-number>

## Context

<What triggered this. Reference the PR or review.>

## Acceptance criteria

- <criterion>
```

Labels: `debt`, `epic:<name>`, severity, effort

## Conventions

- **Commits:** [Conventional Commits][cc] — imperative mood,
  types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`.
  Keep the subject line concise; body is optional and must
  not exceed 10 lines.
- **Code style:** `rustfmt` + `clippy` with zero warnings.
  `dprint` for Markdown formatting. Always run `just fmt`
  before committing to ensure Rust and Markdown files are
  properly formatted. Target zero IDE warnings — add
  project-specific words to `.vscode/settings.json` under
  `cSpell.words` to fix spellcheck warnings.
- **Branches:** descriptive kebab-case from `main`,
  optionally prefixed with issue number
  (e.g., `42-fix-tag-detection`).
- **Comments:** doc comments on all public API items, brief
  comments on tricky internals. Skip comments where the code
  is self-explanatory.
- **Versioning:** [Semantic Versioning][semver]. Releases are
  cut with a `chore(release): <version>` commit and an
  annotated `v<version>` tag.

[cc]: https://www.conventionalcommits.org/
[semver]: https://semver.org/
