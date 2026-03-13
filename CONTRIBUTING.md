# Contributing guidelines

## Reporting issues

Open bugs and feature requests at
<https://github.com/driftsys/git-std/issues>.

## Prerequisites

- **Rust**: stable toolchain (install via [rustup](https://rustup.rs))
- **[just]**: command runner
- **[dprint]**: Markdown formatter
- **[cargo-audit]**: dependency auditor

```bash
just build    # Compile + run all checks
just test     # Run tests
just lint     # Lint + format check
just fmt      # Format Rust + Markdown
just verify   # Full pre-PR gate (commitlint + build)
```

[just]: https://github.com/casey/just
[dprint]: https://dprint.dev
[cargo-audit]: https://github.com/rustsec/rustsec

## Coding style

Rust code must pass `cargo fmt` and `cargo clippy` with no warnings. Markdown
files must pass `dprint check`. Run `just fmt` to auto-format everything.

### Meaningful names

Consistency across the codebase is the law. Follow the "Clean Code" naming
rules:

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

## Branches and pull requests

Create a feature branch from `main` with a short, descriptive name — e.g.,
`add-changelog-parser` or `fix-tag-detection`. Reference the issue number when
applicable: `42-fix-tag-detection`.

Pull request guidelines:

- Prefer a single commit per pull request addressing one concern.
- Include a brief description of the changes and a reference to the related
  issue.
- Code must be reviewed and approved before merge.

## Code review

Reviews should check for:

- **Functionality** — does the code behave as intended?
- **Complexity** — could it be simpler?
- **Naming** — are names clear and consistent?
- **Tests** — are there correct, well-designed tests?
- **Documentation** — is relevant documentation updated?

Be respectful — assume competence and goodwill, explain your reasoning, and
mention positives.
