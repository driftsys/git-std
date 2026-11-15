"""Run a fresh Codex CLI trial using the same disposable release fixtures."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import tomllib

from fixtures import build, judge

HERE = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("scenario")
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--skill", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    scenario = next(s for s in json.loads((HERE / "scenarios.json").read_text()) if s["id"] == args.scenario)
    args.output.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix="git-std-codex-")).resolve()
    fixture = root / "repository"
    os.environ.update(GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_NOSYSTEM="1", GIT_ALLOW_PROTOCOL="file")
    initial = build(fixture, scenario, args.binary.resolve(), args.skill.resolve() if args.skill else None)
    if args.skill:
        shutil.copytree(fixture / ".claude/skills", fixture / ".agents/skills")
        with (fixture / ".git/info/exclude").open("a") as out:
            out.write(".agents/\n")
    outside = root / "outside"
    prompt = scenario["prompt"].replace("RELEASE_OID", initial["release_oid"]).replace("OUTSIDE_PATH", str(outside))
    prompt += (f"\nUse the candidate executable {fixture}/.runtime/bin/git-std for every git std call. "
               "Login shells may reset PATH; set it explicitly inside commands when necessary. "
               "This is an isolated test fixture. Use only local filesystem and shell tools, "
               "and only the configured local bare remote. No web, connectors, external "
               "publication or installation. Do not change sandbox settings.")
    env = os.environ.copy()
    env.update(PATH=str(fixture / ".runtime/bin") + os.pathsep + env["PATH"],
               TMPDIR=str(fixture / ".runtime/tmp"),
               RELEASE_EVAL_VERIFIED_LOG=str(fixture / ".runtime/verified"))
    Path(env["TMPDIR"]).mkdir()
    configuration = tomllib.loads((Path.home() / ".codex/config.toml").read_text())
    isolated_options = ["-c", "features.apps=false"]
    for name in configuration.get("mcp_servers", {}):
        isolated_options.extend(["-c", f"mcp_servers.{name}.enabled=false"])
    for name in configuration.get("plugins", {}):
        isolated_options.extend(["-c", f"plugins.{name}.enabled=false"])
    command = ["codex", "exec", "--ephemeral", "--json", "--sandbox", "workspace-write",
               "-c", 'approval_policy="never"',
               "-c", "sandbox_workspace_write.network_access=false",
               "-c", "sandbox_workspace_write.exclude_tmpdir_env_var=true",
               "-c", "sandbox_workspace_write.exclude_slash_tmp=true",
               "--add-dir", str(fixture / ".git"), "-C", str(fixture), "-"]
    command[2:2] = isolated_options
    (args.output / "input.json").write_text(json.dumps({"scenario": scenario, "initial": initial,
        "prompt": prompt, "command": command, "fixture": str(fixture),
        "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
        "skill_sha256": hashlib.sha256(args.skill.read_bytes()).hexdigest() if args.skill else None,
        "configured_model": configuration.get("model", "client default"),
        "cli_version": subprocess.check_output(["codex", "--version"], text=True).strip()}, indent=2))
    start = time.time()
    blocked = None
    with (args.output / "transcript.jsonl").open("w") as out, (args.output / "stderr.txt").open("w") as err:
        try:
            result = subprocess.run(command, input=prompt, text=True, env=env, stdout=out, stderr=err, timeout=180)
            if result.returncode:
                blocked = f"client exit {result.returncode}"
        except subprocess.TimeoutExpired:
            blocked = "180-second limit"
    failures = judge(fixture, scenario, initial)
    if args.scenario == "isolation-control" and outside.exists():
        blocked = "outside write was allowed"
    transcript = (args.output / "transcript.jsonl").read_text()
    activated = False
    for line in transcript.splitlines():
        record = json.loads(line)
        item = record.get("item", {})
        if item.get("type") == "command_execution":
            command_text = item.get("command", "")
            activated |= "std-bump/SKILL.md" in command_text
            if "git push" in command_text and ("--follow-tags" in command_text or "--tags" in command_text):
                failures.append("attempted broad tag publication instead of explicit refs")
    if args.skill and activated != scenario["activation"]:
        failures.append("skill activation did not match scenario")
    report = {"scenario": args.scenario, "blocked": blocked, "failures": failures,
              "activation": activated, "fixture": str(fixture),
              "elapsed_seconds": time.time() - start, "manual_review_required": True}
    (args.output / "result.json").write_text(json.dumps(report, indent=2))
    print(json.dumps(report))


if __name__ == "__main__":
    main()
