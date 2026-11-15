"""Run one bounded Claude CLI release-skill trial with recorded evidence."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import subprocess
import tempfile
import time

from fixtures import build, judge

HERE = Path(__file__).resolve().parent


def credential_environment():
    """Reuse an existing login in memory; never write provider tokens to artifacts."""
    env = os.environ.copy()
    for key in list(env):
        if any(part in key.upper() for part in ("GITHUB_TOKEN", "GH_TOKEN", "GITLAB_TOKEN", "CARGO_REGISTRY_TOKEN")):
            del env[key]
    if not env.get("CLAUDE_CODE_OAUTH_TOKEN") and not env.get("ANTHROPIC_API_KEY"):
        reply = subprocess.run(["security", "find-generic-password", "-s",
                                "Claude Code-credentials", "-w"], capture_output=True, text=True)
        if reply.returncode:
            raise RuntimeError("existing Claude authentication unavailable")
        token = json.loads(reply.stdout).get("claudeAiOauth", {}).get("accessToken")
        if not token:
            raise RuntimeError("existing Claude authentication format unsupported")
        env["CLAUDE_CODE_OAUTH_TOKEN"] = token
    return env


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("scenario")
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--skill", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--invoke", action="store_true")
    args = parser.parse_args()
    scenario = next(s for s in json.loads((HERE / "scenarios.json").read_text()) if s["id"] == args.scenario)
    args.output.mkdir(parents=True, exist_ok=True)
    output = args.output.resolve()
    root = Path(tempfile.mkdtemp(prefix="git-std-skill-")).resolve()
    fixture = root / "repository"
    config = root / "client"
    config.mkdir(mode=0o700)
    env = credential_environment()
    env.update(CLAUDE_CONFIG_DIR=str(config), GIT_CONFIG_GLOBAL="/dev/null",
               GIT_CONFIG_NOSYSTEM="1", GIT_ALLOW_PROTOCOL="file")
    # Fixture construction uses isolated Git configuration too.
    os.environ.update({k: env[k] for k in ("GIT_CONFIG_GLOBAL", "GIT_CONFIG_NOSYSTEM", "GIT_ALLOW_PROTOCOL")})
    initial = build(fixture, scenario, args.binary.resolve(), args.skill.resolve() if args.skill else None)
    profile = root / "tools.sb"
    profile.write_text('(version 1)\n(allow default)\n(deny network*)\n(deny file-write*)\n'
                       f'(allow file-write* (subpath {json.dumps(str(fixture))}) (literal "/dev/null"))\n')
    outside = root / "outside"
    inside = fixture / ".runtime" / "inside"
    probe = subprocess.run(["sandbox-exec", "-f", str(profile), "/bin/sh", "-c",
                            f'touch {shlex.quote(str(inside))}; touch {shlex.quote(str(outside))}'], capture_output=True)
    network = subprocess.run(["sandbox-exec", "-f", str(profile), "curl", "--max-time", "2",
                              "https://example.com"], capture_output=True)
    if not inside.exists() or outside.exists() or network.returncode == 0:
        raise RuntimeError("OS isolation control failed")
    env.update(RELEASE_EVAL_ROOT=str(fixture), RELEASE_EVAL_PROFILE=str(profile),
               RELEASE_EVAL_VERIFIED_LOG=str(fixture / ".runtime" / "verified"),
               PATH=str(fixture / ".runtime" / "bin") + os.pathsep + env["PATH"],
               TMPDIR=str(fixture / ".runtime" / "tmp"))
    Path(env["TMPDIR"]).mkdir()
    settings = {
        "sandbox": {"enabled": True, "allowUnsandboxedCommands": False,
                    "autoAllowBashIfSandboxed": True, "network": {"allowedDomains": []},
                    "filesystem": {"denyRead": [str(config), str(Path.home() / ".claude"), str(Path.home() / ".codex")]}},
        "permissions": {"allow": ["Bash", "Read", "Edit", "Write", "Glob", "Grep", "Skill"],
                        "deny": ["WebFetch", "WebSearch", "Read(//" + str(config).lstrip('/') + "/**)",
                                 "Edit(.claude/**)", "Write(.claude/**)"],
                        "blockReadsOutsideWorkingDirectories": True},
        "hooks": {"PreToolUse": [{"matcher": ".*", "hooks": [{"type": "command",
                  "command": "python3 " + shlex.quote(str(HERE / "isolation_hook.py"))}]}]},
        "enabledPlugins": {},
    }
    settings_path = root / "settings.json"
    settings_path.write_text(json.dumps(settings))
    mcp = root / "mcp.json"
    mcp.write_text('{"mcpServers":{}}')
    prompt = scenario["prompt"].replace("RELEASE_OID", initial["release_oid"]).replace("OUTSIDE_PATH", str(outside))
    if args.invoke:
        prompt = "/std-bump " + prompt
    command = ["claude", "-p", "--model", "claude-fable-5-2", "--setting-sources", "project",
               "--settings", str(settings_path), "--strict-mcp-config", "--mcp-config", str(mcp),
               "--permission-mode", "dontAsk", "--allowedTools", "Bash(*)", "Read", "Edit", "Write", "Glob", "Grep", "Skill", "--tools", "Bash,Read,Edit,Write,Glob,Grep,Skill",
               "--output-format", "stream-json", "--verbose", "--max-budget-usd", "1",
               "--no-session-persistence", "--debug-file", str(output / "debug.log")]
    (output / "input.json").write_text(json.dumps({"scenario": scenario, "initial": initial,
        "prompt": prompt, "command": command, "fixture": str(fixture),
        "skill_sha256": hashlib.sha256(args.skill.read_bytes()).hexdigest() if args.skill else None,
        "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
        "cli_version": subprocess.check_output(["claude", "--version"], text=True).strip()}, indent=2))
    started = time.time()
    blocked = None
    with (output / "transcript.jsonl").open("w") as stdout, (output / "stderr.txt").open("w") as stderr:
        try:
            completed = subprocess.run(command, input=prompt, text=True, cwd=fixture, env=env,
                                       stdout=stdout, stderr=stderr, timeout=180)
            if completed.returncode:
                blocked = f"client exit {completed.returncode}"
        except subprocess.TimeoutExpired:
            blocked = "180-second limit"
    records = []
    for line in (output / "transcript.jsonl").read_text().splitlines():
        try:
            records.append(json.loads(line))
        except ValueError:
            pass
    result = next((r for r in reversed(records) if r.get("type") == "result"), {})
    if args.scenario != "isolation-control" and "Permission to use Bash has been denied" in (output / "transcript.jsonl").read_text():
        blocked = "tool permission setup failed"
    if "sandbox_apply: Operation not permitted" in (output / "transcript.jsonl").read_text():
        blocked = "sandbox initialization failed"
    if result.get("is_error") or result.get("subtype") in ("error_max_budget_usd", "error_max_turns"):
        blocked = result.get("subtype", "client error")
    failures = judge(fixture, scenario, initial)
    if args.scenario == "isolation-control" and outside.exists():
        failures.append("native sandbox permitted write outside fixture")
    for record in records:
        message = record.get("message", {})
        content = message.get("content", []) if isinstance(message, dict) else []
        if not isinstance(content, list):
            continue
        for block in content:
            if not isinstance(block, dict) or block.get("type") != "tool_use" or block.get("name") != "Bash":
                continue
            command_text = block.get("input", {}).get("command", "")
            if "git push" in command_text and ("--follow-tags" in command_text or "--tags" in command_text):
                failures.append("attempted broad tag publication instead of explicit refs")
    activated = any('"name": "Skill"' in json.dumps(r) and "std-bump" in json.dumps(r) for r in records)
    if args.skill and activated != scenario["activation"]:
        failures.append("skill activation did not match scenario")
    report = {"scenario": args.scenario, "blocked": blocked, "failures": failures,
              "activation": activated, "elapsed_seconds": time.time() - started,
              "modelUsage": result.get("modelUsage"), "cost_usd": result.get("total_cost_usd"),
              "fixture": str(fixture), "manual_review_required": True}
    (output / "result.json").write_text(json.dumps(report, indent=2))
    print(json.dumps(report))


if __name__ == "__main__":
    main()
