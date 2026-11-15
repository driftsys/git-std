use super::{
    identity, remote, snapshot,
    types::{PlannedTag, TagOptions, TagPlan, refuse, require},
    versions,
};
use crate::{config, git::release_refs as git};
use anyhow::{Context, Result};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub(super) fn repository() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    require(
        git::output(&cwd, &["--version"])?.status.success(),
        "Git lacks required --no-lazy-fetch capability; upgrade Git",
    )?;
    let help = git::output(&cwd, &["fetch", "-h"])?;
    let help = format!(
        "{}{}",
        String::from_utf8_lossy(&help.stdout),
        String::from_utf8_lossy(&help.stderr)
    );
    require(
        help.contains("write-fetch-head"),
        "Git lacks fetch without FETCH_HEAD; upgrade Git",
    )?;
    let root = PathBuf::from(git::text(&cwd, &["rev-parse", "--show-toplevel"])?);
    require(
        git::text(&root, &["rev-parse", "--is-shallow-repository"])? == "false",
        "shallow repository: fetch complete history before tagging",
    )?;
    require(
        git::optional(&root, &["config", "--get", "extensions.partialclone"])?.is_none(),
        "partial repositories are not supported",
    )?;
    if let Some(promisors) = git::optional(
        &root,
        &["config", "--get-regexp", r"^remote\..*\.promisor$"],
    )? {
        require(
            !promisors.lines().any(|l| !l.ends_with(" false")),
            "promisor repositories are not supported",
        )?;
    }
    Ok(root)
}

pub(super) fn build(root: &Path, options: &TagOptions, plan: &mut TagPlan) -> Result<()> {
    plan.target_oid = git::text(
        root,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{}^{{commit}}", options.commit),
        ],
    )
    .map_err(|e| refuse(format!("cannot resolve release commit: {e}")))?;
    let (snapshot, config) = snapshot::inspect(root, &plan.target_oid, &mut plan.diagnostics)?;
    if let Some(name) = &options.push {
        plan.endpoint = Some(remote::endpoint(root, name)?);
    }
    let remote_refs = plan
        .endpoint
        .as_ref()
        .map(|e| remote::refs(root, e))
        .transpose()?;
    plan.release_branch =
        if let Some(branch) = options.branch.as_ref().or(config.release_branch.as_ref()) {
            branch.clone()
        } else {
            let mut chosen = None;
            for branch in ["main", "master"] {
                let reference = format!("refs/heads/{branch}");
                let exists = if let Some(refs) = &remote_refs {
                    refs.contains_key(&reference)
                } else {
                    git::ref_oid(root, &reference)?.is_some()
                };
                if exists {
                    chosen = Some(branch.into());
                    break;
                }
            }
            chosen.ok_or_else(|| refuse("no release branch found; specify --branch"))?
        };
    require(
        !plan.release_branch.starts_with('-')
            && git::output(
                root,
                &[
                    "check-ref-format",
                    &format!("refs/heads/{}", plan.release_branch),
                ],
            )?
            .status
            .success(),
        "invalid release branch",
    )?;
    if options.push.is_none() {
        let reference = format!("refs/heads/{}", plan.release_branch);
        let oid = git::ref_oid(root, &reference)?
            .ok_or_else(|| refuse("local release branch is missing"))?;
        require(
            git::ancestor(root, &plan.target_oid, &oid)?,
            "release commit is not reachable from the local release branch",
        )?;
    }
    let subject = git::text(root, &["show", "-s", "--format=%s", &plan.target_oid])?;
    let entries = identity::parse_release_subject(&subject, config.monorepo)
        .map_err(|e| refuse(e.to_string()))?;
    let packages = config.resolved_packages(snapshot.path());
    for package in &packages {
        snapshot::safe_path(&package.path)?;
    }
    require(
        config.monorepo || options.packages.is_empty(),
        "--package requires a monorepo release",
    )?;
    for selected in &options.packages {
        require(
            entries.iter().any(|(p, _)| p.as_ref() == Some(selected)),
            format!("package is absent from release: {selected}"),
        )?;
    }
    let mut names = BTreeSet::new();
    for (package, version) in entries {
        let (directory, custom, scheme, name) = if let Some(package) = &package {
            let pkg = packages
                .iter()
                .find(|p| &p.name == package)
                .ok_or_else(|| refuse(format!("unknown release package: {package}")))?;
            (
                snapshot.path().join(&pkg.path),
                pkg.version_files.as_deref().unwrap_or(&[]),
                pkg.scheme.as_ref().unwrap_or(&config.scheme),
                config
                    .versioning
                    .tag_template
                    .replace("{name}", package)
                    .replace("{version}", &version),
            )
        } else {
            (
                snapshot.path().to_owned(),
                config.version_files.as_slice(),
                &config.scheme,
                format!("{}{version}", config.versioning.tag_prefix),
            )
        };
        versions::validate_version(&config, scheme, &version)?;
        let basis = versions::check(&directory, snapshot.path(), &version, custom)?;
        if !config.monorepo && snapshot.path().join("Cargo.toml").exists() {
            let source = std::fs::read_to_string(snapshot.path().join("Cargo.toml"))?;
            let table = source
                .parse::<toml::Table>()
                .context("invalid Cargo manifest")?;
            if table.contains_key("workspace") {
                for member in config::discover_packages(snapshot.path()) {
                    snapshot::safe_path(&member.path)?;
                    versions::check(
                        &snapshot.path().join(member.path),
                        snapshot.path(),
                        &version,
                        &[],
                    )?;
                }
            }
        }
        require(names.insert(name.clone()), "release tag names collide")?;
        require(
            git::output(root, &["check-ref-format", &format!("refs/tags/{name}")])?
                .status
                .success(),
            format!("invalid release tag name: {name}"),
        )?;
        if !options.packages.is_empty()
            && !package
                .as_ref()
                .is_some_and(|p| options.packages.contains(p))
        {
            continue;
        }
        plan.tags.push(PlannedTag {
            name,
            version,
            version_basis: basis,
            oid: None,
            local: "missing",
            remote: "not_checked",
            action: "create",
            signature: "unsigned",
        });
    }
    require(!plan.tags.is_empty(), "release contains no selected tags")
}
