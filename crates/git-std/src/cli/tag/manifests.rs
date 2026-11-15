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
    for name in ["package.json", "project.json", "deno.json", "deno.jsonc"] {
        let path = root.join(name);
        if !path.exists() {
            continue;
        }
        let content = std::fs::read_to_string(path)?;
        let document: serde_json::Value = if name == "package.json" {
            serde_json::from_str(&content).map_err(|e| refuse(format!("invalid {name}: {e}")))?
        } else {
            jsonc_parser::parse_to_serde_value(
                &content,
                &jsonc_parser::ParseOptions {
                    allow_comments: true,
                    allow_trailing_commas: true,
                    allow_loose_object_property_names: false,
                    allow_missing_commas: false,
                    allow_single_quoted_strings: false,
                    allow_hexadecimal_numbers: false,
                    allow_unary_plus_numbers: false,
                    allow_bare_decimal_point_numbers: false,
                    allow_non_finite_numbers: false,
                    allow_extended_string_escapes: false,
                },
            )
            .map_err(|e| refuse(format!("invalid {name}: {e}")))?
        };
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
    for name in ["pubspec.yaml", "project.yaml", "project.yml"] {
        let path = root.join(name);
        if !path.exists() {
            continue;
        }
        let documents = yaml_rust2::YamlLoader::load_from_str(&std::fs::read_to_string(path)?)
            .map_err(|e| refuse(format!("invalid {name}: {e}")))?;
        require(
            documents.len() == 1,
            format!("{name} must contain one document"),
        )?;
        let document = documents[0]
            .as_hash()
            .ok_or_else(|| refuse(format!("{name} must contain a mapping")))?;
        if let Some(field) = document.get(&yaml_rust2::Yaml::String("version".into())) {
            let version = field
                .as_str()
                .ok_or_else(|| refuse(format!("version in {name} must be a string")))?;
            let version = if name == "pubspec.yaml" {
                version.split('+').next().unwrap_or(version)
            } else {
                version
            };
            require(
                version == expected,
                format!("version mismatch in {name}: expected {expected}, found {version}"),
            )?;
            observed.insert(name.into());
        }
    }
    Ok(observed)
}
