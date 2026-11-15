use super::{
    snapshot::safe_path,
    types::{refuse, require},
    versions,
};
use anyhow::Result;
use std::path::Path;

/// Validate each Cargo member without relying on tolerant package discovery.
pub(super) fn check(root: &Path, release_version: &str) -> Result<()> {
    let path = root.join("Cargo.toml");
    if !path.exists() {
        return Ok(());
    }
    let manifest = read_manifest(&path)?;
    let Some(workspace) = manifest.get("workspace") else {
        return Ok(());
    };
    let Some(members) = workspace.get("members") else {
        return check_fallback(root, release_version);
    };
    let members = members
        .as_array()
        .ok_or_else(|| refuse("Cargo workspace members must be an array"))?;
    if members.is_empty() {
        return check_fallback(root, release_version);
    }
    for member in members {
        let pattern = member
            .as_str()
            .ok_or_else(|| refuse("Cargo workspace member must be a path"))?;
        safe_path(pattern)?;
        let pattern = root.join(pattern).join("Cargo.toml");
        let matches = glob::glob(&pattern.to_string_lossy()).map_err(|e| refuse(e.to_string()))?;
        let mut found = false;
        for path in matches {
            let path = path.map_err(|e| refuse(e.to_string()))?;
            require(
                path.canonicalize()?.starts_with(root.canonicalize()?),
                "Cargo member escapes release snapshot",
            )?;
            let manifest = read_manifest(&path)?;
            let expected = independent_version(&manifest)?.unwrap_or(release_version);
            let directory = path
                .parent()
                .ok_or_else(|| refuse("Cargo member has no directory"))?;
            versions::check(directory, root, expected, &[])?;
            found = true;
        }
        require(found, "missing Cargo workspace member manifest")?;
    }
    Ok(())
}

// Preserve the established non-Cargo authority checks for hybrid workspaces.
fn check_fallback(root: &Path, release_version: &str) -> Result<()> {
    for member in crate::config::discover_packages(root) {
        safe_path(&member.path)?;
        versions::check(&root.join(member.path), root, release_version, &[])?;
    }
    Ok(())
}

fn read_manifest(path: &Path) -> Result<toml::Table> {
    std::fs::read_to_string(path)?
        .parse()
        .map_err(|e| refuse(format!("invalid Cargo manifest: {e}")))
}

fn independent_version(manifest: &toml::Table) -> Result<Option<&str>> {
    let Some(package) = manifest.get("package") else {
        return Ok(None);
    };
    if package.get("publish").and_then(toml::Value::as_bool) != Some(false) {
        return Ok(None);
    }
    let version = package.get("version").and_then(toml::Value::as_str);
    if let Some(version) = version {
        require(
            semver::Version::parse(version).is_ok(),
            "invalid independent Cargo member version",
        )?;
    }
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_explicitly_unpublished_pinned_versions_are_independent() {
        for (fields, expected) in [
            ("publish=false\nversion='0.3.0'", Some("0.3.0")),
            ("publish=false\nversion.workspace=true", None),
            ("publish=true\nversion='0.3.0'", None),
            ("version='0.3.0'", None),
            ("publish=[]\nversion='0.3.0'", None),
        ] {
            let manifest = format!("[package]\n{fields}").parse().unwrap();
            assert_eq!(independent_version(&manifest).unwrap(), expected);
        }
    }

    #[test]
    fn independent_version_must_be_valid_semver() {
        let manifest = "[package]\npublish=false\nversion='banana'"
            .parse()
            .unwrap();
        assert!(independent_version(&manifest).is_err());
    }
}
