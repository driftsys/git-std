# AGENTS.md

Instructions for AI coding agents working in this repository.

## Project

git-std is a single Rust CLI binary that consolidates
conventional commits, version bumping, changelog generation,
and git hooks management. It replaces commitizen, commitlint,
standard-version, husky, and lefthook with one
statically-linked tool and zero runtime dependencies.

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

| Crate                | Role                                                              |
| -------------------- | ----------------------------------------------------------------- |
| `git-std`            | CLI binary — orchestrates I/O, git, config, dispatch              |
| `standard-commit`    | Conventional commit parsing, linting, formatting                  |
| `standard-version`   | Version bump (semver + calver), version file detection and update |
| `standard-changelog` | Changelog generation from conventional commits                    |
| `standard-githooks`  | Hook file format parsing, shim generation                         |

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
- Uses `git` CLI subprocess calls for all git operations
  (no C dependency on libgit2).
- Target binary size: ~5-8 MB
  (`lto = true`, `strip = true`, `codegen-units = 1`).
- Static linking with `musl` on Linux.

**Key dependencies:** `clap` (CLI), `inquire` (prompts),
`semver` (version parsing), `toml`
(config), `yansi` (colours).

## Workflow

Follow [CONTRIBUTING.md](CONTRIBUTING.md) for issue model,
PR process, severity/effort/priority, and review flow.

**Agent-specific rules:**

- **Ask first.** Before implementing, propose an approach
  and wait for approval. Read `docs/SPEC.md` to understand
  the intended behavior.
- **Follow the story plan.** Read the GitHub issue and its
  acceptance criteria before starting.
- **ATDD + TDD.** Write acceptance tests first from the
  story's acceptance criteria, then TDD the unit tests and
  implementation. Acceptance tests go in `tests/`, unit
  tests go inline in `#[cfg(test)]` modules.
- **Single PR = code + tests + docs.** Every pull request
  ships implementation, tests, and updated documentation
  together.
- **Before PR.** Run `just verify` — all must pass.

## Conventions

- **Code style:** `rustfmt` + `clippy` with zero warnings.
  `dprint` for Markdown. Always run `just fmt` before
  committing. Target zero IDE warnings — add words to
  `.vscode/settings.json` under `cSpell.words` for
  spellcheck.
- **Comments:** doc comments on all public API items, brief
  comments on tricky internals. Skip comments where the
  code is self-explanatory.
