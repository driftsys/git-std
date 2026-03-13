# AGENTS.md

Instructions for AI coding agents working in this repository.

## Project

git-std is a single Rust CLI binary that consolidates conventional commits, version bumping, changelog generation, and git hooks management. It replaces commitizen, commitlint, standard-version, git-cliff (CLI), husky, and lefthook with one statically-linked tool and zero runtime dependencies.

Invoked as `git std` via git's subcommand discovery (binary name `git-std`).

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

The project is in early scaffold stage (v0.0.0). The full specification lives in `docs/SPEC.md`.

**Four subcommands**, each a separate concern:

| Subcommand       | Purpose                                 |
| ---------------- | --------------------------------------- |
| `git std commit` | Interactive conventional commit builder |
| `git std check`  | Commit message validation               |
| `git std bump`   | Version bump + changelog + commit + tag |
| `git std hooks`  | Git hooks management (install/run/list) |

**Key design decisions:**

- Config is `.versionrc` (TOML). Hooks config is `.githooks/*.hooks` (plain text, one command per line).
- Uses `git2` (libgit2) for git operations — no shelling out to `git`.
- Uses `git_cliff_core` as a library for changelog rendering.
- Target binary size: ~5–8 MB (`lto = true`, `strip = true`, `codegen-units = 1`).
- Static linking with `musl` on Linux.

## Workflow

- **Ask first.** Before implementing, propose an approach and wait for approval. Read `docs/SPEC.md` to understand the intended behavior.
- **Follow the story plan.** Implementation tasks come from GitHub issues. Read the issue and its acceptance criteria before starting.
- **ATDD + TDD.** Write acceptance tests first from the story's acceptance criteria, then TDD the unit tests and implementation to make them pass. Acceptance tests go in `tests/`, unit tests go inline in `#[cfg(test)]` modules.
- **Single PR = code + tests + docs.** Every pull request ships implementation, tests, and updated documentation together. Before opening a PR, update rustdoc comments, `README.md`, and `docs/` pages (mdbook) to stay consistent with the code changes.
- **Single commit.** Squash work into one conventional commit per task.
- **Before PR.** Run `just verify` (commitlint + build) — all must pass. Then ask for approval before creating the pull request.

## Conventions

- **Commits:** [Conventional Commits](https://www.conventionalcommits.org/) — imperative mood, types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`. Keep the subject line concise; body is optional and must not exceed 10 lines.
- **Code style:** `rustfmt` + `clippy` with zero warnings. `dprint` for Markdown formatting. Always run `just fmt` before committing to ensure Rust and Markdown files are properly formatted. Target zero IDE warnings — add project-specific words to `.vscode/settings.json` under `cSpell.words` to fix spellcheck warnings.
- **Branches:** descriptive kebab-case from `main`, optionally prefixed with issue number (e.g., `42-fix-tag-detection`).
- **Comments:** doc comments on all public API items, brief comments on tricky internals. Skip comments where the code is self-explanatory.
- **Versioning:** [Semantic Versioning](https://semver.org/). Releases are cut with a `chore(release): <version>` commit and an annotated `v<version>` tag.
