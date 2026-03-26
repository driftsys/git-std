# AGENTS.md

Instructions for AI coding agents working in this repository.

## Project

git-std is a single Rust CLI binary that consolidates conventional commits,
version bumping, changelog generation, and git hooks management. It replaces
commitizen, commitlint, standard-version, husky, and lefthook with one
statically-linked tool and zero runtime dependencies.

Invoked as `git std` via git's subcommand discovery (binary name `git-std`).

## Build commands

```bash
cargo test <test_name>  # Run a single test
just assemble           # Compile
just test               # Run all tests
just lint               # Lint + format check
just audit              # Audit dependencies
just check              # Run all checks (test + lint + audit)
just build              # Assemble + check
just verify             # Commitlint + build — run before PR
just fmt                # Format Rust + Markdown
```

## Architecture

The full specification lives in `docs/SPEC.md`.

**Workspace structure — five crates:**

| Crate                | Role                                                              |
| -------------------- | ----------------------------------------------------------------- |
| `git-std`            | CLI binary — orchestrates I/O, git, config, dispatch              |
| `standard-commit`    | Conventional commit parsing, linting, formatting                  |
| `standard-version`   | Version bump (semver + calver), version file detection and update |
| `standard-changelog` | Changelog generation from conventional commits                    |
| `standard-githooks`  | Hook file format parsing, shim generation, enable/disable         |

Library crates are pure — no git2, no I/O, no terminal output — except
`standard-version`, which performs file I/O for version file detection and
updates.

**Seven subcommands**, each a separate concern:

| Subcommand            | Purpose                                                |
| --------------------- | ------------------------------------------------------ |
| `git std commit`      | Interactive conventional commit builder                |
| `git std check`       | Commit message validation                              |
| `git std bump`        | Version bump + changelog + commit + tag                |
| `git std changelog`   | Changelog generation (incremental or full)             |
| `git std bootstrap`   | Post-clone environment setup                           |
| `git std hooks`       | Git hooks management (install/run/list/enable/disable) |
| `git std completions` | Generate shell completion scripts                      |

**Key design decisions:**

- Config is `.git-std.toml`. Hooks config is `.githooks/*.hooks` (plain text,
  one command per line).
- Uses `git` CLI subprocess calls — no C dependency on libgit2.

**Key dependencies:** `clap` (CLI), `inquire` (prompts), `semver` (version
parsing), `toml` (config), `yansi` (colours).

## Workflow

Follow [CONTRIBUTING.md](CONTRIBUTING.md) for issue model, PR process,
severity/effort/priority, and review flow.

**Agent-specific rules:**

- **Start from the issue.** Read the acceptance criteria and `docs/SPEC.md`,
  propose an approach, and wait for approval before implementing.
- **ATDD + TDD.** Write acceptance tests first from the story's acceptance
  criteria, then TDD the unit tests and implementation. Three test layers, each
  with a distinct purpose:
  - `crates/git-std/spec/` — blackbox e2e snapshot tests (acceptance criteria;
    binary input/output, no internals).
  - `crates/git-std/tests/` — integration tests (functional coverage; exercise
    the CLI wiring).
  - `#[cfg(test)]` inline modules — unit tests (code coverage; test library
    logic in isolation).
- **Worktree isolation.** Every feature must work in a git worktree. Do not
  assume the working directory is the repo root; resolve paths via
  `git rev-parse` where needed.
- **Single PR = code + tests + docs.** Every pull request ships implementation,
  tests, and updated documentation together.
- **Commits.** Use Conventional Commits — `feat`, `fix`, `refactor`, `docs`,
  `test`, `chore`. Imperative mood. One commit per PR.
- **Before PR.** Run `just verify` — all must pass.
- **PR review.** After opening a PR, review it and submit findings. Triage each
  finding:
  - **Must fix (`K0`)** — fix immediately before merging.
  - **Should fix (`K1`)** — open a debt issue linking to the PR.
  - **Nice to have (`K2`)** — open a debt issue linking to the PR.
    Debt issues must link to the PR that surfaced the finding and include
    enough context to understand the problem without reading the PR.

**Issue labels and priority:**

Issue types: `story` (user-facing), `task` (technical), `debt` (refactor/review
finding). Every issue body must start with `Epic: #N`. Severity: `K0` must-have,
`K1` should-fix, `K2` nice-to-have. Effort: `XS` `S` `M` `L` `XL`. Priority is
derived from the K × size matrix:

| K↓ Size→ | XS | S  | M  | L    | XL   |
| -------- | -- | -- | -- | ---- | ---- |
| K0       | P0 | P0 | P0 | P1   | P1   |
| K1       | P0 | P1 | P1 | P2   | drop |
| K2       | P1 | P2 | P2 | drop | drop |

P0 = do now · P1 = do next · P2 = do when convenient · drop = close as
won't-fix.

## Module structure

Group modules by domain concept. Technical layers (`cli`, `git`, `ui`) are fine
when they reflect a genuine concern, but keep each module focused and small.

**Rules:**

- **One concept per module.** Name modules after what they do (`parse`, `lint`,
  `shim`, `calver`, `git`, `config`). Never `utils`, `helpers`, or `common`.
- **`lib.rs` is an index.** Re-exports and submodule declarations only — no
  logic, no types.
- **File size.** Soft limit 300 lines, hard limit 500. Near the hard limit means
  a concept is ready to extract.
- **Low coupling.** Modules depend on types, not on each other's internals.
- **Crate boundaries follow the workspace table.** The five-crate split is the
  primary separation of concerns. Only `git-std` imports from multiple library
  crates.
- **Test files follow the same rules.** Split large test files by the concept
  they exercise.

## Conventions

- **Zero warnings.** No warnings anywhere — Rust compiler, `cargo test`,
  `clippy`, Markdown (`dprint` + markdownlint), or IDE. Fix warnings as they
  appear; do not suppress with `#[allow(...)]` unless unavoidable, and document
  the reason.
- **Code style:** `rustfmt` + `clippy`. Always run `just fmt` before committing.
- **Naming.** Names must reveal intent. Avoid `temp`, `data`, `flag`, `info`,
  and noise suffixes (`Manager`, `Handler`, `Helper`). Booleans use
  `is_`/`has_`/`can_`. Prefer newtypes over encoding units in names
  (`Milliseconds` over `elapsed_ms`). No `get_` prefix — use `fn name()` not
  `fn get_name()`. **Rust API guidelines and `clippy` supersede all of the
  above** — when they conflict, follow Rust.
- **Error handling.** Library crates use `thiserror` — typed, matchable error
  enums are part of the public contract. The `git-std` binary uses `anyhow` —
  errors only need to be displayed, never matched; use `.context()` for
  user-friendly messages.
- **UI consistency.** Follow [clig.dev](https://clig.dev) as the reference for
  CLI behaviour. Key rules:
  - Human output → stderr via `ui::` helpers. Never call `eprintln!` directly in
    command handlers.
  - Machine/pipeable output → stdout only.
  - Messages are human-first: describe what was done in plain language
    (e.g. "git hooks configured"), not internal details
    (e.g. "core.hooksPath → .githooks"). Use past tense for completed actions.
    Stay silent when nothing was done — don't report skips.
  - Prompts use `inquire`. Disable prompts when stdin is not a TTY (CI safety) —
    fail fast with a clear error instead of hanging.
  - Exit codes: `0` success, non-zero failure. Be consistent — a command either
    succeeds or it doesn't.
  - Colour via `yansi`, already wired through `--color` and TTY detection. Never
    use ANSI escape codes directly. Respect `NO_COLOR`.
- **Comments:** doc comments on all public API items, brief inline comments on
  tricky internals only.

<!-- git-std:bootstrap -->

## Post-clone setup

Run `./bootstrap` after `git clone` or `git worktree add`.
