# Release skill qualification — 2026-11-15

The revised skill passed all 13 scenarios and six critical repeats on both
clients, with independent review of actual tool calls and final reports.
These are cooperative behavioral trials against disposable local repositories
and bare remotes, not hosted release or registry-publication evidence.

| Client                                      | No skill                   | Original skill             | Revised skill | Critical repeats |
| ------------------------------------------- | -------------------------- | -------------------------- | ------------- | ---------------- |
| Claude CLI 2.1.308, `claude-fable-5-2`      | 10 pass, 2 fail, 1 blocked | 9 pass, 3 fail, 1 blocked  | 13 pass       | 6 pass           |
| Codex CLI 0.169.0, configured `gpt-6-astra` | 10 pass, 3 fail            | 10 pass, 2 fail, 1 blocked | 13 pass       | 6 pass           |

Claude response metadata confirms the canonical Fable model. Codex records its
configured model. The comparison runs used one pinned candidate executable;
`evidence.json` records its SHA-256, skill hashes, CLI versions, outcomes and
transcript digests. Raw inputs, transcripts, fixture state and blocked attempts
are retained under the worktree's ignored
`.superpowers/sdd/2026-11-15-agent-release-workflows/evals/` directory. Each
`evidence` value names a directory there. Seven Codex revised trials were
replaced after routing failures or timeouts; failed setup is never a pass.

## Observed improvements

- Baselines published unrelated tags or used broad, non-atomic tag pushes.
  Revised direct releases used explicit selected refs and atomic publication.
- Baselines proceeded from dirty or divergent caller checkouts. An initial
  revised trial also worked around a dirty checkout with a clean clone. The
  final instruction explicitly stops that request; the final trial passed.
- Revised preparation trials pushed a checked release branch without tagging
  it. Historical and recovery trials tagged the supplied merged revision
  without another bump, even after main advanced.
- Existing approval was reused; unapproved major promotion and unavailable
  commands left release state unchanged. Unrelated requests did not load the
  release skill.

## Evidence boundaries

The automated judge checks repository state and a receive-hook record of
validation before publication. Additional checks confirm annotated tags,
released versions, preserved caller changes and unchanged remote refs when
required. Three mutation controls prove that late validation, altered dirty
content, and an empty preparation branch are rejected. Independent transcript
review additionally checks activation, exact atomic pushes and truthful reports.

Claude's failed-validation trials refused safely on the unchanged base rather
than generating a failing release. The Codex failed-validation repeat generated
a local release, failed checks on that revision, and published nothing. Do not
claim every refusal trial exercised post-bump validation.

Actual client isolation controls demonstrated denied outside writes, denied raw
TCP connections and absent provider credentials in tool subprocesses. Some
Claude preparation trials attempted client-memory access outside the fixture;
the boundary denied it and their reports acknowledged the failure. This is
boundary-enforcement evidence, not perfect tool-selection behavior.

The instrumentation is visible to the agent; no tampering was observed. Rust
acceptance/integration tests independently cover squash/rebase identity,
conflicting tags, signing, partial local creation, lost push responses,
unsupported atomic publication and concurrent ref movement. The final local-ref
readback hardening followed the pinned skill comparison build and is covered by
those Rust regressions and the full repository checks.

Claude experiment cost, including setup pilots and repetitions, was USD 8.876,
below the USD 30 battery limit. Pre-implementation Fable reviews are separate.
No real release, protected branch, hosted CI policy or registry was modified.
