use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};
use standard_version::VersionFile;

use crate::config::ProjectConfig;
use crate::git;

use super::BumpOptions;

mod tool_resolution;

/// Fidelity assigned to a declared bump effect.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Fidelity {
    /// Before and after bytes are determined by git-std.
    Exact,
    /// The effect is deterministic but its identifier exists only after apply.
    Runtime,
    /// The effect depends on an external program or remote system.
    Conditional,
    /// The effect can run arbitrary user code and cannot be predicted.
    Opaque,
}

/// A declared effect of applying a bump.
#[derive(Clone, Debug, Serialize)]
pub struct PlannedEffect {
    /// Stable effect category.
    pub kind: &'static str,
    /// Repository path, reference, hook, or remote affected.
    pub target: String,
    /// Strength of the effect prediction.
    pub fidelity: Fidelity,
    /// Digest of current bytes, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_sha256: Option<String>,
    /// Digest of predicted bytes, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_sha256: Option<String>,
    /// Explanation for non-exact effects.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Digest of one input that contributes to a plan ID.
#[derive(Clone, Debug, Serialize)]
pub struct InputDigest {
    /// Repository-relative path or logical input name.
    pub path: String,
    /// SHA-256 digest of the input bytes.
    pub sha256: String,
}

/// All declared inputs bound by a bump plan ID.
#[derive(Clone, Debug, Serialize)]
pub struct BumpInputs {
    /// Commit at which the plan was made.
    pub head: String,
    /// Current branch, or `HEAD` when detached.
    pub branch: String,
    /// Existing tag names and targets.
    pub tags_sha256: String,
    /// Digest of configured remote names, URLs, and directions.
    pub remotes_sha256: String,
    /// Calendar day used by calver/changelog generation.
    pub effective_date: String,
    /// Digest of effective serialized configuration.
    pub config_sha256: String,
    /// Digest of PATH because ecosystem-tool selection is PATH-dependent.
    pub path_sha256: String,
    /// Executable paths selected from the current PATH.
    pub tools: Vec<ToolResolution>,
    /// Normalized behavior-affecting options.
    pub options: serde_json::Value,
    /// Version files, lock files, config bytes, and lifecycle hooks.
    pub files: Vec<InputDigest>,
}

/// Resolution of an external ecosystem tool relevant to bump behavior.
#[derive(Clone, Debug, Serialize)]
pub struct ToolResolution {
    /// Executable name.
    pub name: &'static str,
    /// Selected path, or null when unavailable.
    pub path: Option<String>,
}

/// Summary of whether every declared effect is predictable.
#[derive(Clone, Debug, Serialize)]
pub struct FidelitySummary {
    /// True only when no conditional or opaque effect is present.
    pub exact: bool,
    /// Explicit reasons exact fidelity is unavailable.
    pub limitations: Vec<String>,
}

/// Deterministic dry-run/apply contract.
#[derive(Clone, Debug, Serialize)]
pub struct BumpContract {
    /// SHA-256 identity of the canonical input and effect document.
    pub plan_id: String,
    /// Inputs covered by the identity.
    pub inputs: BumpInputs,
    /// Intended effects and their fidelity.
    pub effects: Vec<PlannedEffect>,
    /// Overall fidelity summary.
    pub fidelity: FidelitySummary,
}

/// Construct the contract for one single-version bump without mutation.
pub fn build_contract(
    root: &Path,
    config: &ProjectConfig,
    opts: &BumpOptions,
    new_version: &str,
    detected: &[standard_version::DetectedFile],
    lock_files: &[String],
    changelog_after: Option<&[u8]>,
) -> Result<BumpContract, String> {
    let head = git::head_oid(root).map_err(|error| error.to_string())?;
    let branch = git::current_branch(root).unwrap_or_else(|_| "HEAD".to_string());
    let tags = command_stdout(root, &["show-ref", "--tags"]);
    let remotes = command_stdout(root, &["remote", "--verbose"]);
    let config_bytes = serde_json::to_vec(config).map_err(|error| error.to_string())?;
    let options = normalized_options(opts, new_version);
    let mut files = relevant_inputs(root, detected, lock_files);
    files.sort_by(|left, right| left.path.cmp(&right.path));

    let inputs = BumpInputs {
        head,
        branch,
        tags_sha256: sha256(tags.as_bytes()),
        remotes_sha256: sha256(remotes.as_bytes()),
        effective_date: current_date(),
        config_sha256: sha256(&config_bytes),
        path_sha256: sha256(
            std::env::var_os("PATH")
                .unwrap_or_default()
                .as_encoded_bytes(),
        ),
        tools: tool_resolutions(),
        options,
        files,
    };
    let effects = planned_effects(
        root,
        config,
        opts,
        new_version,
        detected,
        lock_files,
        changelog_after,
    );
    let limitations: Vec<String> = effects
        .iter()
        .filter_map(|effect| match effect.fidelity {
            Fidelity::Conditional | Fidelity::Opaque => effect.reason.clone(),
            Fidelity::Exact | Fidelity::Runtime => None,
        })
        .collect();
    let fidelity = FidelitySummary {
        exact: limitations.is_empty(),
        limitations,
    };

    let identity = serde_json::to_vec(&(&inputs, &effects)).map_err(|error| error.to_string())?;
    Ok(BumpContract {
        plan_id: format!("sha256:{}", sha256(&identity)),
        inputs,
        effects,
        fidelity,
    })
}

fn normalized_options(opts: &BumpOptions, new_version: &str) -> serde_json::Value {
    serde_json::json!({
        "version": new_version,
        "prerelease": opts.prerelease,
        "release_as": opts.release_as,
        "first_release": opts.first_release,
        "first_major_release": opts.first_major_release,
        "no_tag": opts.no_tag,
        "no_commit": opts.no_commit,
        "skip_changelog": opts.skip_changelog,
        "sign": opts.sign,
        "force": opts.force,
        "stable": opts.stable,
        "minor": opts.minor,
        "packages": opts.packages,
        "push": opts.push,
        "skip_hooks": hooks_are_skipped(),
    })
}

fn hooks_are_skipped() -> bool {
    std::env::var("GIT_STD_SKIP_HOOKS")
        .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}

fn relevant_inputs(
    root: &Path,
    detected: &[standard_version::DetectedFile],
    lock_files: &[String],
) -> Vec<InputDigest> {
    let mut paths: Vec<PathBuf> = detected.iter().map(|file| file.path.clone()).collect();
    paths.extend(lock_files.iter().map(|file| root.join(file)));
    paths.push(root.join(".git-std.toml"));
    for hook in ["pre-bump", "post-version", "post-changelog", "post-bump"] {
        paths.push(root.join(".githooks").join(format!("{hook}.hooks")));
    }
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .map(|path| {
            let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy();
            let bytes = std::fs::read(&path).unwrap_or_default();
            InputDigest {
                path: relative.into_owned(),
                sha256: sha256(&bytes),
            }
        })
        .collect()
}

fn planned_effects(
    root: &Path,
    config: &ProjectConfig,
    opts: &BumpOptions,
    new_version: &str,
    detected: &[standard_version::DetectedFile],
    lock_files: &[String],
    changelog_after: Option<&[u8]>,
) -> Vec<PlannedEffect> {
    let mut effects: Vec<PlannedEffect> = detected
        .iter()
        .map(|file| {
            let target = file
                .path
                .strip_prefix(root)
                .unwrap_or(&file.path)
                .to_string_lossy()
                .into_owned();
            let uses_external_tool = target == "package.json" || target == "pyproject.toml";
            let before = std::fs::read(&file.path).ok();
            let after_sha256 = if uses_external_tool {
                None
            } else {
                before
                    .as_deref()
                    .and_then(|content| preview_version_file(&target, content, new_version))
                    .map(|content| sha256(content.as_bytes()))
            };
            let is_exact = !uses_external_tool && after_sha256.is_some();
            PlannedEffect {
                kind: "version_file",
                target,
                fidelity: if is_exact {
                    Fidelity::Exact
                } else {
                    Fidelity::Conditional
                },
                before_sha256: before.map(|bytes| sha256(&bytes)),
                after_sha256,
                reason: (!is_exact).then(|| {
                    if uses_external_tool {
                        "ecosystem CLI availability may change the written bytes".to_string()
                    } else {
                        "custom version-file replacement cannot be predicted without its matcher"
                            .to_string()
                    }
                }),
            }
        })
        .collect();

    effects.extend(lock_files.iter().map(|file| {
        PlannedEffect {
            kind: "lock_sync",
            target: file.clone(),
            fidelity: Fidelity::Conditional,
            before_sha256: std::fs::read(root.join(file))
                .ok()
                .map(|bytes| sha256(&bytes)),
            after_sha256: None,
            reason: Some("lock bytes are produced by an external ecosystem tool".to_string()),
        }
    }));
    if let Some(changelog_after) = changelog_after {
        effects.push(PlannedEffect {
            kind: "changelog",
            target: "CHANGELOG.md".to_string(),
            fidelity: Fidelity::Exact,
            before_sha256: std::fs::read(root.join("CHANGELOG.md"))
                .ok()
                .map(|bytes| sha256(&bytes)),
            after_sha256: Some(sha256(changelog_after)),
            reason: None,
        });
    }
    if !hooks_are_skipped() {
        let mut hooks = vec!["pre-bump", "post-version"];
        if changelog_after.is_some() {
            hooks.push("post-changelog");
        }
        if !opts.no_commit {
            hooks.push("post-bump");
        }
        for hook in hooks {
            if root
                .join(".githooks")
                .join(format!("{hook}.hooks"))
                .exists()
            {
                effects.push(PlannedEffect {
                    kind: "hook",
                    target: hook.to_string(),
                    fidelity: Fidelity::Opaque,
                    before_sha256: None,
                    after_sha256: None,
                    reason: Some(
                        "lifecycle hooks may perform arbitrary external effects".to_string(),
                    ),
                });
            }
        }
    }
    if !opts.no_commit {
        effects.push(PlannedEffect {
            kind: "commit",
            target: format!("chore(release): {new_version}"),
            fidelity: if opts.sign {
                Fidelity::Conditional
            } else {
                Fidelity::Runtime
            },
            before_sha256: None,
            after_sha256: None,
            reason: Some(if opts.sign {
                "commit signing depends on the configured key, signing agent, and produced signature"
                    .to_string()
            } else {
                "commit OID is observed after apply".to_string()
            }),
        });
    }
    if !opts.no_commit && !opts.no_tag {
        effects.push(PlannedEffect {
            kind: "tag",
            target: format!("{}{new_version}", config.versioning.tag_prefix),
            fidelity: if opts.sign {
                Fidelity::Conditional
            } else {
                Fidelity::Runtime
            },
            before_sha256: None,
            after_sha256: None,
            reason: Some(if opts.sign {
                "tag signing depends on the configured key, signing agent, and produced signature"
                    .to_string()
            } else {
                "annotated tag OID is observed after apply".to_string()
            }),
        });
    }
    if !opts.no_commit
        && !opts.no_tag
        && let Some(remote) = &opts.push
    {
        effects.push(PlannedEffect {
            kind: "push",
            target: remote.clone(),
            fidelity: Fidelity::Conditional,
            before_sha256: None,
            after_sha256: None,
            reason: Some("remote state and network effects cannot be predicted".to_string()),
        });
    }
    effects
}

fn preview_version_file(target: &str, bytes: &[u8], new_version: &str) -> Option<String> {
    let content = std::str::from_utf8(bytes).ok()?;
    if target == "Cargo.toml" {
        return crate::ecosystem::preview_workspace_manifest(content, new_version);
    }
    let engine: Box<dyn VersionFile> = match target {
        _ if target.ends_with("/Cargo.toml") => Box::new(standard_version::CargoVersionFile),
        "deno.json" | "deno.jsonc" => Box::new(standard_version::DenoVersionFile),
        "gradle.properties" => Box::new(standard_version::GradleVersionFile),
        "package.json" => Box::new(standard_version::JsonVersionFile),
        "project.json" => Box::new(standard_version::ProjectJsonVersionFile),
        "project.toml" => Box::new(standard_version::ProjectTomlVersionFile),
        "project.yaml" => Box::new(standard_version::ProjectYamlVersionFile),
        "pubspec.yaml" => Box::new(standard_version::PubspecVersionFile),
        "pyproject.toml" => Box::new(standard_version::PyprojectVersionFile),
        "VERSION" => Box::new(standard_version::PlainVersionFile),
        _ => return None,
    };
    engine.write_version(content, new_version).ok()
}

fn tool_resolutions() -> Vec<ToolResolution> {
    [
        "cargo", "deno", "flutter", "gradle", "npm", "pnpm", "poetry", "uv", "yarn",
    ]
    .into_iter()
    .map(|name| ToolResolution {
        name,
        path: resolve_tool(name),
    })
    .collect()
}

fn resolve_tool(name: &str) -> Option<String> {
    tool_resolution::resolve(name)
}

fn command_stdout(root: &Path, args: &[&str]) -> String {
    std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

fn current_date() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    standard_changelog::format_date(seconds)
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
