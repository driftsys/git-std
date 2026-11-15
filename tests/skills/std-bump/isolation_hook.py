"""Constrain evaluator tool processes and file tools to one disposable fixture."""
import json
import os
from pathlib import Path
import sys

request = json.load(sys.stdin)
root = Path(os.environ["RELEASE_EVAL_ROOT"]).resolve()
name = request.get("tool_name", "")
with (root / ".runtime" / "tool-hooks.jsonl").open("a") as log:
    log.write(json.dumps({"tool": name, "input": request.get("tool_input", {})}) + "\n")
arguments = request.get("tool_input", {})
if name == "Bash":
    response = {"permissionDecision": "allow"}
elif name in ("Read", "Edit", "Write", "Glob", "Grep"):
    supplied = arguments.get("file_path", arguments.get("path", str(root)))
    path = Path(supplied)
    if not path.is_absolute():
        path = root / path
    permitted = path.resolve().is_relative_to(root)
    if name in ("Edit", "Write") and ".claude" in path.parts:
        permitted = False
    response = {"permissionDecision": "allow" if permitted else "deny"}
else:
    response = {"permissionDecision": "allow" if name == "Skill" else "deny"}
response["hookEventName"] = "PreToolUse"
print(json.dumps({"hookSpecificOutput": response}))
