# API Reference

Machine consumers should use the [versioned CLI contracts](CLI-CONTRACTS.md).
Release consumers should follow the [release-integrity checks](RELEASE-INTEGRITY.md).

`git-std` is built on five crates. The four library crates
implement domain logic only — no CLI, no git operations, no
terminal output.

| Crate              | Description                                    | docs.rs                                                                                     |
| ------------------ | ---------------------------------------------- | ------------------------------------------------------------------------------------------- |
| git-std            | CLI binary — orchestrates I/O, git, config     | [docs.rs/git-std](https://docs.rs/git-std/latest/git_std/)                                  |
| standard-commit    | Conventional commit parsing, linting           | [docs.rs/standard-commit](https://docs.rs/standard-commit/latest/standard_commit/)          |
| standard-version   | Version bump (semver + calver), file detection | [docs.rs/standard-version](https://docs.rs/standard-version/latest/standard_version/)       |
| standard-changelog | Changelog generation from conventional commits | [docs.rs/standard-changelog](https://docs.rs/standard-changelog/latest/standard_changelog/) |
| standard-githooks  | Hook file format parsing, shim generation      | [docs.rs/standard-githooks](https://docs.rs/standard-githooks/latest/standard_githooks/)    |

## `git std tag`

Tag an existing release commit without another bump. The explicit `<commit>`
argument identifies a release reachable from the configured release branch.

| Option                  | Purpose                                       |
| ----------------------- | --------------------------------------------- |
| `--dry-run`             | Validate the exact plan without creating refs |
| `--branch <name>`       | Override the release branch                   |
| `--package <name>`      | Select a released package; repeat for several |
| `--sign`                | Create signed annotated tags                  |
| `--push <remote>`       | Publish only the planned tags atomically      |
| `--format <text\|json>` | Select human or versioned machine output      |

The command reads configuration and versions from the target commit and leaves
HEAD, the index and working files unchanged. It reuses matching annotated tags,
refuses conflicts, and reports partial or uncertain outcomes without rollback.
With `--push`, it verifies remote branch reachability and exact remote tag objects.

See [release workflows](releases.md) for preflight requirements, both release
processes, retry behavior and limitations. See [CLI contracts](CLI-CONTRACTS.md)
for structured results and exit semantics.

### Existing-release tagging design

The command's orchestration lives in
[`cli/tag/mod.rs`](../crates/git-std/src/cli/tag/mod.rs). It builds one plan,
preflights local tags and, when requested, the remote, then creates or adopts
missing refs and publishes. Dry-run stops after preflight. Output is produced
from the accumulated plan even on runtime failure, so callers can inspect
partial completion.

[`plan.rs`](../crates/git-std/src/cli/tag/plan.rs) pins the commit and branch,
selects release entries and derives tag names.
[`identity.rs`](../crates/git-std/src/cli/tag/identity.rs) parses release subjects
without Git access. [`snapshot.rs`](../crates/git-std/src/cli/tag/snapshot.rs)
materializes selected blobs in a `tempfile` directory rather than checking out
the target or registering a worktree.
[`configuration.rs`](../crates/git-std/src/cli/tag/configuration.rs) validates
configuration types before the tolerant shared loader is used.
[`manifests.rs`](../crates/git-std/src/cli/tag/manifests.rs) checks structural
TOML/JSON version fields, and
[`versions.rs`](../crates/git-std/src/cli/tag/versions.rs) reuses standard-version
readers and validates custom captures and Cargo inheritance. This keeps Git and
terminal operations in the binary crate.

[`tags.rs`](../crates/git-std/src/cli/tag/tags.rs) validates annotations and
signatures, preserves satisfied objects and uses create-only ref operations.
[`remote.rs`](../crates/git-std/src/cli/tag/remote.rs) resolves one endpoint,
observes branch ancestry, adopts remote objects, pushes pinned object IDs and
checks publication. Git subprocess reads use
[`git/release_refs.rs`](../crates/git-std/src/git/release_refs.rs), which disables
lazy fetching and replacement-object interpretation. Publishing the validated
object ID protects the planned target when a local tag ref moves during a
pre-push hook.

A dedicated `tag` command keeps existing-release validation separate from bump's
file updates and commit creation. The considered alternatives were a
`bump --tag-only` mode, whose name would suggest a bump, and manual Git tagging,
which leaves target validation and recovery to each caller. Agent guidance stays
in `std-bump` so release preparation and finalization share one routing point.
The combined bump/tag process remains preferred where branch policy permits it;
post-merge tagging supports explicit user preference and required release PRs.

The binary acceptance cases are in [`spec/tests/tag.rs`](../spec/tests/tag.rs).
The focused `tag_*` integration tests under
[`crates/git-std/tests/`](../crates/git-std/tests/) cover historical snapshots,
remote-only adoption, partial completion, rejected atomic pushes and concurrent
ref changes. Agent scenario fixtures and judges live in
[`tests/skills/std-bump/`](../tests/skills/std-bump/), outside the installed skill:
ancillary files within a skill directory are copied by upskill. Scenario harness
checks and binary tests do not establish cross-client skill qualification;
qualification requires the separate client transcripts and final-state judgments.
