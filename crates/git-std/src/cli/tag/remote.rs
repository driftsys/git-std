use super::{
    tags,
    types::{TagPlan, refuse, require},
};
use crate::git::release_refs as git;
use anyhow::Result;
use std::{collections::BTreeMap, path::Path};

type Refs = BTreeMap<String, String>;

pub(super) fn endpoint(root: &Path, name: &str) -> Result<String> {
    require(
        !name.is_empty() && !name.starts_with('-') && !name.contains(['/', ':']),
        "--push requires a configured remote name",
    )?;
    let remotes = git::text(root, &["remote"])?;
    require(
        remotes.lines().any(|r| r == name),
        "publication remote is not configured",
    )?;
    let fetch = git::text(root, &["remote", "get-url", "--all", name])?;
    let push = git::text(root, &["remote", "get-url", "--push", "--all", name])?;
    require(
        fetch.lines().count() == 1 && push.lines().count() == 1 && fetch == push,
        "publication requires one identical fetch and push URL",
    )?;
    require(
        git::optional(
            root,
            &[
                "config",
                "--bool",
                "--get",
                &format!("remote.{name}.mirror"),
            ],
        )?
        .as_deref()
            != Some("true"),
        "mirror remotes cannot publish release tags",
    )?;
    require(!fetch.starts_with('-'), "invalid remote URL")?;
    Ok(fetch)
}

pub(super) fn refs(root: &Path, endpoint: &str) -> Result<Refs> {
    let listing = git::text(root, &["ls-remote", "--refs", "--", endpoint])?;
    Ok(listing
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(oid, name)| (name.into(), oid.into()))
        .collect())
}

fn fetch(root: &Path, endpoint: &str, reference: &str) -> Result<()> {
    git::text(
        root,
        &[
            "fetch",
            "--no-tags",
            "--no-write-fetch-head",
            "--no-recurse-submodules",
            "--no-auto-maintenance",
            "--refmap=",
            "--",
            endpoint,
            reference,
        ],
    )?;
    Ok(())
}

pub(super) fn membership(root: &Path, plan: &TagPlan) -> Result<Refs> {
    let endpoint = plan
        .endpoint
        .as_deref()
        .ok_or_else(|| refuse("missing remote endpoint"))?;
    let reference = format!("refs/heads/{}", plan.release_branch);
    for _ in 0..2 {
        let before = refs(root, endpoint)?;
        let oid = before
            .get(&reference)
            .ok_or_else(|| refuse("remote release branch does not exist"))?;
        fetch(root, endpoint, &reference)?;
        let after = refs(root, endpoint)?;
        if after.get(&reference) != Some(oid) {
            continue;
        }
        require(
            git::ancestor(root, &plan.target_oid, oid)?,
            "release commit is not reachable from the remote release branch",
        )?;
        return Ok(after);
    }
    anyhow::bail!("remote release branch changed during inspection; retry after it settles")
}

pub(super) fn preflight(root: &Path, plan: &mut TagPlan, sign: bool) -> Result<()> {
    let refs = membership(root, plan)?;
    let endpoint = plan
        .endpoint
        .clone()
        .ok_or_else(|| refuse("missing remote endpoint"))?;
    for tag in &mut plan.tags {
        let reference = format!("refs/tags/{}", tag.name);
        if let Some(oid) = refs.get(&reference) {
            tag.remote = "conflict";
            if let Some(local) = &tag.oid {
                require(
                    local == oid,
                    format!(
                        "tag_object_conflict: {}; inspect both tag objects and reconcile the local ref separately",
                        tag.name
                    ),
                )?;
            } else {
                fetch(root, &endpoint, &reference)?;
                tags::validate(
                    root,
                    &plan.target_oid,
                    tag,
                    oid,
                    sign,
                    &mut plan.diagnostics,
                )?;
                tag.oid = Some(oid.clone());
                tag.action = "adopt";
            }
            tag.remote = "existing";
        } else {
            tag.remote = "missing";
        }
    }
    Ok(())
}

pub(super) fn publish(root: &Path, plan: &mut TagPlan) -> Result<()> {
    let endpoint = plan
        .endpoint
        .clone()
        .ok_or_else(|| refuse("missing remote endpoint"))?;
    let refspecs: Vec<_> = plan
        .tags
        .iter()
        .filter(|t| t.remote == "missing")
        .map(|t| {
            t.oid
                .as_ref()
                .map(|oid| format!("{oid}:refs/tags/{}", t.name))
                .ok_or_else(|| anyhow::anyhow!("missing validated tag object: {}", t.name))
        })
        .collect::<Result<_>>()?;
    if !refspecs.is_empty() {
        let mut args = vec![
            "push",
            "--atomic",
            "--no-follow-tags",
            "--recurse-submodules=no",
            "--",
            endpoint.as_str(),
        ];
        args.extend(refspecs.iter().map(String::as_str));
        // A lost response may mean the server accepted the push; never assume missing.
        for tag in &mut plan.tags {
            if tag.remote == "missing" {
                tag.remote = "unknown";
            }
        }
        git::text(root, &args)?;
    }
    let observed =
        membership(root, plan).map_err(|e| anyhow::anyhow!("cannot verify publication: {e}"))?;
    for tag in &mut plan.tags {
        let actual = observed.get(&format!("refs/tags/{}", tag.name));
        if actual != tag.oid.as_ref() {
            tag.remote = "unknown";
            anyhow::bail!("cannot verify remote tag {}", tag.name);
        }
        if tag.remote == "unknown" {
            tag.remote = "pushed";
        }
        if git::ref_oid(root, &format!("refs/tags/{}", tag.name))? != tag.oid {
            tag.local = "conflict";
            anyhow::bail!("local tag changed during publication: {}", tag.name);
        }
    }
    Ok(())
}
