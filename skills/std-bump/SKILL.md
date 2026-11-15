---
schema: 1
name: std-bump
description: Use when asked to bump a version, cut or prepare a release, tag an existing release, or recover interrupted release publication with git std.
---

# Release with git std

Preserve the release identity: every published tag must point to the intended
release commit in the configured release branch's history. Prefer a combined
bump and tag. Use preparation followed by tagging when requested or required by
repository policy. Never bump again to recover an existing release.

## Establish context and authorization

1. Read repository instructions, release documentation and `.git-std.toml`.
   Inspect `git std --version`, `git std --context` and relevant command help.
   If unavailable or not bootstrapped, explain the blocker. Ask before installing.
   An older binary without `git std tag` needs an approved upgrade; do not silently
   substitute a manual tagging workflow.
2. Resolve the repository root with Git. Discover the configured release branch,
   upstream and publication remote. Do not hard-code `main`, `master` or `origin`.
   For publication require one identical effective fetch/push URL, no mirror
   remote, and explicit selected refs. Inspect current remote state.
3. Preserve dirty, untracked and staged work. Before a new bump, require a clean
   release base synchronized in both directions with the remote. Stop on
   divergence; do not reset, stash, force-push or discard work to make it pass.
   If the caller's checkout is dirty, stop this new-release request before any
   bump, branch change or publication. Do not clone or create a clean worktree
   to work around this admission failure. Ask the user to resolve it first.
   Respect branches owned by other worktrees.
4. Reuse decisions already authorized in this session: workflow, version, package
   scope, signing and publication. Ask only for missing decisions. General
   release approval does not authorize bypassing branch protection. A requested
   preview authorizes no mutation. Promotion from 0.x to 1.0 requires an explicit
   stability commitment before adding `--first-major-release`.

## Plan a new release

Run `git std bump --dry-run --format json` with the intended version and package
options. Read the planned version, files, tags and warnings. Keep the complete
package/version list for a multi-package release. Do not force a bump when no
bump-worthy commits exist without approval. Select `--first-release`,
`--prerelease`, `--release-as` or package flags only when warranted.

Show the concrete plan. Existing approval for that exact scope is sufficient;
ask if the plan changes it. Where supported, bind apply with `--expect-plan`
using the returned `plan_id`; keep preview and apply options consistent.

## Preferred: combined bump and tag

Use this when direct release commits on the release branch are permitted.

1. Run `git std bump` locally with the approved options. Do not use `--push` yet.
2. Record the generated release OID and exact annotated tag names and objects.
   Run the repository's required checks on this generated revision. Checks run
   before the bump do not validate its manifest, lockfile and changelog changes.
3. If checks fail, stop publication and report local state. Do not bypass checks
   or amend a tagged release silently.
4. When publication is authorized and checks pass, push the release branch and
   only the selected tags together with `git push --atomic --no-follow-tags
   --recurse-submodules=no <remote> <explicit-branch-refspec>
   <explicit-tag-refspec>...`. Refuse an unsupported atomic push; do not fall back
   to sequential pushes. Never use `--tags`, `--follow-tags` or force.
5. Read back the remote branch and tag objects; verify each tag's peeled commit
   belongs to that branch. A push exit alone is not final verification.

## Supported: prepare, then tag after merge

Use this for release PR policy or an explicitly requested two-step process.

1. Create an isolated preparation branch from the synchronized release base.
   Preview and apply `git std bump --no-tag` with the approved options and
   `--yes` to acknowledge preparation off the release branch. Do not use
   `--no-commit`: the generated release commit is the handoff identity.
2. Check the generated revision. Push only the preparation branch explicitly
   when authorized; `--no-tag --push` does not push it. Create the release PR
   where a forge is available. Preserve the generated `chore(release): ...`
   subject and full version list as the PR's merge identity.
3. If unmerged, stop with branch, preparation OID, planned tags, PR status and
   checks recorded. Do not tag the preparation commit early.
4. After merge, obtain the resulting release OID from authoritative merge
   evidence. Squash/rebase can rewrite it. Fetch it; never substitute current
   HEAD just because main has advanced. Validate that exact merged revision in
   an isolated checkout, preserving the caller's checkout and local work.
5. Inspect `git std tag <merged-release-OID> --dry-run --push <remote>` with the
   approved package/signing options. Then apply the same command without
   `--dry-run`. Omit `--push` for an authorized local-only tag. This command
   reads historical configuration and versions, checks branch membership,
   creates or reuses annotated tags, and pushes only those tags atomically.

## Recover and report

On any interrupted or rejected publication, inspect local and remote branch,
annotated tag object and peeled commit identities before deciding what remains.
If the release commit is already on the remote release branch, retry
`git std tag <release-OID> --push <remote>` after its dry-run and required checks.
Do not run another bump. Matching existing tags are reused; a remote-only tag
can be adopted. Conflicting objects or targets require explicit reconciliation;
do not delete, retag, overwrite or force them. Exit 2 may mean partial local
creation or uncertain remote acceptance, not that nothing happened.

Report separately: local release commit, local tags, remote branch, published
tags, exact revision checked and check results, and remaining work. Only claim
hosted CI or package publication when independently observed. Include a resumable
handoff for pending PRs, failed checks or incomplete publication.
