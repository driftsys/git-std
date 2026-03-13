# Contributing to git-std

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
CLI dispatch, config loading, git operations (via `git2`), and terminal I/O.

Read [docs/SPEC.md](docs/SPEC.md) for the full specification.

## Testing

```bash
just test               # Run all tests
cargo test <test_name>  # Run a specific test
just check              # Tests + lint + audit
just verify             # Full pre-PR gate (commit lint + build)
```

### Test conventions

- **Acceptance tests** go in `crates/git-std/tests/` — these test the CLI
  binary end-to-end.
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

## Documentation style

Write documentation in Markdown following the [Google documentation style
guide]:

- Use second person ("you") and active voice.
- Use sentence case for headings.
- Use numbered lists for sequences, bulleted lists for everything else.
- Put code-related text in code blocks.

[Google documentation style guide]: https://developers.google.com/style

## Commit messages

Follow the [Conventional Commits](https://www.conventionalcommits.org/)
specification. Write the description in imperative mood — e.g., "add support
for X" instead of "added support for X".

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `ci`, `build`,
`perf`, `style`.

Squash work into one conventional commit per task. Body is optional and must not
exceed 10 lines.

## Branches and pull requests

Create a feature branch from `main` with a short, descriptive name — e.g.,
`add-changelog-parser` or `fix-tag-detection`. Reference the issue number when
applicable: `42-fix-tag-detection`.

Pull request guidelines:

1. Create a branch from `main`.
2. Implement the feature or fix with tests.
3. Run `just verify` — all checks must pass.
4. Open a PR with a clear description referencing the issue.
5. Every PR ships implementation, tests, and updated documentation together.

## Code review

Reviews should check for:

- **Functionality** — does the code behave as intended?
- **Complexity** — could it be simpler?
- **Naming** — are names clear and consistent?
- **Tests** — are there correct, well-designed tests?
- **Documentation** — is relevant documentation updated?

Be respectful — assume competence and goodwill, explain your reasoning, and
mention positives.
