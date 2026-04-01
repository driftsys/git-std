//! Rust ecosystem — Cargo.

use std::path::Path;

use standard_version::{CargoVersionFile, UpdateResult, VersionFile};

use super::{Ecosystem, SyncOutcome, WriteOutcome, native_write, try_sync};
use crate::ui;

pub struct Rust;

/// Native workspace-aware version writer.
///
/// Handles both:
/// - Single-crate manifests: updates `[package] version`.
/// - Workspace manifests: updates `[workspace.package] version` (if present),
///   each member crate that carries a pinned (non-inherited) version, and
///   `[workspace.dependencies]` entries that are local path deps.
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

    // Update [workspace.dependencies] entries that are local path deps.
    // Re-read root content after the [workspace.package] rewrite above.
    let root_content_after = std::fs::read_to_string(&root_cargo).unwrap_or(root_content);
    if let Some(updated) = update_workspace_deps(&root_content_after, &parsed, new_version) {
        if std::fs::write(&root_cargo, &updated).is_err() {
            ui::warning(&format!("{}: failed to write", root_cargo.display()));
        } else if results.is_empty() {
            // Ensure the root manifest path is tracked even when
            // [workspace.package] has no version field.
            results.push(UpdateResult {
                path: root_cargo,
                name: CargoVersionFile.name().to_string(),
                old_version: String::new(),
                new_version: new_version.to_string(),
                extra: None,
            });
        }
    }

    if results.is_empty() {
        WriteOutcome::NotDetected
    } else {
        WriteOutcome::Fallback { results }
    }
}

/// Rewrite `version = "..."` values inside `[workspace.dependencies]` for
/// entries that declare a local `path`.
///
/// Returns `Some(updated_content)` if any line was changed, `None` otherwise.
/// Lines that cannot be handled (e.g. multi-line inline tables) are silently
/// skipped — they are never corrupted.
fn update_workspace_deps(content: &str, parsed: &toml::Value, new_version: &str) -> Option<String> {
    // Collect names of local path deps from the parsed value.
    let local_deps: std::collections::HashSet<String> = parsed
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(|d| d.as_table())
        .map(|t| {
            t.iter()
                .filter(|(_, v)| v.get("path").is_some())
                .map(|(k, _)| k.clone())
                .collect()
        })
        .unwrap_or_default();

    if local_deps.is_empty() {
        return None;
    }

    let mut result = String::with_capacity(content.len());
    let mut changed = false;
    let mut in_section = false;
    let mut rewritten: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Track section boundaries.
        if trimmed == "[workspace.dependencies]" {
            in_section = true;
            result.push_str(line);
            result.push('\n');
            continue;
        } else if trimmed.starts_with('[') {
            in_section = false;
        }

        if in_section {
            // Check if this line starts with a known local dep name.
            if let Some((new_line, dep_name)) = try_rewrite_dep_line(line, &local_deps, new_version)
            {
                result.push_str(&new_line);
                result.push('\n');
                rewritten.insert(dep_name);
                changed = true;
                continue;
            }
        }

        result.push_str(line);
        result.push('\n');
    }

    // Warn about any local path deps that were found in the parsed manifest
    // but could not be rewritten (e.g. multi-line or table-header style).
    for dep in &local_deps {
        if !rewritten.contains(dep) {
            ui::warning(&format!(
                "[workspace.dependencies] {dep}: version not updated \
                 — unsupported format (inline table required). \
                 Update manually: {dep} = {{ version = \"{new_version}\", path = \"...\" }}"
            ));
        }
    }

    // Preserve original trailing-newline behaviour.
    if !content.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    changed.then_some(result)
}

/// If `line` declares one of the `local_deps`, rewrite its inline `version =
/// "..."` value. Returns `Some((new_line, dep_name))` on success, `None` if
/// the line doesn't match or can't be handled safely (never corrupts).
fn try_rewrite_dep_line(
    line: &str,
    local_deps: &std::collections::HashSet<String>,
    new_version: &str,
) -> Option<(String, String)> {
    let trimmed = line.trim();

    // Line must look like:  dep-name = { ... }
    // Find the dep name (everything before the first `=`).
    let eq_pos = trimmed.find('=')?;
    let dep_name = trimmed[..eq_pos].trim().trim_matches('"');

    if !local_deps.contains(dep_name) {
        return None;
    }

    // Only handle single-line inline tables: `dep = { version = "x", path = "y" }`.
    // If the value doesn't start with `{` or doesn't close on this line, skip.
    let value_part = trimmed[eq_pos + 1..].trim();
    if !value_part.starts_with('{') || !value_part.contains('}') {
        return None;
    }

    // Surgically replace `version = "old"` with `version = "new"` inside the
    // inline table using a simple quoted-value replacement.
    let new_value = replace_inline_version(value_part, new_version)?;

    // Reconstruct the line preserving leading whitespace.
    let leading = &line[..line.len() - line.trim_start().len()];
    let key_part = &trimmed[..eq_pos + 1]; // "dep-name ="
    Some((
        format!("{leading}{key_part} {new_value}"),
        dep_name.to_string(),
    ))
}

/// Replace `version = "..."` inside an inline TOML table string.
/// Returns `None` if the pattern is not found (nothing to do).
fn replace_inline_version(inline: &str, new_version: &str) -> Option<String> {
    // Find `version = "` followed by any chars up to the closing `"`.
    let marker = "version = \"";
    let start = inline.find(marker)?;
    let after_open = start + marker.len();
    let end_quote = inline[after_open..].find('"')?;
    let after_close = after_open + end_quote + 1; // position after closing `"`

    Some(format!(
        "{}version = \"{new_version}\"{}",
        &inline[..start],
        &inline[after_close..]
    ))
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
        workspace_native_write(root, new_version)
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
