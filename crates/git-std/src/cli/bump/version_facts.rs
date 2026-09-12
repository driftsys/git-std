use std::path::Path;

use serde::Serialize;

/// A version value observed in a project manifest.
#[derive(Clone, Debug, Serialize)]
pub struct VersionObservation {
    /// Repository-relative manifest path.
    pub path: String,
    /// Version resolved from the manifest.
    pub version: String,
    /// Detector or inheritance source.
    pub source: String,
}

/// A manifest whose version differs from the canonical source version.
#[derive(Clone, Debug, Serialize)]
pub struct VersionMismatch {
    /// Repository-relative manifest path.
    pub path: String,
    /// Version found in the manifest.
    pub observed: String,
    /// Version selected from the release tag lineage.
    pub canonical: String,
}

/// Collected single-version observations and disagreements.
pub struct VersionFacts {
    /// All readable version sources in deterministic path order.
    pub observations: Vec<VersionObservation>,
    /// Sources that do not equal the canonical version.
    pub mismatches: Vec<VersionMismatch>,
}

/// Collect version sources used by the single-version bump contract.
pub fn collect_version_facts(
    root: &Path,
    canonical: &str,
    detected: &[standard_version::DetectedFile],
) -> VersionFacts {
    let mut observations: Vec<VersionObservation> = detected
        .iter()
        .filter(|file| !file.old_version.is_empty())
        .map(|file| VersionObservation {
            path: relative(root, &file.path),
            version: file.old_version.clone(),
            source: file.name.clone(),
        })
        .collect();
    collect_cargo_members(root, &mut observations);
    observations.sort_by(|left, right| left.path.cmp(&right.path));
    observations.dedup_by(|left, right| left.path == right.path);

    let mismatches = observations
        .iter()
        .filter(|observation| observation.version != canonical)
        .map(|observation| VersionMismatch {
            path: observation.path.clone(),
            observed: observation.version.clone(),
            canonical: canonical.to_string(),
        })
        .collect();

    VersionFacts {
        observations,
        mismatches,
    }
}

fn collect_cargo_members(root: &Path, observations: &mut Vec<VersionObservation>) {
    let root_manifest = root.join("Cargo.toml");
    let Ok(content) = std::fs::read_to_string(&root_manifest) else {
        return;
    };
    let Ok(document) = toml::from_str::<toml::Value>(&content) else {
        return;
    };
    let workspace_version = document
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str);
    let Some(members) = document
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array)
    else {
        return;
    };

    for member in members.iter().filter_map(toml::Value::as_str) {
        let pattern = root.join(member).join("Cargo.toml");
        let Ok(entries) = glob::glob(&pattern.to_string_lossy()) else {
            continue;
        };
        for path in entries.flatten() {
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(document) = toml::from_str::<toml::Value>(&content) else {
                continue;
            };
            let package = document.get("package");
            let pinned = package
                .and_then(|value| value.get("version"))
                .and_then(toml::Value::as_str);
            let inherited = package
                .and_then(|value| value.get("version"))
                .and_then(|value| value.get("workspace"))
                .and_then(toml::Value::as_bool)
                .unwrap_or(false);
            let resolved = pinned.or_else(|| inherited.then_some(workspace_version).flatten());
            if let Some(version) = resolved {
                observations.push(VersionObservation {
                    path: relative(root, &path),
                    version: version.to_string(),
                    source: if inherited {
                        "cargo_workspace".to_string()
                    } else {
                        "cargo_member".to_string()
                    },
                });
            }
        }
    }
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}
