# Release workflows

Prefer creating the release commit and annotated tag together on the configured
release branch. Git-std also supports preparing a release first and tagging it
later, for repositories that require release PRs or teams that prefer two steps.
Both processes leave the tag pointing to the intended release commit in the
release branch's history.

## Before releasing

Read `.git-std.toml`, repository instructions and hosting rules. Discover the
release branch and its remote; do not assume `main` and `origin`. Fetch current
remote state, preserve local work, and require a clean, synchronized release
branch before creating a new release. Stop on divergence rather than discarding
commits or force-pushing. In a worktree, avoid switching a branch owned by another
checkout.

Run `git std bump --dry-run` with the intended package and version options.
Review the version, changed files and changelog. Confirm unresolved release and
publication decisions; an agent should reuse authorization already given in the
session. Promoting 0.x to 1.0 requires an explicit stability decision and
`--first-major-release`. A prerequisite error is not permission to bypass policy.

The examples below use `main`, `origin` and `1.2.3`. Substitute the discovered
branch, remote and exact planned tags. Run the repository's validation command
on the generated release revision before publishing it. A successful check on
the revision before the bump does not validate the changed release files.

## Preferred: create the commit and tag together

Normal development changes arrive through reviewed PRs. Where release commits
may be pushed directly, create the bump commit and tag on the release branch:

```bash
git switch main
git fetch origin
git merge --ff-only origin/main
git std bump --dry-run
git std bump
just verify
git push --atomic --no-follow-tags --recurse-submodules=no origin \
  refs/heads/main:refs/heads/main refs/tags/v1.2.3:refs/tags/v1.2.3
```

`just verify` is an example: use the project's prescribed validation command.
Check synchronization in both directions before bumping; `merge --ff-only` alone
does not detect local commits ahead of the remote. For multiple package tags,
include each planned tag as an explicit refspec in the same atomic push.

An atomic push prevents a rejected branch update from leaving a published tag
whose release commit did not reach main. If the server lacks atomic support,
stop and resolve the publication process; do not retry non-atomically. Check that
the remote has one identical effective fetch/push URL before using this recipe.

`git std bump --push` and `git push --follow-tags` remain supported, but do not
guarantee atomic branch/tag publication. The agent skill uses the explicit atomic
sequence so checks run after the bump and only the selected refs are sent.

## Supported alternative: prepare, then tag

Prepare version files, changelog and the release commit without a tag:

```bash
git switch -c release/1.2.3 origin/main
git std bump --dry-run --no-tag
git std bump --no-tag --yes
just verify
git push origin HEAD:refs/heads/release/1.2.3
```

Here `--yes` acknowledges the deliberate preparation on a non-release branch;
it does not grant hosting-policy exceptions. `--no-tag --push` skips pushing,
so push the preparation branch explicitly. Open the release PR with the generated
`chore(release): 1.2.3` subject as its title. Preserve that identity when merging.
For package releases, preserve the complete generated package/version list.

If the PR is pending, stop with the preparation branch, PR and validation results
recorded. When it merges, obtain the resulting release commit ID from the forge
and fetch it. Squash and rebase merges can rewrite the original commit; a tag
created beforehand does not follow the new commit. Never infer the target solely
from the current tip of main or an ambiguous search for a release subject.

Validate the resulting release revision. If main has advanced, use an isolated
checkout of the exact release commit for validation. Then:

```bash
git fetch origin
git std tag <merged-release-commit> --push origin --dry-run
git std tag <merged-release-commit> --push origin
```

`git std tag` reads release identity, versions and tag configuration from that
commit. It creates no new release commit, changes no version or changelog,
and runs no bump lifecycle hooks. With `--push`, the observed remote release
branch is authoritative; a stale or absent local main does not block a valid
merged release. Only the selected tags are pushed atomically, never the branch.
For local-only tagging, omit `--push` and ensure the target is reachable from
the local release branch.

## Existing-release tagging reference

```text
git std tag <commit> [--dry-run] [--branch <name>]
                    [--package <name>]... [--sign]
                    [--push <remote>] [--format text|json]
```

- `<commit>` is required and pinned to its full object ID. It must identify a
  `chore(release)` commit, optionally with a GitHub squash suffix such as `(#123)`.
- `--branch` selects a maintenance/release branch. Otherwise the target commit's
  `release_branch` applies, then main/master fallback in the local or remote
  namespace being checked.
- `--package` is repeatable and selects released packages in a monorepo; root
  tags are omitted when selecting packages. Without it, all entries in the
  release subject are tagged, not every package in the workspace.
- `--sign` creates signed tags. Existing signatures are inspected and their
  trust-verification results reported separately; signature presence is not a
  verified signer-trust claim.
- `--push` requires a configured remote name with one identical effective fetch
  and push URL. Mirror remotes, shallow repositories and partial clones are
  refused. Required Git capabilities are checked before work.
- `--dry-run --push` reads remote state and may fetch objects, but changes no
  refs, working files, index or FETCH_HEAD.
- `--format json` reports the target, branch, tag states, actions and diagnostics.
  Exit 0 means the requested work is satisfied, 1 a policy/input-state refusal,
  and 2 an operational failure or partial/uncertain completion.

## Verify and recover

After publication, verify the remote tag object and that its peeled commit is
reachable from the remote release branch. Report local bump, pushed refs,
release CI, GitHub Release creation and package publication as separate states.
A pushed tag is not proof that CI or a registry publication succeeded.

On interruption, inspect local and remote refs before doing anything else.
Rerun `git std tag` with the same commit to finish missing tags or confirm a push
whose response was lost. Matching annotated tags are preserved. A lightweight
tag, different target, different annotation or distinct local/remote tag object
is a conflict, even when two tag objects peel to the same commit. Inspect and
resolve that conflict deliberately; the command never deletes or overwrites it.

If a signing operation or pre-push hook fails, some local tags may already
exist. The error reports known state; no destructive rollback occurs. Do not
repeat the bump to recover a tag push. Never move an already published protected
version tag: fix forward with a new release. These workflows do not change
hosting rulesets or disable repository validation.
