//! Rust ecosystem — Cargo.

use std::path::Path;
use std::process::Command;

use standard_version::{CargoVersionFile, UpdateResult, VersionFile};

use super::{Ecosystem, SyncOutcome, WriteOutcome, cmd, native_write, try_sync};
use crate::ui;

pub struct Rust;

/// Use `git diff --name-only` to discover all files modified by the CLI tool.
fn git_modified_files(root: &Path) -> Vec<std::path::PathBuf> {
    let output = Command::new("git")
        .args(["diff", "--name-only"])
        .current_dir(root)
        .output();

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| root.join(l))
            .collect(),
        _ => vec![root.join("Cargo.toml")],
    }
}

/// Native workspace-aware version writer.
///
/// When `cargo set-version` is unavailable, this function handles both:
/// - Single-crate manifests: updates `[package] version`.
/// - Workspace manifests: updates `[workspace.package] version` (if present)
///   and each member crate that carries a pinned (non-inherited) version.
///
/// Members that use `version.workspace = true` are silently skipped — their
/// version is inherited from `[workspace.package]` and must not be
/// independently overridden.
fn workspace_native_write(root: &Path, new_version: &str) -> WriteOutcome {
    let root_cargo = root.join("Cargo.toml");

    let root_content = match std::fs::read_to_string(&root_cargo) {
        Ok(c) => c,
        Err(_) => return WriteOutcome::NotDetected,
    };

    // Parse to discover workspace members. Fall back to single-crate path on
    // any parse error.
    let parsed: toml::Value = match toml::from_str(&root_content) {
        Ok(v) => v,
        Err(_) => return native_write(root, &CargoVersionFile, new_version),
    };

    let members: Vec<String> = parsed
        .get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(|m| m.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    if members.is_empty() {
        // Single crate — update root manifest only.
        return native_write(root, &CargoVersionFile, new_version);
    }

    let mut results: Vec<UpdateResult> = Vec::new();

    // Update root manifest ([workspace.package] or [package] if present).
    if let WriteOutcome::Fallback { results: r } =
        native_write(root, &CargoVersionFile, new_version)
    {
        results.extend(r);
    }

    // Update each workspace member that carries a pinned version.
    for pattern in &members {
        let glob_pat = root.join(pattern).join("Cargo.toml");
        let entries = match glob::glob(&glob_pat.to_string_lossy()) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            update_member_crate(&entry, new_version, &mut results);
        }
    }

    if results.is_empty() {
        WriteOutcome::NotDetected
    } else {
        WriteOutcome::Fallback { results }
    }
}

/// Try to update a single member crate's `Cargo.toml`.
///
/// Silently skips the file if it has no pinned `version` field (e.g. uses
/// `version.workspace = true`). Emits a warning only on I/O errors.
fn update_member_crate(path: &Path, new_version: &str, results: &mut Vec<UpdateResult>) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            ui::warning(&format!("{}: {e}", path.display()));
            return;
        }
    };

    // read_version returns None when there is no pinned version field
    // (including version.workspace = true after the toml_helpers fix).
    let old_version = match CargoVersionFile.read_version(&content) {
        Some(v) => v,
        None => return,
    };

    let updated = match CargoVersionFile.write_version(&content, new_version) {
        Ok(u) => u,
        Err(_) => return,
    };

    let new_ver = CargoVersionFile
        .read_version(&updated)
        .unwrap_or_else(|| new_version.to_string());

    if std::fs::write(path, &updated).is_err() {
        ui::warning(&format!("{}: failed to write", path.display()));
        return;
    }

    results.push(UpdateResult {
        path: path.to_path_buf(),
        name: CargoVersionFile.name().to_string(),
        old_version,
        new_version: new_ver,
        extra: None,
    });
}

impl Ecosystem for Rust {
    fn name(&self) -> &'static str {
        "rust"
    }

    fn detect(&self, root: &Path) -> bool {
        root.join("Cargo.toml").exists()
    }

    fn version_files(&self) -> &[&str] {
        &["Cargo.toml"]
    }

    fn write_version(&self, root: &Path, new_version: &str) -> WriteOutcome {
        // Try `cargo set-version --workspace <V>` first (requires cargo-edit).
        match cmd::run_tool(root, "cargo", &["set-version", "--workspace", new_version]) {
            Ok(status) if status.success() => {
                // cargo set-version may modify multiple Cargo.toml files
                // in a workspace. Discover all modified files via git.
                WriteOutcome::CliModified {
                    files: git_modified_files(root),
                }
            }
            _ => workspace_native_write(root, new_version),
        }
    }

    fn sync_lock(&self, root: &Path) -> Vec<SyncOutcome> {
        vec![try_sync(
            root,
            "Cargo.lock",
            "cargo",
            &["update", "--workspace"],
        )]
    }

    fn lock_files(&self) -> &[&str] {
        &["Cargo.lock"]
    }

    fn version_file_engine(&self) -> Option<Box<dyn VersionFile>> {
        Some(Box::new(CargoVersionFile))
    }
}
