//! Dependency graph resolution for monorepo workspaces.
//!
//! Parses workspace manifests (Cargo.toml, package.json, deno.json) to build
//! a graph of runtime dependencies between packages. Dev-dependencies are
//! excluded to avoid circular cascade and unnecessary patch bumps.

use std::collections::HashMap;
use std::path::Path;

use super::PackageConfig;

/// A dependency graph mapping each package name to its runtime dependents.
///
/// `dependents["core"] = ["cli", "api"]` means both `cli` and `api` depend
/// on `core` — so when `core` bumps, `cli` and `api` get at least a patch.
#[derive(Debug, Default)]
pub struct DependencyGraph {
    /// Maps package name → list of packages that depend on it.
    dependents: HashMap<String, Vec<String>>,
}

impl DependencyGraph {
    /// Packages that directly depend on `name`.
    pub fn dependents_of(&self, name: &str) -> &[String] {
        self.dependents
            .get(name)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Returns `true` when the graph has no edges.
    pub fn is_empty(&self) -> bool {
        self.dependents.values().all(|v| v.is_empty())
    }
}

/// Build a dependency graph from workspace manifests.
///
/// For each package, parse its manifest and collect runtime (non-dev)
/// dependencies that reference other workspace packages. The result is
/// inverted: instead of "A depends on B", we store "B has dependent A"
/// so cascade lookups are O(1).
pub fn resolve_dependency_graph(root: &Path, packages: &[PackageConfig]) -> DependencyGraph {
    let pkg_names: Vec<&str> = packages.iter().map(|p| p.name.as_str()).collect();
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

    for pkg in packages {
        let pkg_dir = root.join(&pkg.path);
        let deps = parse_runtime_deps(&pkg_dir, &pkg_names);
        for dep in deps {
            dependents.entry(dep).or_default().push(pkg.name.clone());
        }
    }

    // Sort dependent lists for deterministic output.
    for v in dependents.values_mut() {
        v.sort();
        v.dedup();
    }

    DependencyGraph { dependents }
}

/// Parse runtime dependencies from a package directory.
///
/// Tries Cargo.toml first, then package.json, then deno.json/deno.jsonc.
/// Returns only workspace-internal dependencies (names present in `workspace_names`).
fn parse_runtime_deps(pkg_dir: &Path, workspace_names: &[&str]) -> Vec<String> {
    if let Some(deps) = parse_cargo_deps(pkg_dir, workspace_names) {
        return deps;
    }
    if let Some(deps) = parse_node_deps(pkg_dir, workspace_names) {
        return deps;
    }
    if let Some(deps) = parse_deno_deps(pkg_dir, workspace_names) {
        return deps;
    }
    Vec::new()
}

/// Parse `[dependencies]` from `Cargo.toml`, excluding `[dev-dependencies]`.
fn parse_cargo_deps(pkg_dir: &Path, workspace_names: &[&str]) -> Option<Vec<String>> {
    let cargo_toml = pkg_dir.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml).ok()?;
    let table: toml::Table = content.parse().ok()?;

    let mut deps = Vec::new();

    // [dependencies]
    if let Some(dep_table) = table.get("dependencies").and_then(|v| v.as_table()) {
        collect_cargo_workspace_deps(dep_table, workspace_names, &mut deps);
    }

    // [build-dependencies] — included in cascade (needed at build time)
    if let Some(dep_table) = table.get("build-dependencies").and_then(|v| v.as_table()) {
        collect_cargo_workspace_deps(dep_table, workspace_names, &mut deps);
    }

    // [target.'cfg(...)'.dependencies] — scan for workspace deps
    if let Some(target_table) = table.get("target").and_then(|v| v.as_table()) {
        for (_target_spec, target_val) in target_table {
            if let Some(t) = target_val.as_table()
                && let Some(d) = t.get("dependencies").and_then(|v| v.as_table())
            {
                collect_cargo_workspace_deps(d, workspace_names, &mut deps);
            }
        }
    }

    // Explicitly skip [dev-dependencies]

    deps.sort();
    deps.dedup();
    Some(deps)
}

/// Collect workspace-internal dependency names from a Cargo dependency table.
///
/// Handles both simple (`foo = "1.0"`) and table (`foo = { path = "..." }`)
/// dependency specs. Only includes names matching workspace packages.
fn collect_cargo_workspace_deps(
    dep_table: &toml::Table,
    workspace_names: &[&str],
    out: &mut Vec<String>,
) {
    for (dep_name, _dep_spec) in dep_table {
        if workspace_names.contains(&dep_name.as_str()) {
            out.push(dep_name.clone());
        }
    }
}

/// Parse `"dependencies"` from `package.json`, excluding `"devDependencies"`.
fn parse_node_deps(pkg_dir: &Path, workspace_names: &[&str]) -> Option<Vec<String>> {
    let pkg_json = pkg_dir.join("package.json");
    let content = std::fs::read_to_string(&pkg_json).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    let mut deps = Vec::new();

    // "dependencies" only — skip "devDependencies"
    if let Some(dep_obj) = json.get("dependencies").and_then(|v| v.as_object()) {
        for dep_name in dep_obj.keys() {
            if workspace_names.contains(&dep_name.as_str()) {
                deps.push(dep_name.clone());
            }
        }
    }

    // "peerDependencies" — include in cascade (runtime requirement)
    if let Some(dep_obj) = json.get("peerDependencies").and_then(|v| v.as_object()) {
        for dep_name in dep_obj.keys() {
            if workspace_names.contains(&dep_name.as_str()) {
                deps.push(dep_name.clone());
            }
        }
    }

    deps.sort();
    deps.dedup();
    Some(deps)
}

/// Parse `"imports"` from `deno.json` / `deno.jsonc`, excluding
/// entries that look like dev or test imports.
fn parse_deno_deps(pkg_dir: &Path, workspace_names: &[&str]) -> Option<Vec<String>> {
    let deno_path = ["deno.json", "deno.jsonc"]
        .iter()
        .map(|f| pkg_dir.join(f))
        .find(|p| p.exists())?;

    let content = std::fs::read_to_string(&deno_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    let mut deps = Vec::new();

    if let Some(imports) = json.get("imports").and_then(|v| v.as_object()) {
        for dep_name in imports.keys() {
            if workspace_names.contains(&dep_name.as_str()) {
                deps.push(dep_name.clone());
            }
        }
    }

    deps.sort();
    deps.dedup();
    Some(deps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(root: &Path, path: &str, content: &str) {
        let full = root.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, content).unwrap();
    }

    fn make_packages(specs: &[(&str, &str)]) -> Vec<PackageConfig> {
        specs
            .iter()
            .map(|(name, path)| PackageConfig {
                name: name.to_string(),
                path: path.to_string(),
                ..Default::default()
            })
            .collect()
    }

    // ── Cargo dependency parsing ───────────────────────────

    #[test]
    fn cargo_runtime_deps_included() {
        let dir = tempfile::tempdir().unwrap();
        write_file(
            dir.path(),
            "crates/cli/Cargo.toml",
            r#"
[package]
name = "my-cli"
version = "0.1.0"

[dependencies]
core = { path = "../core" }
serde = "1"
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
        let packages = make_packages(&[("core", "crates/core"), ("my-cli", "crates/cli")]);
        let graph = resolve_dependency_graph(dir.path(), &packages);

        assert_eq!(graph.dependents_of("core"), &["my-cli"]);
        assert!(graph.dependents_of("my-cli").is_empty());
    }

    #[test]
    fn cargo_dev_deps_excluded() {
        let dir = tempfile::tempdir().unwrap();
        write_file(
            dir.path(),
            "crates/cli/Cargo.toml",
            r#"
[package]
name = "my-cli"
version = "0.1.0"

[dev-dependencies]
core = { path = "../core" }
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
        let packages = make_packages(&[("core", "crates/core"), ("my-cli", "crates/cli")]);
        let graph = resolve_dependency_graph(dir.path(), &packages);

        assert!(graph.dependents_of("core").is_empty());
    }

    #[test]
    fn cargo_build_deps_included() {
        let dir = tempfile::tempdir().unwrap();
        write_file(
            dir.path(),
            "crates/cli/Cargo.toml",
            r#"
[package]
name = "my-cli"
version = "0.1.0"

[build-dependencies]
core = { path = "../core" }
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
        let packages = make_packages(&[("core", "crates/core"), ("my-cli", "crates/cli")]);
        let graph = resolve_dependency_graph(dir.path(), &packages);

        assert_eq!(graph.dependents_of("core"), &["my-cli"]);
    }

    // ── npm dependency parsing ─────────────────────────────

    #[test]
    fn node_runtime_deps_included() {
        let dir = tempfile::tempdir().unwrap();
        write_file(
            dir.path(),
            "packages/web/package.json",
            r#"{"name": "web", "dependencies": {"core": "workspace:*", "react": "^18"}}"#,
        );
        write_file(
            dir.path(),
            "packages/core/package.json",
            r#"{"name": "core"}"#,
        );
        let packages = make_packages(&[("core", "packages/core"), ("web", "packages/web")]);
        let graph = resolve_dependency_graph(dir.path(), &packages);

        assert_eq!(graph.dependents_of("core"), &["web"]);
    }

    #[test]
    fn node_dev_deps_excluded() {
        let dir = tempfile::tempdir().unwrap();
        write_file(
            dir.path(),
            "packages/web/package.json",
            r#"{"name": "web", "devDependencies": {"core": "workspace:*"}}"#,
        );
        write_file(
            dir.path(),
            "packages/core/package.json",
            r#"{"name": "core"}"#,
        );
        let packages = make_packages(&[("core", "packages/core"), ("web", "packages/web")]);
        let graph = resolve_dependency_graph(dir.path(), &packages);

        assert!(graph.dependents_of("core").is_empty());
    }

    #[test]
    fn node_peer_deps_included() {
        let dir = tempfile::tempdir().unwrap();
        write_file(
            dir.path(),
            "packages/plugin/package.json",
            r#"{"name": "plugin", "peerDependencies": {"core": "^1.0"}}"#,
        );
        write_file(
            dir.path(),
            "packages/core/package.json",
            r#"{"name": "core"}"#,
        );
        let packages = make_packages(&[("core", "packages/core"), ("plugin", "packages/plugin")]);
        let graph = resolve_dependency_graph(dir.path(), &packages);

        assert_eq!(graph.dependents_of("core"), &["plugin"]);
    }

    // ── Transitive / multi-edge ────────────────────────────

    #[test]
    fn transitive_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        // C depends on B, B depends on A
        write_file(
            dir.path(),
            "crates/a/Cargo.toml",
            r#"
[package]
name = "a"
version = "0.1.0"
"#,
        );
        write_file(
            dir.path(),
            "crates/b/Cargo.toml",
            r#"
[package]
name = "b"
version = "0.1.0"

[dependencies]
a = { path = "../a" }
"#,
        );
        write_file(
            dir.path(),
            "crates/c/Cargo.toml",
            r#"
[package]
name = "c"
version = "0.1.0"

[dependencies]
b = { path = "../b" }
"#,
        );
        let packages = make_packages(&[("a", "crates/a"), ("b", "crates/b"), ("c", "crates/c")]);
        let graph = resolve_dependency_graph(dir.path(), &packages);

        // A's direct dependents: B
        assert_eq!(graph.dependents_of("a"), &["b"]);
        // B's direct dependents: C
        assert_eq!(graph.dependents_of("b"), &["c"]);
    }

    #[test]
    fn multiple_dependents() {
        let dir = tempfile::tempdir().unwrap();
        write_file(
            dir.path(),
            "crates/core/Cargo.toml",
            r#"
[package]
name = "core"
version = "0.1.0"
"#,
        );
        write_file(
            dir.path(),
            "crates/cli/Cargo.toml",
            r#"
[package]
name = "cli"
version = "0.1.0"

[dependencies]
core = { path = "../core" }
"#,
        );
        write_file(
            dir.path(),
            "crates/api/Cargo.toml",
            r#"
[package]
name = "api"
version = "0.1.0"

[dependencies]
core = { path = "../core" }
"#,
        );
        let packages = make_packages(&[
            ("api", "crates/api"),
            ("cli", "crates/cli"),
            ("core", "crates/core"),
        ]);
        let graph = resolve_dependency_graph(dir.path(), &packages);

        assert_eq!(graph.dependents_of("core"), &["api", "cli"]);
    }

    // ── Edge cases ─────────────────────────────────────────

    #[test]
    fn empty_packages_produces_empty_graph() {
        let dir = tempfile::tempdir().unwrap();
        let graph = resolve_dependency_graph(dir.path(), &[]);
        assert!(graph.is_empty());
    }

    #[test]
    fn external_deps_ignored() {
        let dir = tempfile::tempdir().unwrap();
        write_file(
            dir.path(),
            "crates/app/Cargo.toml",
            r#"
[package]
name = "app"
version = "0.1.0"

[dependencies]
serde = "1"
tokio = { version = "1", features = ["full"] }
"#,
        );
        let packages = make_packages(&[("app", "crates/app")]);
        let graph = resolve_dependency_graph(dir.path(), &packages);

        assert!(graph.is_empty());
    }

    #[test]
    fn no_manifest_produces_no_deps() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("crates/orphan")).unwrap();
        let packages = make_packages(&[("orphan", "crates/orphan")]);
        let graph = resolve_dependency_graph(dir.path(), &packages);

        assert!(graph.is_empty());
    }
}
