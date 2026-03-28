//! Workspace auto-discovery for monorepo package detection.
//!
//! Scans workspace manifests to discover packages when `monorepo = true`
//! and no explicit `[[packages]]` are configured. Priority:
//! 1. Cargo (`[workspace] members` in `Cargo.toml`)
//! 2. npm/yarn/pnpm (`"workspaces"` in `package.json`)
//! 3. Deno (`"workspace"` in `deno.json` / `deno.jsonc`)
//! 4. Plain scan (any subdirectory containing a version file)

use std::path::Path;

use super::PackageConfig;

/// Discover packages from workspace manifests.
///
/// Tries each ecosystem in priority order and returns the first non-empty
/// result. Falls back to scanning subdirectories for version files.
pub fn discover_packages(root: &Path) -> Vec<PackageConfig> {
    if let Some(pkgs) = discover_cargo(root).filter(|p| !p.is_empty()) {
        return pkgs;
    }
    if let Some(pkgs) = discover_node(root).filter(|p| !p.is_empty()) {
        return pkgs;
    }
    if let Some(pkgs) = discover_deno(root).filter(|p| !p.is_empty()) {
        return pkgs;
    }
    discover_plain(root)
}

/// Discover Cargo workspace members from `Cargo.toml`.
fn discover_cargo(root: &Path) -> Option<Vec<PackageConfig>> {
    let manifest_path = root.join("Cargo.toml");
    let content = std::fs::read_to_string(&manifest_path).ok()?;
    let table: toml::Table = content.parse().ok()?;

    let workspace = table.get("workspace")?.as_table()?;
    let members = workspace.get("members")?.as_array()?;

    let mut packages = Vec::new();
    for member in members {
        let pattern = member.as_str()?;
        for path in expand_glob(root, pattern) {
            let cargo_toml = root.join(&path).join("Cargo.toml");
            if let Some(name) = read_cargo_package_name(&cargo_toml) {
                packages.push(PackageConfig {
                    name,
                    path,
                    ..Default::default()
                });
            }
        }
    }
    Some(packages)
}

/// Read `[package] name` from a crate-level `Cargo.toml`.
fn read_cargo_package_name(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let table: toml::Table = content.parse().ok()?;
    let package = table.get("package")?.as_table()?;
    package.get("name")?.as_str().map(String::from)
}

/// Discover npm/yarn/pnpm workspace members from `package.json`.
fn discover_node(root: &Path) -> Option<Vec<PackageConfig>> {
    let pkg_path = root.join("package.json");
    let content = std::fs::read_to_string(&pkg_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    let workspaces = json.get("workspaces")?;
    // npm/yarn: "workspaces": ["packages/*"]
    // pnpm: "workspaces": ["packages/*"] (same format in package.json)
    let patterns = workspaces.as_array()?;

    let mut packages = Vec::new();
    for pattern in patterns {
        let glob = pattern.as_str()?;
        for path in expand_glob(root, glob) {
            let pkg_json = root.join(&path).join("package.json");
            if let Some(name) = read_json_name(&pkg_json) {
                packages.push(PackageConfig {
                    name,
                    path,
                    ..Default::default()
                });
            }
        }
    }
    Some(packages)
}

/// Read `"name"` from a `package.json` or `deno.json`.
fn read_json_name(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("name")?.as_str().map(String::from)
}

/// Discover Deno workspace members from `deno.json` or `deno.jsonc`.
fn discover_deno(root: &Path) -> Option<Vec<PackageConfig>> {
    let deno_path = ["deno.json", "deno.jsonc"]
        .iter()
        .map(|f| root.join(f))
        .find(|p| p.exists())?;

    let content = std::fs::read_to_string(&deno_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    let workspace = json.get("workspace")?;
    let members = workspace.as_array().or_else(|| {
        workspace
            .as_object()
            .and_then(|o| o.get("members"))
            .and_then(|m| m.as_array())
    })?;

    let mut packages = Vec::new();
    for member in members {
        let path = member.as_str()?;
        let deno_json = root.join(path).join("deno.json");
        let deno_jsonc = root.join(path).join("deno.jsonc");
        let name = read_json_name(&deno_json)
            .or_else(|| read_json_name(&deno_jsonc))
            .unwrap_or_else(|| {
                Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(path)
                    .to_string()
            });
        packages.push(PackageConfig {
            name,
            path: path.to_string(),
            ..Default::default()
        });
    }
    Some(packages)
}

/// Version files that indicate a package directory.
const VERSION_FILE_NAMES: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "deno.json",
    "deno.jsonc",
    "pyproject.toml",
    "pubspec.yaml",
    "VERSION",
];

/// Discover packages by scanning subdirectories for version files.
///
/// Looks one level deep in common workspace directories (`crates/`, `packages/`,
/// `modules/`) and falls back to any top-level subdirectory containing a version file.
fn discover_plain(root: &Path) -> Vec<PackageConfig> {
    let mut packages = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Scan known workspace directories first
    let workspace_dirs = ["crates", "packages", "modules", "libs"];
    for dir in &workspace_dirs {
        let parent = root.join(dir);
        if parent.is_dir() {
            scan_subdirs(root, &parent, &mut packages, &mut seen);
        }
    }

    packages.sort_by(|a, b| a.name.cmp(&b.name));
    packages
}

/// Scan subdirectories for version files and add them as packages.
fn scan_subdirs(
    root: &Path,
    parent: &Path,
    packages: &mut Vec<PackageConfig>,
    seen: &mut std::collections::HashSet<String>,
) {
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }
        let has_version_file = VERSION_FILE_NAMES
            .iter()
            .any(|f| entry_path.join(f).exists());
        if !has_version_file {
            continue;
        }
        let rel_path = entry_path
            .strip_prefix(root)
            .unwrap_or(&entry_path)
            .to_string_lossy()
            .to_string();
        if !seen.insert(rel_path.clone()) {
            continue;
        }
        let name = entry.file_name().to_str().unwrap_or_default().to_string();
        packages.push(PackageConfig {
            name,
            path: rel_path,
            ..Default::default()
        });
    }
}

/// Expand a glob pattern relative to root, returning matching directory paths
/// as strings relative to root.
fn expand_glob(root: &Path, pattern: &str) -> Vec<String> {
    let full_pattern = root.join(pattern);
    let pattern_str = full_pattern.to_string_lossy();

    let mut results = Vec::new();
    if let Ok(entries) = glob::glob(&pattern_str) {
        for entry in entries.flatten() {
            if entry.is_dir()
                && let Ok(rel) = entry.strip_prefix(root)
            {
                results.push(rel.to_string_lossy().to_string());
            }
        }
    }
    results.sort();
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(root: &Path, path: &str, content: &str) {
        let full = root.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, content).unwrap();
    }

    // ── Cargo workspace ─────────────────────────────────────

    #[test]
    fn cargo_workspace_discovered() {
        let dir = tempfile::tempdir().unwrap();
        write_file(
            dir.path(),
            "Cargo.toml",
            r#"
[workspace]
members = ["crates/core", "crates/cli"]
"#,
        );
        write_file(
            dir.path(),
            "crates/core/Cargo.toml",
            r#"
[package]
name = "my-core"
version = "0.1.0"
"#,
        );
        write_file(
            dir.path(),
            "crates/cli/Cargo.toml",
            r#"
[package]
name = "my-cli"
version = "0.1.0"
"#,
        );

        let packages = discover_packages(dir.path());
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].name, "my-core");
        assert_eq!(packages[0].path, "crates/core");
        assert_eq!(packages[1].name, "my-cli");
        assert_eq!(packages[1].path, "crates/cli");
    }

    #[test]
    fn cargo_workspace_with_glob() {
        let dir = tempfile::tempdir().unwrap();
        write_file(
            dir.path(),
            "Cargo.toml",
            r#"
[workspace]
members = ["crates/*"]
"#,
        );
        write_file(
            dir.path(),
            "crates/alpha/Cargo.toml",
            r#"
[package]
name = "alpha"
version = "0.1.0"
"#,
        );
        write_file(
            dir.path(),
            "crates/beta/Cargo.toml",
            r#"
[package]
name = "beta"
version = "0.1.0"
"#,
        );

        let packages = discover_packages(dir.path());
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].name, "alpha");
        assert_eq!(packages[1].name, "beta");
    }

    // ── npm workspace ───────────────────────────────────────

    #[test]
    fn npm_workspace_discovered() {
        let dir = tempfile::tempdir().unwrap();
        write_file(
            dir.path(),
            "package.json",
            r#"{"name": "root", "workspaces": ["packages/*"]}"#,
        );
        write_file(
            dir.path(),
            "packages/web/package.json",
            r#"{"name": "@scope/web", "version": "1.0.0"}"#,
        );
        write_file(
            dir.path(),
            "packages/api/package.json",
            r#"{"name": "@scope/api", "version": "1.0.0"}"#,
        );

        let packages = discover_packages(dir.path());
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].name, "@scope/api");
        assert_eq!(packages[1].name, "@scope/web");
    }

    // ── Deno workspace ──────────────────────────────────────

    #[test]
    fn deno_workspace_discovered() {
        let dir = tempfile::tempdir().unwrap();
        write_file(
            dir.path(),
            "deno.json",
            r#"{"workspace": ["libs/core", "libs/utils"]}"#,
        );
        write_file(
            dir.path(),
            "libs/core/deno.json",
            r#"{"name": "core", "version": "1.0.0"}"#,
        );
        write_file(
            dir.path(),
            "libs/utils/deno.json",
            r#"{"name": "utils", "version": "0.5.0"}"#,
        );

        let packages = discover_packages(dir.path());
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].name, "core");
        assert_eq!(packages[1].name, "utils");
    }

    #[test]
    fn deno_workspace_with_members_object() {
        let dir = tempfile::tempdir().unwrap();
        write_file(
            dir.path(),
            "deno.json",
            r#"{"workspace": {"members": ["libs/core"]}}"#,
        );
        write_file(dir.path(), "libs/core/deno.json", r#"{"name": "core"}"#);

        let packages = discover_packages(dir.path());
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "core");
    }

    // ── Plain directory scan ────────────────────────────────

    #[test]
    fn plain_scan_discovers_version_files() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), "crates/alpha/VERSION", "0.1.0");
        write_file(
            dir.path(),
            "crates/beta/Cargo.toml",
            "[package]\nname = \"beta\"\n",
        );

        let packages = discover_packages(dir.path());
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].name, "alpha");
        assert_eq!(packages[1].name, "beta");
    }

    #[test]
    fn plain_scan_ignores_dirs_without_version_files() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), "crates/alpha/VERSION", "0.1.0");
        std::fs::create_dir_all(dir.path().join("crates/empty")).unwrap();

        let packages = discover_packages(dir.path());
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "alpha");
    }

    // ── Priority ────────────────────────────────────────────

    #[test]
    fn cargo_takes_precedence_over_plain_scan() {
        let dir = tempfile::tempdir().unwrap();
        write_file(
            dir.path(),
            "Cargo.toml",
            r#"
[workspace]
members = ["crates/core"]
"#,
        );
        write_file(
            dir.path(),
            "crates/core/Cargo.toml",
            r#"
[package]
name = "core"
version = "0.1.0"
"#,
        );
        // Also has a VERSION file — should be ignored since Cargo wins
        write_file(dir.path(), "crates/core/VERSION", "0.1.0");
        write_file(dir.path(), "packages/extra/VERSION", "0.2.0");

        let packages = discover_packages(dir.path());
        // Only Cargo members, not the plain scan
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "core");
    }

    // ── Edge cases ──────────────────────────────────────────

    #[test]
    fn empty_workspace_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let packages = discover_packages(dir.path());
        assert!(packages.is_empty());
    }

    #[test]
    fn no_overrides_on_discovered_packages() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), "crates/alpha/VERSION", "0.1.0");

        let packages = discover_packages(dir.path());
        assert!(packages[0].scheme.is_none());
        assert!(packages[0].version_files.is_none());
        assert!(packages[0].changelog.is_none());
    }
}
