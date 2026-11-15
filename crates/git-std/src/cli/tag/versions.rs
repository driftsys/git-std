use super::{
    snapshot::safe_path,
    types::{refuse, require},
};
use crate::config::{ProjectConfig, Scheme, VersionFileConfig};
use anyhow::Result;
use std::path::Path;

pub(super) fn validate_version(
    config: &ProjectConfig,
    scheme: &Scheme,
    version: &str,
) -> Result<()> {
    let valid = if *scheme == Scheme::Calver {
        standard_version::calver::parse::version_matches(&config.versioning.calver_format, version)
            .map_err(|e| refuse(e.to_string()))?
    } else {
        semver::Version::parse(version).is_ok()
    };
    require(valid, format!("invalid release version: {version}"))
}

pub(super) fn check(
    root: &Path,
    workspace: &Path,
    expected: &str,
    configs: &[VersionFileConfig],
) -> Result<String> {
    let mut custom = Vec::new();
    for config in configs {
        safe_path(&config.path)?;
        let resolved =
            crate::config::resolve_custom_version_files(root, std::slice::from_ref(config));
        require(
            !resolved.is_empty(),
            format!("missing version source: {}", config.path),
        )?;
        for source in resolved {
            require(
                root.join(&source.path).is_file(),
                format!("missing version source: {}", source.path.display()),
            )?;
            let engine = standard_version::RegexVersionFile::new(&source)
                .map_err(|e| refuse(e.to_string()))?;
            let content = std::fs::read_to_string(root.join(&source.path))?;
            let value = engine.read_version(&content).ok_or_else(|| {
                refuse(format!(
                    "unreadable configured version: {}",
                    source.path.display()
                ))
            })?;
            require(
                value == expected,
                format!(
                    "configured version mismatch in {}: expected {expected}, found {value}",
                    source.path.display()
                ),
            )?;
            custom.push(source);
        }
    }
    let structural = super::manifests::validate(root, expected)?;
    let observed =
        standard_version::detect_version_files(root, &custom).map_err(|e| refuse(e.to_string()))?;
    for file in &observed {
        if structural.contains(&file.name) {
            continue;
        }
        let version = if file.name == "pubspec.yaml" {
            file.old_version
                .split('+')
                .next()
                .unwrap_or(&file.old_version)
        } else {
            &file.old_version
        };
        require(
            version == expected,
            format!(
                "version mismatch in {}: expected {expected}, found {}",
                file.path.display(),
                file.old_version
            ),
        )?;
    }
    let inherited = inherited_cargo_version(root, workspace)?;
    if let Some(version) = &inherited {
        require(
            version == expected,
            format!("inherited Cargo version mismatch: expected {expected}, found {version}"),
        )?;
    }
    Ok(
        if observed.is_empty() && inherited.is_none() && structural.is_empty() {
            "release_subject"
        } else {
            "manifest"
        }
        .into(),
    )
}

fn inherited_cargo_version(root: &Path, workspace: &Path) -> Result<Option<String>> {
    let path = root.join("Cargo.toml");
    if !path.exists() {
        return Ok(None);
    }
    let manifest = std::fs::read_to_string(path)?
        .parse::<toml::Table>()
        .map_err(|e| refuse(e.to_string()))?;
    let version = manifest.get("package").and_then(|p| p.get("version"));
    if version
        .and_then(|v| v.get("workspace"))
        .and_then(toml::Value::as_bool)
        != Some(true)
    {
        return Ok(None);
    }
    let manifest = std::fs::read_to_string(workspace.join("Cargo.toml"))
        .map_err(|e| refuse(e.to_string()))?
        .parse::<toml::Table>()
        .map_err(|e| refuse(e.to_string()))?;
    let version = manifest
        .get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("version"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| refuse("missing inherited Cargo workspace version"))?;
    Ok(Some(version.into()))
}
