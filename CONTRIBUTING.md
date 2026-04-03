# Contributing to git-std

For org-wide guidelines — AI policy, commit messages, pull request workflow,
code review, issue model, and documentation style — see the
[driftsys contributing guide][org-contributing] and [process][org-process].

This file covers what is specific to the git-std repository.

[org-contributing]: https://github.com/driftsys/.github/blob/main/CONTRIBUTING.md
[org-process]: https://github.com/driftsys/.github/blob/main/PROCESS.md

## Reporting issues

Open bugs and feature requests at
<https://github.com/driftsys/git-std/issues>.

## Dev setup

You need the Rust toolchain and a few extra tools:

- **Rust**: stable toolchain (install via [rustup](https://rustup.rs))
- **[just]**: command runner
- **[dprint]**: Markdown formatter
- **[cargo-audit]**: dependency auditor

```bash
# Clone and build
git clone https://github.com/driftsys/git-std.git
cd git-std
./bootstrap          # post-clone setup (optional, requires git-std)
just build
```

[just]: https://github.com/casey/just
[dprint]: https://dprint.dev
[cargo-audit]: https://github.com/rustsec/rustsec

## Architecture

The project is a Cargo workspace with one binary crate and four library crates:

```text
git-std/
├── crates/
│   ├── git-std/              # CLI binary — arg parsing, I/O, config loading
│   ├── standard-commit/      # Conventional commit parsing, linting, formatting
│   ├── standard-version/     # Semver and calver bump calculation
│   ├── standard-changelog/   # Changelog generation from parsed commits
│   └── standard-githooks/    # Hook file format, shim generation, execution model
├── docs/
│   └── SPEC.md               # Full specification
└── .githooks/                # Hook definitions
```

**Design principle:** library crates are pure domain logic — strings in, data
out. They have no dependency on git, the filesystem, or the terminal. The
`git-std` binary crate is the orchestrator that wires libraries together with
CLI dispatch, config loading, git operations (via git CLI subprocess
calls), and terminal I/O.

Read [docs/SPEC.md](docs/SPEC.md) for the full specification.

## Testing

```bash
just test               # Run all tests
cargo test <test_name>  # Run a specific test
just check              # Tests + lint + audit
just verify             # Full pre-PR gate (commit lint + build)
```

### Test conventions

- **Acceptance tests** go in `crates/git-std/spec/` — blackbox e2e snapshot
  tests driven by the story's acceptance criteria (binary input/output only).
- **Integration tests** go in `crates/git-std/tests/` — functional coverage
  of the CLI wiring.
- **Unit tests** go inline in `#[cfg(test)]` modules alongside the code they
  test.
- Follow ATDD + TDD: write acceptance tests from the story's acceptance
  criteria first, then TDD the implementation.

## Code style

```bash
just fmt    # Format Rust + Markdown
just lint   # Lint + format check
```

- Rust code must pass `cargo fmt` and `cargo clippy` with no warnings.
- Markdown files must pass `dprint check`.
- Always run `just fmt` before committing.

### Meaningful names

Follow the "Clean Code" naming rules:

- Use intention-revealing names.
- Avoid disinformation — don't call something a `list` if it isn't one.
- Make meaningful distinctions.
- Use pronounceable, searchable names.
- Avoid encodings and prefixes.
- Class/struct names should be nouns; function names should be verbs.
- Pick one word per concept and stick with it.
