use super::{
    configuration,
    types::{refuse, require},
};
use crate::{
    config::{ProjectConfig, VersionFileConfig},
    git::release_refs as git,
};
use anyhow::{Context, Result};
use std::{
    collections::BTreeSet,
    path::{Component, Path},
};

pub(super) const MANIFESTS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "deno.json",
    "deno.jsonc",
    "pyproject.toml",
    "pubspec.yaml",
    "gradle.properties",
    "project.toml",
    "project.json",
    "project.yaml",
    "project.yml",
    "VERSION",
];

pub(super) fn safe_path(path: &str) -> Result<()> {
    require(
        !path.is_empty()
            && !path.contains('\\')
            && Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        format!("unsafe release source path: {path}"),
    )
}

pub(super) fn inspect(
    root: &Path,
    oid: &str,
    diagnostics: &mut Vec<String>,
) -> Result<(tempfile::TempDir, ProjectConfig)> {
    let listing = git::bytes(root, &["ls-tree", "-rz", oid])?;
    let mut entries = Vec::new();
    for entry in listing.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let text = std::str::from_utf8(entry).map_err(|_| refuse("non-UTF-8 release tree path"))?;
        let (metadata, path) = text.split_once('\t').context("invalid Git tree entry")?;
        let fields: Vec<_> = metadata.split_whitespace().collect();
        entries.push((fields[0], fields[2], path));
    }
    let config_source =
        if let Some((mode, blob, _)) = entries.iter().find(|e| e.2 == ".git-std.toml") {
            require(
                *mode == "100644" || *mode == "100755",
                "configuration must be a regular file",
            )?;
            String::from_utf8(git::bytes(root, &["cat-file", "blob", blob])?)?
        } else {
            String::new()
        };
    let config = configuration::parse(&config_source, diagnostics)?;
    let mut custom = Vec::new();
    patterns("", &config.version_files, &mut custom)?;
    for package in &config.packages {
        safe_path(&package.path)?;
        patterns(
            &package.path,
            package.version_files.as_deref().unwrap_or(&[]),
            &mut custom,
        )?;
    }
    let dir = tempfile::tempdir().context("cannot create release snapshot")?;
    let mut names = BTreeSet::new();
    for (mode, blob, path) in entries {
        let filename = Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if path != ".git-std.toml"
            && !MANIFESTS.contains(&filename)
            && !custom.iter().any(|p| p.matches(path))
        {
            continue;
        }
        safe_path(path)?;
        require(
            mode == "100644" || mode == "100755",
            format!("release source is not a regular file: {path}"),
        )?;
        // Include directory prefixes: case-colliding directories also lose identity.
        let parts: Vec<_> = path.split('/').collect();
        for i in 1..=parts.len() {
            let name = parts[..i].join("/");
            let folded = name.to_lowercase();
            if let Some((_, original)) = names.iter().find(|(key, _)| key == &folded) {
                require(
                    original == &name,
                    format!("case-colliding release source: {path}"),
                )?;
            } else {
                names.insert((folded, name));
            }
        }
        let content = git::bytes(root, &["cat-file", "blob", blob])?;
        require(
            !content.starts_with(b"version https://git-lfs.github.com/spec/v1"),
            format!("LFS pointer cannot supply release version: {path}"),
        )?;
        let target = dir.path().join(path);
        std::fs::create_dir_all(target.parent().context("missing source parent")?)?;
        std::fs::write(target, content)?;
    }
    validate_workspace_paths(dir.path())?;
    Ok((dir, config))
}

fn patterns(
    prefix: &str,
    configs: &[VersionFileConfig],
    patterns: &mut Vec<glob::Pattern>,
) -> Result<()> {
    for config in configs {
        safe_path(&config.path)?;
        for expanded in crate::config::expand_braces(&config.path) {
            safe_path(&expanded)?;
            let path = if prefix.is_empty() {
                expanded
            } else {
                format!("{prefix}/{expanded}")
            };
            patterns.push(
                glob::Pattern::new(&path)
                    .map_err(|e| refuse(format!("invalid version source glob: {e}")))?,
            );
        }
    }
    Ok(())
}

// Validate discovery inputs before existing workspace discovery can follow them.
fn validate_workspace_paths(root: &Path) -> Result<()> {
    let cargo = root.join("Cargo.toml");
    if cargo.exists() {
        let manifest = std::fs::read_to_string(cargo)?
            .parse::<toml::Table>()
            .map_err(|e| refuse(format!("invalid Cargo.toml: {e}")))?;
        if let Some(members) = manifest
            .get("workspace")
            .and_then(|w| w.get("members"))
            .and_then(toml::Value::as_array)
        {
            for member in members {
                safe_path(
                    member
                        .as_str()
                        .ok_or_else(|| refuse("workspace member must be a path"))?,
                )?;
            }
        }
    }
    for name in ["package.json", "deno.json", "deno.jsonc"] {
        let path = root.join(name);
        if !path.exists() {
            continue;
        }
        // Match the discovery parser: JSONC that it cannot parse is not followed.
        let Ok(manifest) =
            serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(path)?)
        else {
            continue;
        };
        let members = manifest.get(if name == "package.json" {
            "workspaces"
        } else {
            "workspace"
        });
        if let Some(members) = members.and_then(|m| {
            m.as_array()
                .or_else(|| m.get("members").and_then(serde_json::Value::as_array))
        }) {
            for member in members {
                safe_path(
                    member
                        .as_str()
                        .ok_or_else(|| refuse("workspace member must be a path"))?,
                )?;
            }
        }
    }
    Ok(())
}
