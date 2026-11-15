# Release skill evaluations

Run each scenario in `scenarios.json` with a fresh client session and an
isolated repository. The candidate binary is identical in baseline and revised
runs, except for the explicit old-binary scenario. Store full transcripts,
commands, skill hashes, model/version, costs and Git state in ignored run output.

The deterministic judge checks the target commit, remote branch and tag refs,
commit count, preserved dirty work, and validation order. Manual transcript
review checks activation and unsupported claims. An in-scope scenario requires
loading `std-bump`; the unrelated request must not load it. Direct invocation
is a separate control and cannot prove discovery.

Baseline failures establish missing guidance. Every revised trial on both
Claude CLI and fresh Codex sessions must pass. Repeat direct release, two-step,
advanced main, failed validation, partial push and existing authorization once.
Use at most 180 seconds and USD 1 per Claude invocation, USD 30 per battery.
Timeout, isolation failure, authentication failure and budget exhaustion are
blocked results, never passes.

Before any mutation trial, the harness must prove that external network,
external Git publication and writes outside its fixture are denied. Only
local bare remotes are used; publication tools are unavailable. Claude provider
authentication remains in the parent process and is never included in artifacts.

The original, no-skill and candidate trials share prompts and written criteria.
Keep evaluation files out of generated client skills; only `SKILL.md` and its
user-facing references belong in the installed skill.

## Run a trial

Build the candidate once, then keep that executable unchanged for both baseline
and revised runs:

```bash
cargo build -p git-std
python3 tests/skills/std-bump/run.py direct \
  --binary target/debug/git-std --skill skills/std-bump/SKILL.md \
  --output /tmp/git-std-evals/claude-direct
python3 tests/skills/std-bump/run_codex.py direct \
  --binary target/debug/git-std --skill skills/std-bump/SKILL.md \
  --output /tmp/git-std-evals/codex-direct
```

Omit `--skill` for the no-skill baseline; supply the original skill for the
current-skill baseline. Use a new output directory for every trial. Run
`isolation-control` first and inspect the actual tool results: a successful
no-op, a denied outside write, a denied raw socket connection and absent provider
credentials in the tool subprocess. A DNS failure alone is insufficient evidence
of network isolation. The runners target macOS, where Claude's native sandbox
and `sandbox-exec` are available.

Claude uses `claude-fable-5-2`, project-only discovery, a strict empty MCP
configuration and a tool allowlist. It reuses an existing login without writing
credentials into artifacts. Codex uses the configured model, ephemeral sessions,
workspace-write with explicit fixture Git metadata access, disabled temporary
root allowances, disabled network access, and disabled configured MCP servers
and plugins. Provider access belongs to each client process. Marketplace refresh
warnings can occur because external Git protocols are intentionally prohibited.

The bare remote's receive hook records attempts to publish a release tag before
the exact target's validation succeeds. Later validation cannot erase that
failure. Run the deterministic judge's mutation controls with:

```bash
RELEASE_EVAL_BINARY="$PWD/target/debug/git-std" \
  python3 -m unittest discover -s tests/skills/std-bump -p test_judge.py
```

Inspect transcripts in addition to state assertions, including exact selected
refspecs, atomic publication, no repeated approval requests, and truthful final
reports. Mark setup failures as blocked, retain their evidence, fix the harness,
and rerun; never count them as skill passes or skill failures. Sum recorded
Claude costs across the battery, reserving USD 1 for each concurrent invocation
before starting it, and stop before the total can exceed USD 30.
