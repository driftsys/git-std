//! Ecosystem abstraction for version bumping.
//!
//! Each supported ecosystem implements the [`Ecosystem`] trait, providing
//! version-file writing (CLI-first with native fallback) and lock-file
//! synchronisation (always delegated to the ecosystem tool).

pub mod cmd;
mod deno;
mod flutter;
mod gradle;
mod node;
mod plain;
mod python;
mod rust;

use std::fs;
use std::path::{Path, PathBuf};

use standard_version::{
    CustomVersionFile, DetectedFile, RegexVersionFile, UpdateResult, VersionFile,
};

use crate::ui;

// ---------------------------------------------------------------------------
// Trait and outcome types
// ---------------------------------------------------------------------------

/// An ecosystem that git-std knows how to manage during version bumps.
pub trait Ecosystem {
    /// Human-readable name (e.g. `"rust"`, `"node"`).
    fn name(&self) -> &'static str;

    /// Check whether this ecosystem is present at `root`.
    fn detect(&self, root: &Path) -> bool;

    /// Version file name(s) this ecosystem owns.
    fn version_files(&self) -> &[&str];

    /// Write `new_version` into this ecosystem's version file(s).
    ///
    /// Implementations should try an ecosystem CLI tool first if one exists,
    /// then fall back to native string manipulation via [`native_write`].
    fn write_version(&self, root: &Path, new_version: &str) -> WriteOutcome;

    /// Regenerate lock file(s) after a version change.
    fn sync_lock(&self, root: &Path) -> Vec<SyncOutcome>;

    /// Lock file name(s) this ecosystem manages (for dry-run display).
    fn lock_files(&self) -> &[&str] {
        &[]
    }

    /// Return the version-file engine used for read-only detection.
    ///
    /// Used by [`dry_run_version_files`] to detect existing versions without
    /// writing. Ecosystems that own a `standard_version::VersionFile` engine
    /// should return it here.
    fn version_file_engine(&self) -> Option<Box<dyn VersionFile>> {
        None
    }

    /// Whether this ecosystem is a last-resort fallback.
    ///
    /// Fallback ecosystems (e.g. `Plain`) are skipped when any specific
    /// ecosystem has already matched and written version files. This prevents
    /// a bare `VERSION` file from being updated in projects that already have
    /// a dedicated ecosystem (Node, Rust, etc.) managing their version.
    fn is_fallback(&self) -> bool {
        false
    }
}

/// Outcome of a version-write operation.
pub enum WriteOutcome {
    /// An ecosystem CLI tool wrote the version; these files were modified.
    CliModified { files: Vec<PathBuf> },
    /// Fell back to native string manipulation.
    Fallback { results: Vec<UpdateResult> },
    /// This ecosystem's version file was not present.
    NotDetected,
}

/// Outcome of a single lock-sync operation.
pub enum SyncOutcome {
    /// Lock file successfully regenerated.
    Synced { lock_file: String },
    /// Tool not on PATH — includes hint command for the user.
    ToolMissing {
        lock_file: String,
        tool: String,
        hint: String,
    },
    /// Tool ran but exited non-zero.
    Failed { lock_file: String, exit_code: i32 },
    /// No lock file exists for this ecosystem.
    NoLockFile,
}

/// Aggregate result of the ecosystem bump orchestration.
pub struct BumpResult {
    /// Version file update results (for display and JSON output).
    pub update_results: Vec<UpdateResult>,
    /// All file paths modified during version writing (for staging).
    pub modified_paths: Vec<PathBuf>,
    /// Lock file names that were successfully synced (for staging).
    pub synced_locks: Vec<String>,
}

// ---------------------------------------------------------------------------
// Native write helper
// ---------------------------------------------------------------------------

/// Write a version using a `standard_version::VersionFile` engine (string
/// manipulation). This is the fallback path used when no ecosystem CLI tool
/// is available.
pub fn native_write(root: &Path, engine: &dyn VersionFile, new_version: &str) -> WriteOutcome {
    let mut results = Vec::new();

    for filename in engine.filenames() {
        let path = root.join(filename);
        if !path.exists() {
            continue;
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                ui::warning(&format!("{}: {e}", path.display()));
                continue;
            }
        };

        if !engine.detect(&content) {
            continue;
        }

        let old_version = match engine.read_version(&content) {
            Some(v) => v,
            None => continue,
        };

        let updated = match engine.write_version(&content, new_version) {
            Ok(u) => u,
            Err(e) => {
                ui::warning(&format!("{}: {e}", path.display()));
                continue;
            }
        };

        let extra = engine.extra_info(&content, &updated);
        let actual_new_version = engine
            .read_version(&updated)
            .unwrap_or_else(|| new_version.to_string());

        if fs::write(&path, &updated).is_err() {
            ui::warning(&format!("{}: failed to write file", path.display()));
            continue;
        }

        results.push(UpdateResult {
            path,
            name: engine.name().to_string(),
            old_version,
            new_version: actual_new_version,
            extra,
        });
    }

    if results.is_empty() {
        WriteOutcome::NotDetected
    } else {
        WriteOutcome::Fallback { results }
    }
}

// ---------------------------------------------------------------------------
// Sync helper
// ---------------------------------------------------------------------------

/// Try to sync a single lock file using an ecosystem tool.
pub fn try_sync(root: &Path, lock_file: &str, tool: &str, args: &[&str]) -> SyncOutcome {
    if !root.join(lock_file).exists() {
        return SyncOutcome::NoLockFile;
    }

    let hint = format!("{tool} {}", args.join(" "));

    match cmd::run_tool(root, tool, args) {
        Err(_) => SyncOutcome::ToolMissing {
            lock_file: lock_file.to_string(),
            tool: tool.to_string(),
            hint,
        },
        Ok(status) if status.success() => SyncOutcome::Synced {
            lock_file: lock_file.to_string(),
        },
        Ok(status) => SyncOutcome::Failed {
            lock_file: lock_file.to_string(),
            exit_code: status.code().unwrap_or(-1),
        },
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Return all known ecosystems in detection priority order.
fn all_ecosystems() -> Vec<Box<dyn Ecosystem>> {
    vec![
        Box::new(rust::Rust),
        Box::new(node::Node),
        Box::new(deno::Deno),
        Box::new(python::Python),
        Box::new(flutter::Flutter),
        Box::new(gradle::Gradle),
        Box::new(plain::Plain),
    ]
}

// ---------------------------------------------------------------------------
// Orchestrator
// ---------------------------------------------------------------------------

/// Run all ecosystem operations for a version bump.
///
/// 1. Detect which ecosystems are present.
/// 2. Write versions (CLI-first, fallback to string manipulation).
/// 3. Sync lock files (always delegated to ecosystem tool).
/// 4. Process custom `[[version_files]]` via regex engine.
///
/// Fallback ecosystems (e.g. `Plain`) are skipped when a specific ecosystem
/// has already matched, preventing duplicate updates in mixed-ecosystem repos.
pub fn run_bump(root: &Path, new_version: &str, custom_files: &[CustomVersionFile]) -> BumpResult {
    let ecosystems = all_ecosystems();
    let mut update_results: Vec<UpdateResult> = Vec::new();
    let mut modified_paths: Vec<PathBuf> = Vec::new();
    let mut synced_locks: Vec<String> = Vec::new();
    let mut any_specific_updated = false;

    for eco in &ecosystems {
        if eco.is_fallback() && any_specific_updated {
            continue;
        }
        if !eco.detect(root) {
            continue;
        }

        // Write version.
        let version_updated = match eco.write_version(root, new_version) {
            WriteOutcome::CliModified { files } => {
                modified_paths.extend(files);
                true
            }
            WriteOutcome::Fallback { results } => {
                for r in &results {
                    modified_paths.push(r.path.clone());
                }
                let did_update = !results.is_empty();
                update_results.extend(results);
                did_update
            }
            WriteOutcome::NotDetected => false,
        };

        if version_updated && !eco.is_fallback() {
            any_specific_updated = true;
        }

        // Only sync lock files if version was actually updated.
        if !version_updated {
            continue;
        }

        for outcome in eco.sync_lock(root) {
            match outcome {
                SyncOutcome::Synced { lock_file } => {
                    ui::item("Synced:", &lock_file);
                    synced_locks.push(lock_file);
                }
                SyncOutcome::ToolMissing {
                    lock_file, hint, ..
                } => {
                    ui::warning(&format!("{lock_file} not synced \u{2014} run '{hint}'"));
                }
                SyncOutcome::Failed {
                    lock_file,
                    exit_code,
                } => {
                    ui::warning(&format!("{lock_file} sync failed (exit {exit_code})"));
                }
                SyncOutcome::NoLockFile => {}
            }
        }
    }

    let custom_count_before = update_results.len();

    // Process custom [[version_files]] via regex engine.
    process_custom_files(
        root,
        new_version,
        custom_files,
        &mut update_results,
        &mut modified_paths,
    );

    let custom_updated = update_results.len() > custom_count_before;

    // If custom files were updated but the ecosystem loop didn't trigger a
    // lock sync (e.g. the root Cargo.toml is a workspace manifest with no
    // [package] section), sync lock files now for any detected ecosystem.
    if custom_updated {
        for eco in &ecosystems {
            if !eco.detect(root) {
                continue;
            }
            // Skip ecosystems already synced above.
            if eco
                .lock_files()
                .iter()
                .any(|lf| synced_locks.contains(&lf.to_string()))
            {
                continue;
            }
            for outcome in eco.sync_lock(root) {
                match outcome {
                    SyncOutcome::Synced { lock_file } => {
                        ui::item("Synced:", &lock_file);
                        synced_locks.push(lock_file);
                    }
                    SyncOutcome::ToolMissing {
                        lock_file, hint, ..
                    } => {
                        ui::warning(&format!("{lock_file} not synced \u{2014} run '{hint}'"));
                    }
                    SyncOutcome::Failed {
                        lock_file,
                        exit_code,
                    } => {
                        ui::warning(&format!("{lock_file} sync failed (exit {exit_code})"));
                    }
                    SyncOutcome::NoLockFile => {}
                }
            }
        }
    }

    BumpResult {
        update_results,
        modified_paths,
        synced_locks,
    }
}

/// Emit dry-run lock sync messages. Only checks file existence — never
/// runs ecosystem tools.
pub fn dry_run_lock_sync(root: &Path) {
    let ecosystems = all_ecosystems();
    for eco in &ecosystems {
        if !eco.detect(root) {
            continue;
        }
        for lock_file in eco.lock_files() {
            if root.join(lock_file).exists() {
                ui::item("Would sync:", lock_file);
            }
        }
    }
}

/// Detect version files through the ecosystem layer (read-only).
///
/// Uses each ecosystem's `detect()` gate and `version_file_engine()` to
/// find version files, mirroring the detection that [`run_bump`] would
/// perform. Custom `[[version_files]]` are appended via the regex engine.
///
/// Fallback ecosystems are skipped when a specific ecosystem detected files,
/// consistent with the write-path behaviour in [`run_bump`].
pub fn dry_run_version_files(root: &Path, custom_files: &[CustomVersionFile]) -> Vec<DetectedFile> {
    let ecosystems = all_ecosystems();
    let mut results: Vec<DetectedFile> = Vec::new();
    let mut any_specific_detected = false;

    for eco in &ecosystems {
        if eco.is_fallback() && any_specific_detected {
            continue;
        }
        if !eco.detect(root) {
            continue;
        }
        let Some(engine) = eco.version_file_engine() else {
            continue;
        };
        let before = results.len();
        for filename in engine.filenames() {
            let path = root.join(filename);
            if !path.exists() {
                continue;
            }
            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            if !engine.detect(&content) {
                continue;
            }
            let Some(old_version) = engine.read_version(&content) else {
                continue;
            };
            results.push(DetectedFile {
                path,
                name: engine.name().to_string(),
                old_version,
            });
        }
        if !eco.is_fallback() && results.len() > before {
            any_specific_detected = true;
        }
    }

    // Append custom [[version_files]] via regex engine.
    for cf in custom_files {
        let engine = match RegexVersionFile::new(cf) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = root.join(engine.path());
        if !path.exists() {
            continue;
        }
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if !engine.detect(&content) {
            continue;
        }
        let Some(old_version) = engine.read_version(&content) else {
            continue;
        };
        results.push(DetectedFile {
            path,
            name: engine.name(),
            old_version,
        });
    }

    results
}

/// Return lock file names that would be synced during a real bump.
///
/// Only returns names for lock files that exist on disk, filtered through
/// the ecosystem detection layer.
pub fn dry_run_lock_file_names(root: &Path) -> Vec<String> {
    let ecosystems = all_ecosystems();
    let mut names = Vec::new();
    for eco in &ecosystems {
        if !eco.detect(root) {
            continue;
        }
        for lock_file in eco.lock_files() {
            if root.join(lock_file).exists() {
                names.push(lock_file.to_string());
            }
        }
    }
    names
}

// ---------------------------------------------------------------------------
// Custom version files
// ---------------------------------------------------------------------------

fn process_custom_files(
    root: &Path,
    new_version: &str,
    custom_files: &[CustomVersionFile],
    results: &mut Vec<UpdateResult>,
    paths: &mut Vec<PathBuf>,
) {
    for cf in custom_files {
        let engine = match RegexVersionFile::new(cf) {
            Ok(e) => e,
            Err(e) => {
                ui::warning(&format!("invalid custom version file regex: {e}"));
                continue;
            }
        };

        let path = root.join(engine.path());
        if !path.exists() {
            ui::warning(&format!("{}: file not found", path.display()));
            continue;
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                ui::warning(&format!("{}: {e}", path.display()));
                continue;
            }
        };

        if !engine.detect(&content) {
            ui::warning(&format!(
                "{}: pattern did not match file content",
                path.display()
            ));
            continue;
        }

        let old_version = match engine.read_version(&content) {
            Some(v) => v,
            None => {
                ui::warning(&format!(
                    "{}: could not extract version from matched content",
                    path.display()
                ));
                continue;
            }
        };

        let updated = match engine.write_version(&content, new_version) {
            Ok(u) => u,
            Err(e) => {
                ui::warning(&format!("{}: {e}", path.display()));
                continue;
            }
        };

        let actual_new_version = engine
            .read_version(&updated)
            .unwrap_or_else(|| new_version.to_string());

        if fs::write(&path, &updated).is_err() {
            ui::warning(&format!("{}: failed to write file", path.display()));
            continue;
        }

        paths.push(path.clone());
        results.push(UpdateResult {
            path,
            name: engine.name(),
            old_version,
            new_version: actual_new_version,
            extra: None,
        });
    }
}
