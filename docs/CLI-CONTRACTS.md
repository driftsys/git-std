# CLI contracts

`git-std` exposes versioned machine contracts for pinned-binary integrations.
Machine output is exactly one JSON document on stdout. Human explanations and
logs use stderr and can be ignored by a machine consumer.

The CLI schema version is `1.0.0` and is independent of the executable's
`tool_version`. Additive fields and new diagnostic codes may be introduced in a
minor git-std release. Removing or renaming fields, changing their types, or
changing documented exit semantics requires a new major CLI schema and a
changelog callout.

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

`--expect-plan <plan-id>` recomputes the plan before built-in mutation. If any
bound input changed, apply exits 1 with `GITSTD-BUMP-PLAN-DIVERGED`; it does not
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

Release `v0.11.15` is evidence for the legacy command spellings and successful
JSON fields only. The v1 metadata, registry, SARIF, mismatch facts, plan ID, and
`--expect-plan` guard are absent from the published `v0.11.15` binary. Until the
workspace version is bumped for the next release, branch builds and examples
still report the source package version `0.11.15`; that field alone is not
release evidence. Record the first release containing the contracts here after
a real release is published and verified.
