use super::types::{PlannedTag, TagPlan, require};
use crate::git::release_refs as git;
use anyhow::Result;
use std::path::Path;

pub(super) fn validate(
    root: &Path,
    target: &str,
    tag: &mut PlannedTag,
    oid: &str,
    sign: bool,
    diagnostics: &mut Vec<String>,
) -> Result<()> {
    require(
        git::text(root, &["cat-file", "-t", oid])? == "tag",
        format!("{} is a lightweight tag", tag.name),
    )?;
    let content = String::from_utf8(git::bytes(root, &["cat-file", "tag", oid])?)?;
    let (headers, annotation) = content.split_once("\n\n").unwrap_or((&content, ""));
    require(
        headers.lines().any(|l| l == format!("object {target}"))
            && headers.lines().any(|l| l == "type commit"),
        format!("tag target conflict: {}", tag.name),
    )?;
    require(
        headers.lines().any(|l| l == format!("tag {}", tag.name)),
        format!("tag name conflict: {}", tag.name),
    )?;
    let signature = [
        "-----BEGIN PGP SIGNATURE-----",
        "-----BEGIN SSH SIGNATURE-----",
        "-----BEGIN SIGNED MESSAGE-----",
    ]
    .iter()
    .filter_map(|m| annotation.find(m))
    .min();
    let payload = signature.map_or(annotation, |i| &annotation[..i]);
    require(
        payload.strip_suffix('\n').unwrap_or(payload) == tag.version,
        format!("tag annotation conflict: {}", tag.name),
    )?;
    require(
        !sign || signature.is_some(),
        format!("{} is unsigned", tag.name),
    )?;
    tag.signature = if signature.is_some() {
        let verification = git::output(root, &["verify-tag", oid])?;
        if verification.status.success() {
            "verified"
        } else {
            diagnostics.push(format!(
                "signature trust not established for {}: {}",
                tag.name,
                String::from_utf8_lossy(&verification.stderr).trim()
            ));
            "unverified"
        }
    } else {
        "unsigned"
    };
    Ok(())
}

pub(super) fn preflight(root: &Path, plan: &mut TagPlan, sign: bool) -> Result<()> {
    for tag in &mut plan.tags {
        if let Some(oid) = git::ref_oid(root, &format!("refs/tags/{}", tag.name))? {
            tag.local = "conflict";
            validate(
                root,
                &plan.target_oid,
                tag,
                &oid,
                sign,
                &mut plan.diagnostics,
            )?;
            tag.local = "existing";
            tag.oid = Some(oid);
            tag.action = "none";
        }
    }
    Ok(())
}

pub(super) fn apply(root: &Path, plan: &mut TagPlan, sign: bool) -> Result<()> {
    for tag in &mut plan.tags {
        if tag.local == "existing" {
            continue;
        }
        let reference = format!("refs/tags/{}", tag.name);
        if let Some(oid) = &tag.oid {
            git::text(root, &["update-ref", &reference, oid, ""])?;
            tag.local = "adopted";
        } else {
            git::text(
                root,
                &[
                    "-c",
                    "tag.gpgSign=false",
                    "tag",
                    if sign { "-s" } else { "-a" },
                    "-m",
                    &tag.version,
                    "--",
                    &tag.name,
                    &plan.target_oid,
                ],
            )?;
            tag.local = "created";
            tag.oid = Some(git::ref_oid(root, &reference)?.ok_or_else(|| {
                anyhow::anyhow!("new tag disappeared before verification: {}", tag.name)
            })?);
        }
        if let Some(oid) = tag.oid.clone() {
            validate(
                root,
                &plan.target_oid,
                tag,
                &oid,
                sign,
                &mut plan.diagnostics,
            )?;
        }
    }
    Ok(())
}
