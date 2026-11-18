# CLI contracts

`git-std` exposes versioned machine contracts for pinned-binary integrations.
Machine output is exactly one JSON document on stdout. Human explanations and
logs use stderr and can be ignored by a machine consumer.

The CLI schema version is `1.0.0` and is independent of the executable's
`tool_version`. Additive fields and new diagnostic codes may be introduced in a
minor git-std release. Removing or renaming fields, changing their types, or
changing documented exit semantics requires a new major CLI schema and a
changelog callout.

`git std lfs install` is a human-oriented setup command. It reports progress
and failures on stderr and has no JSON output contract. The resulting LFS entry
is an ordinary declarative pre-push command and appears in the existing
`hook list` and `hook run` contracts.

## Inventory

| Command                                 | Contract                               | Example                                                                     |
| --------------------------------------- | -------------------------------------- | --------------------------------------------------------------------------- |
| `git std version --format json`         | `schemas/v1/cli/version.schema.json`   | `schemas/v1/cli/examples/version.json`                                      |
| `git std bump --dry-run --format json`  | `schemas/v1/cli/bump.schema.json`      | `schemas/v1/cli/examples/bump.json`                                         |
| `git std lint --format json`            | `schemas/v1/cli/lint.schema.json`      | `schemas/v1/cli/examples/lint.json`                                         |
| `git std lint --format sarif`           | SARIF 2.1.0                            | Registered codes are `ruleId` values with matching driver rule descriptors. |
| `git std hook list --format json`       | `schemas/v1/cli/hook-list.schema.json` | `schemas/v1/cli/examples/hook-list.json`                                    |
| `git std hook run <hook> --format json` | `schemas/v1/cli/hook-run.schema.json`  | `schemas/v1/cli/examples/hook-run.json`                                     |
| `git std doctor --format json`          | `schemas/v1/cli/doctor.schema.json`    | `schemas/v1/cli/examples/doctor.json`                                       |
| `git std registry --format json`        | `schemas/v1/cli/registry.schema.json`  | `schemas/v1/cli/examples/registry.json`                                     |
| `git std tag <commit> --format json`    | `schemas/v1/cli/tag.schema.json`       | `schemas/v1/cli/examples/tag.json`                                          |

Object responses contain `schema_version`, `tool_version`, and `status`.
Compatibility preserves the existing array shape of range lint and hook-list
output; each nonempty array entry carries its own contract metadata. An empty
range or empty hook registry remains `[]`.

## Exit classes and diagnostics

- `exit 0`: the command completed successfully and lint found no violations.
- `exit 1`: a rule finding, health-check failure, or failed plan precondition.
- `exit 2`: usage, input/output, repository, or other operational failure.

JSON failures contain an `exit_code` matching the process exit status and a
`diagnostics` array with stable `code`, `severity`, and `message` fields.
SARIF operational failures set `executionSuccessful = false`. For range lint,
JSON preserves its array contract and returns `[]` for an empty reversed range;
SARIF emits the `GITSTD-LINT-EMPTY-RANGE` finding instead. An ordinary empty
SARIF range is a successful empty result document.
`git std registry --format json` is the source
of truth for rule explanations, diagnostic codes, and the convention resolved
from the current project. The same `standard-commit` rule definitions produce
registry entries and lint codes.

## Bump plans

Single-version mode (`monorepo = false`) treats the version selected from the
tag lineage as canonical. `version_observations` reports readable manifests,
including inherited and pinned Cargo workspace members.
`version_mismatches` makes disagreement explicit; it does not silently choose a
different release per manifest.

A dry-run returns a `plan_id` over canonical serialized inputs and declared
effects. Inputs cover normalized options, HEAD, branch, tags, remotes, effective
date, effective and raw configuration, planned version and lock files,
lifecycle-hook definitions, `GIT_STD_SKIP_HOOKS`, PATH, and resolved ecosystem
tools. Repeating the same dry-run with unchanged inputs produces the same ID.

To bind apply to that observation:

```bash
plan_id="$(git std bump --dry-run --format json | jq -r .plan_id)"
git std bump --expect-plan "$plan_id" --format json --yes
```

Mutating bumps first require a clean index and working tree as reported by Git.
Staged, unstaged, nonignored untracked files and dirty submodules cause exit 1 with
`GITSTD-BUMP-DIRTY`, including edits to bound manifests, configuration or hooks.
This admission check precedes plan comparison; dry-run remains available on
a dirty working tree.

After clean admission, `--expect-plan <plan-id>` recomputes the plan before
built-in mutation. If any bound input changed, apply exits 1 with
`GITSTD-BUMP-PLAN-DIVERGED`; it does not
update version files, changelog, commit, tag, or push. The plan is not stored on
a server: the ID is a content-derived precondition. It conflicts with
`--dry-run` because it is meaningful only for apply.

Effects declare one of four fidelity levels:

- `exact`: git-std predicts bytes and includes before/after SHA-256 when known.
- `runtime`: the effect is deterministic, but its identifier is observed after
  apply; results include commit and annotated-tag OIDs.
- `conditional`: external tools, locks, network, or remote state determine the
  result.
- `opaque`: an arbitrary lifecycle hook may perform undeclared external effects.

`fidelity.exact` is false whenever a conditional or opaque effect exists, and
`limitations` explains why. A guarded apply prevents stale built-in changes;
it cannot make arbitrary hook or remote effects transactional. Final comparison
of declared and observed effects belongs to the calling orchestrator.
When `GIT_STD_SKIP_HOOKS` is enabled, that state is bound into `inputs.options`
and hook effects are omitted from the plan.

Monorepo JSON remains available with v1 metadata but does not claim this
single-version fidelity model and does not support `--expect-plan`. Consumers
requiring one canonical project version and guarded apply must use
`monorepo = false`. Stable-branch mode does not expose versioned JSON; it
rejects `--format json` instead of silently claiming incomplete fidelity.

## Released-version evidence

The published `v0.12.1` Apple Silicon binary was qualified on 2026-11-18 against
56 acceptance tests covering clean-state admission, plan/apply fidelity, bound
inputs, JSON failures, registry consistency, and SARIF. Five schema tests also
passed, including real success/failure documents and malformed-input rejection.
The executable reports `tool_version = "0.12.1"`; the CLI schema remains `1.0.0`.

See [Folio qualification](FOLIO-QUALIFICATION.md) for the criterion-to-test mapping,
platform limits, release identifiers, and the separately verified asset set.
`v0.11.15` remains a legacy JSON baseline; it does not provide these v1 contracts.

## Tag results

`tag` reports the pinned `target_oid`, `release_branch`, selected remote,
`dry_run`, and each tag's name, version, object ID, planned action, local state,
remote state, signature state, and version source. Dry-run performs inspection
without creating refs. A successful retry can report existing tags without
creating new objects. See [release workflows](releases.md) for recovery.

Exit 1 refuses an unsafe or inconsistent release. Exit 2 reports operational
failure, including partial local creation or uncertain publication. Inspect
per-tag states and repeat the same explicit commit after resolving the cause;
do not infer that exit 2 means the remote received nothing.

Per-tag `local` states are `missing`, `existing`, `adopted`, `created` and
`conflict`. Remote states are `not_checked`, `missing`, `existing`, `pushed`,
`conflict` and `unknown`. `action` is `create`, `adopt` or `none`; it describes
the plan separately from observed state. `version_basis` is `manifest` or
`release_subject`. An early failure may leave the target or branch unresolved
and the tags array empty.

Signature results currently emitted are `unsigned`, `verified` and `unverified`.
`verified` requires a successful `git verify-tag`; any unsuccessful verification
is `unverified`, with Git's explanation retained in a diagnostic. The schema also
allows `invalid`, but the command does not currently distinguish cryptographic
failure from unavailable verification or trust configuration. A signed tag may
therefore satisfy `--sign` without establishing signer trust.

Runtime refusals use `GITSTD-TAG-REFUSED`; operational errors use
`GITSTD-GIT-OPERATION`. Warnings, including configuration and signature-trust
notes, use `GITSTD-TAG-NOTE`. Human output uses stderr and JSON uses stdout.
A no-op text success is silent; JSON still reports the satisfied plan.
