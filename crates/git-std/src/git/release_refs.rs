//! Git operations for immutable release inspection and explicit tag refs.
use anyhow::{Context, Result, bail};
use std::{
    path::Path,
    process::{Command, Output},
};

pub(crate) fn output(root: &Path, args: &[&str]) -> Result<Output> {
    Command::new("git")
        .current_dir(root)
        .args([
            "--no-lazy-fetch",
            "--no-replace-objects",
            "-c",
            "core.fsmonitor=false",
        ])
        .args(args)
        .output()
        .context("cannot run Git")
}

pub(crate) fn bytes(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = output(root, args)?;
    if !out.status.success() {
        bail!(
            "git {}: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(out.stdout)
}

pub(crate) fn text(root: &Path, args: &[&str]) -> Result<String> {
    Ok(String::from_utf8(bytes(root, args)?)
        .context("Git returned non-UTF-8 text")?
        .trim_end()
        .into())
}

pub(crate) fn optional(root: &Path, args: &[&str]) -> Result<Option<String>> {
    let out = output(root, args)?;
    match out.status.code() {
        Some(0) => Ok(Some(String::from_utf8(out.stdout)?.trim_end().into())),
        Some(1) => Ok(None),
        _ => bail!(
            "Git query failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ),
    }
}

pub(crate) fn ancestor(root: &Path, oid: &str, branch: &str) -> Result<bool> {
    Ok(optional(root, &["merge-base", "--is-ancestor", oid, branch])?.is_some())
}

pub(crate) fn ref_oid(root: &Path, reference: &str) -> Result<Option<String>> {
    let refs = text(
        root,
        &[
            "for-each-ref",
            "--format=%(objectname) %(refname)",
            reference,
        ],
    )?;
    Ok(refs.lines().find_map(|line| {
        let (oid, name) = line.split_once(' ')?;
        (name == reference).then(|| oid.into())
    }))
}
