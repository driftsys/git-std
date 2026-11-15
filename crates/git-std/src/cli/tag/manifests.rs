use super::types::{refuse, require};
use anyhow::Result;
use std::{collections::BTreeSet, path::Path};

/// Validate structural fields before tolerant legacy version discovery.
pub(super) fn validate(root: &Path, expected: &str) -> Result<BTreeSet<String>> {
    let mut observed = BTreeSet::new();
    for name in ["Cargo.toml", "project.toml", "pyproject.toml"] {
        let path = root.join(name);
        if !path.exists() {
            continue;
        }
        let content = std::fs::read_to_string(path)?;
        let document = content
            .parse::<toml::Table>()
            .map_err(|e| refuse(format!("invalid {name}: {e}")))?;
        let fields: Vec<_> = match name {
            "Cargo.toml" => vec![
                document.get("package").and_then(|v| v.get("version")),
                document
                    .get("workspace")
                    .and_then(|v| v.get("package"))
                    .and_then(|v| v.get("version")),
            ],
            "pyproject.toml" => vec![document.get("project").and_then(|v| v.get("version"))],
            _ => vec![document.get("version")],
        };
        for field in fields.into_iter().flatten() {
            if name == "Cargo.toml"
                && field.get("workspace").and_then(toml::Value::as_bool) == Some(true)
            {
                continue;
            }
            let version = field
                .as_str()
                .ok_or_else(|| refuse(format!("version in {name} must be a string")))?;
            require(
                version == expected,
                format!("version mismatch in {name}: expected {expected}, found {version}"),
            )?;
            observed.insert(name.into());
        }
    }
    for name in ["package.json", "project.json", "deno.json"] {
        let path = root.join(name);
        if !path.exists() {
            continue;
        }
        let document: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)
            .map_err(|e| refuse(format!("invalid {name}: {e}")))?;
        require(
            document.is_object(),
            format!("{name} must contain an object"),
        )?;
        if let Some(field) = document.get("version") {
            let version = field
                .as_str()
                .ok_or_else(|| refuse(format!("version in {name} must be a string")))?;
            require(
                version == expected,
                format!("version mismatch in {name}: expected {expected}, found {version}"),
            )?;
            observed.insert(name.into());
        }
    }
    Ok(observed)
}
