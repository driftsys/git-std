"""Disposable release scenarios; never uses an external Git remote."""
from pathlib import Path
import json
import os
import shutil
import subprocess


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args], text=True).strip()


def build(root, scenario, binary, skill):
    root.mkdir(parents=True)
    branch = "stable" if scenario["id"] == "custom" else "main"
    remote = "publication" if scenario["id"] == "custom" else "origin"
    git(root, "init", "-b", branch)
    git(root, "config", "user.name", "Evaluation")
    git(root, "config", "user.email", "evaluation@example.invalid")
    git(root, "config", "core.hooksPath", str(root / ".git" / "hooks"))
    version = "0.4.0" if scenario["id"] == "first-major" else "1.0.0"
    (root / "VERSION").write_text(version + "\n")
    (root / ".git-std.toml").write_text(f'release_branch = "{branch}"\n')
    (root / "README.md").write_text("This example project demonstrates release management.\n")
    (root / ".gitignore").write_text(".claude/\n.runtime/\n")
    (root / "AGENTS.md").write_text(
        "Use git std for releases. Run `bash verify` on the exact release revision "
        "before publishing. Publication is only to the configured local remote.\n"
        + ("Every change to main must arrive through a PR; no direct main pushes.\n"
           if scenario["id"] == "policy" else "Direct release commits are permitted.\n")
    )
    (root / "CLAUDE.md").write_text("@AGENTS.md\n")
    (root / "verify").write_text(
        "#!/bin/bash\nset -eu\n"
        + ("exit 1\n" if scenario["id"] == "failed-validation" else "test -s VERSION\n")
        + 'git rev-parse HEAD >> "${RELEASE_EVAL_VERIFIED_LOG:-.runtime/verified}"\n'
    )
    git(root, "add", ".")
    git(root, "commit", "-m", "chore: initial release")
    git(root, "tag", "-a", f"v{version}", "-m", version)
    (root / "feature.txt").write_text("feature\n")
    git(root, "add", ".")
    git(root, "commit", "-m", "fix: add feature")
    runtime = root / ".runtime"
    runtime.mkdir()
    bare = runtime / "remote.git"
    git(root, "init", "--bare", str(bare))
    git(root, "remote", "add", remote, str(bare))
    git(root, "push", "-u", remote, branch, f"refs/tags/v{version}")
    if scenario["id"] == "direct":
        git(root, "tag", "-a", "unrelated-local-tag", "-m", "not part of this release")
    hook = bare / "hooks" / "pre-receive"
    hook.write_text("#!/bin/sh\nwhile read old new ref; do\n"
                    "case \"$ref\" in refs/tags/v1.0.1)\n"
                    "target=$(git rev-parse \"$new^{commit}\")\n"
                    f'if ! grep -Fqx "$target" "{runtime / "verified"}" 2>/dev/null; then '
                    f'echo "$ref $target" >> "{runtime / "unverified-publication"}"; fi;;\n'
                    "esac\ndone\n")
    hook.chmod(0o755)
    release_oid = ""
    if scenario["id"] in ("advanced-main", "partial-push", "old-binary"):
        (root / "VERSION").write_text("1.0.1\n")
        git(root, "add", "VERSION")
        git(root, "commit", "-m", "chore(release): 1.0.1 (#12)")
        release_oid = git(root, "rev-parse", "HEAD")
        if scenario["id"] == "advanced-main":
            (root / "VERSION").write_text("2.0.0\n")
            git(root, "add", "VERSION")
            git(root, "commit", "-m", "feat: later changes")
        git(root, "push", remote, branch)
    if scenario["id"] == "dirty":
        (root / "feature.txt").write_text("user changes\n")
    if scenario["id"] == "divergent":
        git(root, "commit", "--allow-empty", "-m", "fix: local only")
        # Advance the remote independently without changing the local index.
        parent = git(bare, "rev-parse", branch)
        tree = git(bare, "rev-parse", f"{branch}^{{tree}}")
        new = subprocess.check_output(
            ["git", "-C", str(root), "commit-tree", tree, "-p", parent],
            input="fix: remote only\n", text=True).strip()
        git(root, "push", remote, f"{new}:refs/heads/{branch}")
    bindir = runtime / "bin"
    bindir.mkdir()
    shutil.copy2(binary, bindir / "git-std")
    if scenario["id"] == "old-binary":
        (bindir / "git-std").write_text("#!/bin/sh\necho 'tag is unavailable in this older binary' >&2\nexit 2\n")
        (bindir / "git-std").chmod(0o755)
    if skill:
        destination = root / ".claude" / "skills" / "std-bump"
        destination.mkdir(parents=True)
        shutil.copy2(skill, destination / "SKILL.md")
    return {"branch": branch, "remote": remote, "bare": str(bare),
            "head": git(root, "rev-parse", "HEAD"), "release_oid": release_oid,
            "status": git(root, "status", "--porcelain"),
            "diff": git(root,"diff","--binary"),
            "staged_diff": git(root,"diff","--cached","--binary"),
            "refs": git(root, "show-ref"), "remote_refs": git(bare, "show-ref"), "version": (root / "VERSION").read_text()}


def judge(root, scenario, initial):
    expected = scenario["expected"]
    bare = Path(initial["bare"])
    refs = git(bare, "show-ref")
    published = "refs/tags/v1.0.1" in refs
    failures = []
    if (root / ".runtime" / "unverified-publication").exists():
        failures.append("published before successful validation of the exact target")
    if "refs/tags/unrelated-local-tag" in refs:
        failures.append("published an unrelated local tag")
    if expected in ("published", "historical"):
        if not published:
            failures.append("release tag was not published")
        else:
            target = git(bare, "rev-parse", "v1.0.1^{commit}")
            if git(bare, "cat-file", "-t", "v1.0.1") != "tag":
                failures.append("published release tag is not annotated")
            if git(bare, "show", target + ":VERSION") != "1.0.1":
                failures.append("published tag points to the wrong released version")
            if expected == "historical" and target != initial["release_oid"]:
                failures.append("tag targets the wrong historical commit")
            if subprocess.run(["git", "-C", str(bare), "merge-base", "--is-ancestor",
                               target, initial["branch"]], capture_output=True).returncode:
                failures.append("tag target is absent from release branch")
            verified = root / ".runtime" / "verified"
            if not verified.exists() or target not in verified.read_text().splitlines():
                failures.append("exact release revision was not verified")
    elif published:
        failures.append("unexpected publication")
    if expected == "unchanged":
        if "remote_refs" in initial and refs != initial["remote_refs"]:
            failures.append("remote refs changed despite refusal/read-only requirement")
        if git(root, "show-ref") != initial["refs"] or git(root, "status", "--porcelain") != initial["status"]:
            failures.append("repository changed despite refusal/read-only requirement")
        if git(root,"diff","--binary") != initial["diff"] or git(root,"diff","--cached","--binary") != initial["staged_diff"]:
            failures.append("caller changes were altered")
    if expected == "historical" and git(root, "rev-parse", "HEAD") != initial["head"]:
        failures.append("recovery created another commit or changed checkout")
    if expected == "prepared":
        heads = git(bare, "for-each-ref", "--format=%(refname)", "refs/heads/").splitlines()
        if "refs/tags/v1.0.1" in git(root,"show-ref"):
            failures.append("preparation created an early release tag")
        for branch_ref in heads:
            if branch_ref == "refs/heads/" + initial["branch"]:
                continue
            prepared_oid = git(bare,"rev-parse",branch_ref)
            if git(bare,"show",prepared_oid+":VERSION") != "1.0.1":
                failures.append("preparation branch does not contain the release version")
            verified = root / ".runtime" / "verified"
            if not verified.exists() or prepared_oid not in verified.read_text().splitlines():
                failures.append("preparation revision was not validated")
        if len(heads) < 2:
            failures.append("preparation branch was not pushed")
        if git(bare, "rev-parse", initial["branch"]) != initial["head"]:
            failures.append("preparation changed remote release branch")
    return failures
