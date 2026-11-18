# Folio integration qualification

Verified on 2026-11-18 against published release [v0.12.1][release]. This record
covers issues #561, #554, #555, and #556. Folio continues to own binary pinning,
subprocess invocation, policy, and comparison of plans with observed results.

[release]: https://github.com/driftsys/git-std/releases/tag/v0.12.1

## Release identity and execution scope

- Release commit: `eba9b012652e0835bc451b9a17897e5947f60cb6`.
- Annotated tag object: `acf92edfa57b99ad9d8959fcd5338be6d58ddba3`.
- Hosted release run: [43771684971][run], all five builds and five smoke jobs passed.
- Native contract execution: downloaded `aarch64-apple-darwin` archive on macOS.
  Its binary reports `git-std 0.12.1`.
- Cross-platform evidence: all five downloaded archives passed hosted integrity
  verification. This does not claim the full contract suite ran on every platform.

[run]: https://github.com/driftsys/git-std/actions/runs/43771684971

## CLI and registry contracts

The following acceptance targets passed against the downloaded native binary:

| Target                           | Tests | Evidence                                                                                            |
| -------------------------------- | ----- | --------------------------------------------------------------------------------------------------- |
| `spec/tests/bump_clean_state.rs` | 8     | Staged, unstaged, untracked and submodule admission; read-only dirty preview                        |
| `spec/tests/contracts.rs`        | 10    | JSON metadata, errors, exit classes, version and doctor output                                      |
| `spec/tests/folio_contract.rs`   | 17    | Polyglot canonical version, mismatches, guarded apply, effects and hook limits                      |
| `spec/tests/folio_fidelity.rs`   | 8     | Exact Cargo rewrites, conditional changelog, index and release-path fidelity                        |
| `spec/tests/folio_inputs.rs`     | 3     | Complete normalized inputs, changed tool/config inputs, independent option identity                 |
| `spec/tests/registry.rs`         | 10    | Effective conventions, rule/code consistency and JSON/SARIF behavior                                |
| `spec/tests/folio_push.rs`       | 1     | Guarded apply result compared with actual branch and annotated-tag OIDs in a disposable bare remote |

The push fixture uses a disposable local bare remote. It verifies unchanged-plan
admission, dry-run nonmutation, the selected remote, and exact remote branch/tag
OIDs after guarded apply. Network and concurrent remote changes remain conditional.

The five tests in `crates/git-std/tests/schema_contracts.rs` also passed. They
validate the versioned schemas and examples, real success/failure documents, and
rejection of malformed bump inputs and effects. Static schema rejection tests
exercise repository schemas; they are not tests of binary parsing.

| Issue criterion                                      | Satisfying release and evidence                                                                                                                                       |
| ---------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| #554: command inventory, schemas and examples        | v0.12.1; [CLI contracts](CLI-CONTRACTS.md) and `schemas/v1/cli/`                                                                                                      |
| #554: compatibility, codes, stdout and exits         | v0.12.1; contracts and registry targets; compatibility policy in CLI contracts                                                                                        |
| #554: single canonical polyglot version              | v0.12.1; folio_contract validates one version, tag, changelog and inherited Cargo parity, with explicit mismatch facts                                                |
| #554: unchanged-input plan/apply fidelity            | v0.12.1; folio_contract and folio_fidelity compare file hashes and local commit/tag effects; folio_push compares the planned remote, result and actual published refs |
| #554: config, hooks, tools, time and external inputs | v0.12.1; folio_inputs and hook skip/divergence cases; conditional and opaque limits are explicit                                                                      |
| #554: changed/skipped/failure cases                  | v0.12.1; the acceptance targets above and schema rejection controls                                                                                                   |
| #555: registry facts and resolved convention         | v0.12.1; registry tests distinguish static rules from project settings                                                                                                |
| #555: version/schema compatibility                   | v0.12.1, schema 1.0.0; registry schema and CLI compatibility policy                                                                                                   |
| #555: JSON/SARIF codes and exits                     | v0.12.1; registry and contracts tests, including reversed/ordinary empty ranges                                                                                       |
| #555: common rule definitions                        | v0.12.1; `standard-commit` rules feed lint and registry; emitted codes are checked against descriptors and registry                                                   |
| #555: parseable success/failure stdout               | v0.12.1; registry, contracts and real schema success/failure tests                                                                                                    |

Single-version fidelity requires `monorepo = false`. Arbitrary hooks remain
opaque; lock tools and remote operations remain conditional. Stable-branch JSON
is explicitly refused. These are documented boundaries, not exact guarantees.

## Remaining fidelity debt audit (#561)

| Criterion                                  | Existing fix and regression evidence                                                                                 |
| ------------------------------------------ | -------------------------------------------------------------------------------------------------------------------- |
| Reject pre-existing staged entries         | v0.12.1 clean-state admission; `release_rejects_changes_staged_after_preview` and clean-state tests                  |
| Share Cargo preview/apply transformation   | v0.12.1 `preview_workspace_manifest`; exact-hash and dependency-only/pinned-member fixtures in folio_fidelity        |
| Omit unchanged changelog effects           | v0.12.1 conditional `planned_changelog`; `release_without_changelog_content_declares_no_changelog_effect`            |
| Resolve Windows executable suffix/PATHEXT  | v0.12.1 `contract/tool_resolution.rs`; native executable and CMD-only lookup tests; `Windows tool resolution` CI job |
| Changed-input and exact-effect regressions | v0.12.1 passes the three fidelity/input/clean-state targets above (19 tests)                                         |

The Windows job runs resolver unit tests on a Windows filesystem. It qualifies
lookup semantics, not execution of every ecosystem tool or the complete Windows
CLI contract. The extra CMD-only and executable-precedence tests and Windows job
are qualification additions; the resolver implementation is unchanged.

Related debts #558 (SARIF descriptors), #559 (release-verifier controls), and
issue #560 (schema/input coverage) already have regression coverage in registry,
release shell fixtures, schema_contracts, folio_contract, and folio_inputs.
Their original missing tests are present; release-ref binding was a separate gap
found during this qualification.

## Published asset verification (#556)

All targets have an archive, checksum, Sigstore signature, SPDX SBOM and
provenance bundle. Each downloaded set passed `scripts/release/verify.sh` with
`--hosted`, using Cosign v3.1.4 and GitHub CLI. Verification binds the exact archive
bytes, the release workflow identity at `refs/tags/v0.12.1`, and provenance source
ref. The release run separately executed all five platform smoke jobs.

| Target                       | Archive SHA-256                                                    |
| ---------------------------- | ------------------------------------------------------------------ |
| `aarch64-apple-darwin`       | `34b055bfeedb579aea7c386780816abc37a55a88a97c9bc0811abb9637038461` |
| `aarch64-unknown-linux-musl` | `f8e5411d4b96ec2f3118ed108c55fea947025618c9757ca814b845fcf59dee53` |
| `x86_64-apple-darwin`        | `abcdeecf27191cd3be8ff9fe3ebe212dcbace7bbd8ade113fa8ffc1c32e3c08e` |
| `x86_64-pc-windows-msvc`     | `9397cbd359dc15b62b075fd7579d09c04baa5f4cdafbcb7947f359cc5bbe932e` |
| `x86_64-unknown-linux-musl`  | `ea71d47dbc852e4e0e7b5184698a75b66db2710d25dfc101ea742515c0180703` |

The source helper at tag v0.12.1 accepted any ref from the release workflow.
Changing only an unsigned SBOM version label made it accept the genuine archive
as v9.9.9. The strengthened helper rejects this actual-artifact negative control
and accepts all original published sets. Real verifiers also rejected a
one-byte alteration of the signature in each signature/provenance bundle.
Use the current helper with these assets; the old helper is not the
qualification policy.

`spec/release/release_test.sh` also checks tampered archive/SBOM rejection,
controlled signature/provenance verifier failures, exact workflow arguments,
wrong release identity, and explicit branch qualification. Controlled verifiers
prove policy wiring and failure propagation, not cryptographic correctness; the
actual published-artifact checks above exercise real verifiers.

See [release integrity](RELEASE-INTEGRITY.md) for archive layout, checksum
discovery, verification commands, provenance level, and manual branch checks.
Folio downloads these pinned assets directly; `install.sh` and `cargo install`
remain separate human convenience flows.
