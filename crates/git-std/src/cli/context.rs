//! `git std --context` — project awareness dump for agent consumption.
//!
//! Prints a Markdown document to stdout covering project config, workspace
//! packages, commit rules, and the current staged diff. Safe to pipe into
//! any agent that has shell access.

use std::path::Path;

use crate::app::OutputFormat;
use crate::config::{self, ProjectConfig, Scheme, ScopesConfig};
use crate::git;
use crate::git::workdir;
use crate::ui;

// ── Constants ─────────────────────────────────────────────────────────────────

const STABLE_BRANCHES: &[&str] = &["main", "master"];
const MAX_UNSTAGED_SHOWN: usize = 5;

// ── Type suggestion helpers ───────────────────────────────────────────────────

/// Classify a file path into a semantic category.
fn classify_file(path: &str) -> &'static str {
    // docs: explicit doc files
    if path.starts_with("docs/")
        || path.starts_with("README")
        || path.starts_with("CONTRIBUTING")
        || path.starts_with("AGENTS")
        || path.starts_with("CLAUDE")
        || path.ends_with(".rst")
        || path.ends_with(".adoc")
        || path.ends_with(".asciidoc")
    {
        return "docs";
    }

    // test: test directories and test file patterns
    if path.starts_with("tests/")
        || path.starts_with("test/")
        || path.starts_with("spec/")
        || path.ends_with("_test.rs")
        || path.contains(".test.")
        || path.contains(".spec.")
    {
        return "test";
    }

    // code: source files
    if path.ends_with(".rs")
        || path.ends_with(".ts")
        || path.ends_with(".tsx")
        || path.ends_with(".js")
        || path.ends_with(".jsx")
        || path.ends_with(".py")
        || path.ends_with(".go")
        || path.ends_with(".c")
        || path.ends_with(".cpp")
        || path.ends_with(".h")
        || path.ends_with(".java")
        || path.ends_with(".kt")
        || path.ends_with(".swift")
        || path.ends_with(".rb")
        || path.ends_with(".php")
    {
        return "code";
    }

    // everything else: chore
    "chore"
}

/// Suggest a commit type based on staged files and allowed types.
///
/// Returns:
/// - A single type if all files fall into one category (and type is allowed)
/// - A shortlist like "feat or fix" for ambiguous code changes
/// - `None` if no confident suggestion can be made
fn suggest_type(files: &[String], allowed_types: &[String]) -> Option<String> {
    if files.is_empty() {
        return None;
    }

    // Classify all staged files
    let mut has_docs = false;
    let mut has_test = false;
    let mut has_code = false;
    let mut has_chore = false;

    for file in files {
        match classify_file(file) {
            "docs" => has_docs = true,
            "test" => has_test = true,
            "code" => has_code = true,
            "chore" => has_chore = true,
            _ => {}
        }
    }

    // Rule 1: all docs
    if has_docs
        && !has_test
        && !has_code
        && !has_chore
        && allowed_types.contains(&"docs".to_string())
    {
        return Some("docs".to_string());
    }

    // Rule 2: all test
    if has_test
        && !has_docs
        && !has_code
        && !has_chore
        && allowed_types.contains(&"test".to_string())
    {
        return Some("test".to_string());
    }

    // Rule 3: all chore
    if has_chore
        && !has_docs
        && !has_test
        && !has_code
        && allowed_types.contains(&"chore".to_string())
    {
        return Some("chore".to_string());
    }

    // Rule 4: code with tests → "feat or fix"
    if has_code && has_test && !has_docs && !has_chore {
        let mut suggestion = String::new();
        if allowed_types.contains(&"feat".to_string()) {
            suggestion.push_str("feat");
        }
        if allowed_types.contains(&"fix".to_string()) {
            if !suggestion.is_empty() {
                suggestion.push_str(" or fix");
            } else {
                suggestion.push_str("fix");
            }
        }
        if !suggestion.is_empty() {
            return Some(suggestion);
        }
    }

    // Rule 5: code without tests → "fix or refactor"
    if has_code && !has_test && !has_docs && !has_chore {
        let mut suggestion = String::new();
        if allowed_types.contains(&"fix".to_string()) {
            suggestion.push_str("fix");
        }
        if allowed_types.contains(&"refactor".to_string()) {
            if !suggestion.is_empty() {
                suggestion.push_str(" or refactor");
            } else {
                suggestion.push_str("refactor");
            }
        }
        if !suggestion.is_empty() {
            return Some(suggestion);
        }
    }

    // No confident suggestion
    None
}

// ── Data model ────────────────────────────────────────────────────────────────

struct ProjectInfo {
    scheme: &'static str,
    tag_prefix: String,
    stable: bool,
    stable_detail: String,
}

struct WorkspaceGroup {
    label: &'static str,
    names: Vec<String>,
}

struct CommitConfig {
    types: Vec<String>,
    /// `None` means no scopes configured — omit the Scopes line.
    scopes_line: Option<String>,
    refs_required: Vec<String>,
    /// Suggested type based on staged files. `None` means no confident suggestion.
    /// May be a single type or a shortlist like "feat or fix".
    suggested_type: Option<String>,
}

enum GitState {
    NotBootstrapped,
    Clean,
    NothingStaged { unstaged: Vec<String> },
    Staged { diff: String },
    StagedAndUnstaged { diff: String, unstaged: Vec<String> },
}

// ── Builders ──────────────────────────────────────────────────────────────────

fn build_project_info(root: &Path, cfg: &ProjectConfig) -> ProjectInfo {
    let scheme = match cfg.scheme {
        Scheme::Semver => "semver",
        Scheme::Calver => "calver",
        Scheme::Patch => "patch",
    };

    let tag_prefix = cfg.versioning.tag_prefix.clone();

    let branch = git::current_branch(root).unwrap_or_default();
    let on_stable_branch = STABLE_BRANCHES.contains(&branch.as_str());

    let (tag_is_stable, tag_detail) = match git::find_latest_version_tag(root, &tag_prefix) {
        Ok(Some((_, ver))) => {
            let stable = ver.pre.is_empty();
            let detail = if stable {
                "no prerelease tag".to_owned()
            } else {
                format!("prerelease: {}", ver.pre)
            };
            (stable, detail)
        }
        Ok(None) => (true, "no tag yet".to_owned()),
        Err(_) => (false, "tag lookup failed".to_owned()),
    };

    let stable = on_stable_branch && tag_is_stable;
    let stable_detail = format!("{branch}, {tag_detail}");

    ProjectInfo {
        scheme,
        tag_prefix,
        stable,
        stable_detail,
    }
}

fn build_workspace_groups(root: &Path, cfg: &ProjectConfig) -> Vec<WorkspaceGroup> {
    let packages = cfg.resolved_packages(root);
    if packages.is_empty() {
        return Vec::new();
    }

    let mut crates: Vec<String> = Vec::new();
    let mut pkgs: Vec<String> = Vec::new();
    let mut modules: Vec<String> = Vec::new();
    let mut other: Vec<String> = Vec::new();

    for pkg in &packages {
        if pkg.path.starts_with("crates/") {
            crates.push(pkg.name.clone());
        } else if pkg.path.starts_with("packages/") {
            pkgs.push(pkg.name.clone());
        } else if pkg.path.starts_with("modules/") {
            modules.push(pkg.name.clone());
        } else {
            other.push(pkg.name.clone());
        }
    }

    let mut groups = Vec::new();
    if !crates.is_empty() {
        groups.push(WorkspaceGroup {
            label: "Crates",
            names: crates,
        });
    }
    if !pkgs.is_empty() {
        groups.push(WorkspaceGroup {
            label: "Packages",
            names: pkgs,
        });
    }
    if !modules.is_empty() {
        groups.push(WorkspaceGroup {
            label: "Modules",
            names: modules,
        });
    }
    if !other.is_empty() {
        groups.push(WorkspaceGroup {
            label: "Other",
            names: other,
        });
    }

    groups
}

fn build_commit_config(cfg: &ProjectConfig, files: &[String]) -> CommitConfig {
    let types = cfg.types.clone();

    let scopes_line = match &cfg.scopes {
        ScopesConfig::None => None,
        ScopesConfig::Auto => {
            let q = if cfg.strict {
                "(required, strict)"
            } else {
                "(optional)"
            };
            Some(format!("from workspace {q}"))
        }
        ScopesConfig::List(list) => {
            let q = if cfg.strict {
                "(required, strict)"
            } else {
                "(optional)"
            };
            Some(format!("{} {q}", list.join(", ")))
        }
    };

    let refs_required = cfg.refs_required.clone();
    let suggested_type = suggest_type(files, &types);

    CommitConfig {
        types,
        scopes_line,
        refs_required,
        suggested_type,
    }
}

/// Return `true` when the repo does not need bootstrapping or is already done.
///
/// "Not bootstrapped" = `.githooks/` exists but `core.hooksPath` is not set
/// to `.githooks`. Reuses the same logic as `doctor`'s hooks health check.
fn is_bootstrapped(root: &Path) -> bool {
    if !root.join(".githooks").exists() {
        return true;
    }

    matches!(
        git::config_value(root, "core.hooksPath").as_deref(),
        Ok(".githooks")
    )
}

/// Parse `git status --short` and return lines with working-tree changes or untracked files.
///
/// Format: `XY filename` where X = index status, Y = working-tree status.
/// We keep lines where Y != ' ' (working-tree has changes or file is untracked).
fn unstaged_lines(status_output: &str) -> Vec<String> {
    status_output
        .lines()
        .filter(|line| {
            let mut chars = line.chars();
            chars.next(); // X
            chars.next().is_some_and(|y| y != ' ')
        })
        .map(str::to_owned)
        .collect()
}

fn gather_git_state(root: &Path) -> Result<GitState, git::cmd::GitError> {
    if !is_bootstrapped(root) {
        return Ok(GitState::NotBootstrapped);
    }

    let diff = git::staged_diff(root)?;
    let status_raw = git::short_status(root)?;
    let unstaged = unstaged_lines(&status_raw);

    Ok(match (diff.is_empty(), unstaged.is_empty()) {
        (true, true) => GitState::Clean,
        (true, false) => GitState::NothingStaged { unstaged },
        (false, true) => GitState::Staged { diff },
        (false, false) => GitState::StagedAndUnstaged { diff, unstaged },
    })
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Run `git std --context`. Returns the process exit code.
pub fn run(cwd: &Path, format: OutputFormat) -> i32 {
    let root = match workdir(cwd) {
        Ok(p) => p,
        Err(_) => {
            ui::error("not a git repository");
            return 2;
        }
    };

    let cfg = config::load(&root);
    let project = build_project_info(&root, &cfg);
    let workspace = build_workspace_groups(&root, &cfg);

    // Get staged files for type suggestion
    let staged_files = git::staged_files(&root).unwrap_or_default();

    let commit_cfg = build_commit_config(&cfg, &staged_files);

    let state = match gather_git_state(&root) {
        Ok(s) => s,
        Err(e) => {
            ui::error(&format!("git: {e}"));
            return 1;
        }
    };

    match format {
        OutputFormat::Text => render_text(&project, &workspace, &commit_cfg, &state),
        OutputFormat::Json => render_json(&project, &workspace, &commit_cfg, &state),
    }
}

// ── Text rendering ────────────────────────────────────────────────────────────

fn render_text(
    project: &ProjectInfo,
    workspace: &[WorkspaceGroup],
    commit_cfg: &CommitConfig,
    state: &GitState,
) -> i32 {
    println!("## Project");
    println!("Scheme: {}", project.scheme);
    println!("Stable: {} ({})", project.stable, project.stable_detail);
    println!("Tag prefix: {}", project.tag_prefix);

    if !workspace.is_empty() {
        println!();
        println!("## Workspace");
        for group in workspace {
            println!(
                "  {:<10}{}",
                format!("{}:", group.label),
                group.names.join(", ")
            );
        }
    }

    println!();
    println!("## Commit config");
    println!("Types: {}", commit_cfg.types.join(", "));
    if let Some(suggested) = &commit_cfg.suggested_type {
        println!("Suggested type: {suggested}");
    }
    if let Some(scopes) = &commit_cfg.scopes_line {
        println!("Scopes: {scopes}");
    }
    if !commit_cfg.refs_required.is_empty() {
        println!("Refs: required for {}", commit_cfg.refs_required.join(", "));
    }

    match state {
        GitState::NotBootstrapped => {
            println!();
            println!("⚠ Not bootstrapped — run `git std bootstrap`");
            return 1;
        }
        GitState::Clean => {
            println!();
            println!("Nothing to commit — working tree clean");
        }
        GitState::NothingStaged { unstaged } => {
            println!();
            println!("Nothing staged — run `git add` first");
            println!();
            println!("## Unstaged files");
            render_unstaged(unstaged);
        }
        GitState::Staged { diff } => {
            println!();
            println!("## Staged diff");
            println!("````diff");
            println!("{diff}");
            println!("````");
        }
        GitState::StagedAndUnstaged { diff, unstaged } => {
            println!();
            println!("## Staged diff");
            println!("````diff");
            println!("{diff}");
            println!("````");
            println!();
            println!("## Unstaged files");
            render_unstaged(unstaged);
        }
    }

    0
}

fn render_unstaged(lines: &[String]) {
    let shown = &lines[..lines.len().min(MAX_UNSTAGED_SHOWN)];
    for line in shown {
        println!("{line}");
    }
    if lines.len() > MAX_UNSTAGED_SHOWN {
        println!("... and {} more", lines.len() - MAX_UNSTAGED_SHOWN);
    }
}

// ── JSON rendering ────────────────────────────────────────────────────────────

fn render_json(
    project: &ProjectInfo,
    workspace: &[WorkspaceGroup],
    commit_cfg: &CommitConfig,
    state: &GitState,
) -> i32 {
    let workspace_json: Vec<serde_json::Value> = workspace
        .iter()
        .map(|g| serde_json::json!({ "label": g.label, "packages": g.names }))
        .collect();

    let (exit_code, status_str, staged_diff, unstaged_files) = match state {
        GitState::NotBootstrapped => (
            1,
            "not_bootstrapped",
            serde_json::Value::Null,
            serde_json::Value::Null,
        ),
        GitState::Clean => (0, "clean", serde_json::Value::Null, serde_json::Value::Null),
        GitState::NothingStaged { unstaged } => (
            0,
            "nothing_staged",
            serde_json::Value::Null,
            serde_json::Value::Array(
                unstaged
                    .iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            ),
        ),
        GitState::Staged { diff } => (
            0,
            "staged",
            serde_json::Value::String(diff.clone()),
            serde_json::Value::Null,
        ),
        GitState::StagedAndUnstaged { diff, unstaged } => (
            0,
            "staged_and_unstaged",
            serde_json::Value::String(diff.clone()),
            serde_json::Value::Array(
                unstaged
                    .iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            ),
        ),
    };

    let output = serde_json::json!({
        "project": {
            "scheme": project.scheme,
            "tag_prefix": project.tag_prefix,
            "stable": project.stable,
            "stable_detail": project.stable_detail,
        },
        "workspace": workspace_json,
        "commit_config": {
            "types": commit_cfg.types,
            "suggested_type": commit_cfg.suggested_type,
            "scopes": commit_cfg.scopes_line,
            "refs_required": commit_cfg.refs_required,
        },
        "staged_diff": staged_diff,
        "unstaged_files": unstaged_files,
        "status": status_str,
    });

    println!("{output}");
    exit_code
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unstaged_lines_filters_working_tree_changes() {
        let input = " M src/lib.rs\nA  src/new.rs\n?? scratch.txt\nMM both.rs";
        let lines = unstaged_lines(input);
        // " M" → Y='M' → keep
        // "A " → Y=' ' → drop (staged-only)
        // "??" → Y='?' → keep
        // "MM" → Y='M' → keep
        assert_eq!(lines, vec![" M src/lib.rs", "?? scratch.txt", "MM both.rs"]);
    }

    #[test]
    fn unstaged_lines_empty_for_clean_tree() {
        assert!(unstaged_lines("").is_empty());
        assert!(unstaged_lines("A  staged.rs\nD  deleted.rs").is_empty());
    }

    #[test]
    fn is_bootstrapped_returns_true_when_no_githooks_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert!(is_bootstrapped(dir.path()));
    }

    #[test]
    fn workspace_groups_crates_prefix() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("crates/alpha")).unwrap();
        std::fs::write(
            dir.path().join("crates/alpha/Cargo.toml"),
            "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/alpha\"]\n",
        )
        .unwrap();

        let cfg = config::ProjectConfig {
            monorepo: true,
            ..Default::default()
        };
        let groups = build_workspace_groups(dir.path(), &cfg);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].label, "Crates");
        assert_eq!(groups[0].names, vec!["alpha"]);
    }

    #[test]
    fn scopes_line_none_when_no_scopes() {
        let cfg = config::ProjectConfig::default();
        let cc = build_commit_config(&cfg, &[]);
        assert!(cc.scopes_line.is_none());
    }

    #[test]
    fn scopes_line_auto_strict() {
        let cfg = config::ProjectConfig {
            scopes: config::ScopesConfig::Auto,
            strict: true,
            ..Default::default()
        };
        let cc = build_commit_config(&cfg, &[]);
        assert_eq!(
            cc.scopes_line.as_deref(),
            Some("from workspace (required, strict)")
        );
    }

    #[test]
    fn scopes_line_list_optional() {
        let cfg = config::ProjectConfig {
            scopes: config::ScopesConfig::List(vec!["api".into(), "cli".into()]),
            strict: false,
            ..Default::default()
        };
        let cc = build_commit_config(&cfg, &[]);
        assert_eq!(cc.scopes_line.as_deref(), Some("api, cli (optional)"));
    }

    #[test]
    fn refs_required_empty_when_not_configured() {
        let cfg = config::ProjectConfig::default();
        let cc = build_commit_config(&cfg, &[]);
        assert!(cc.refs_required.is_empty());
    }

    #[test]
    fn refs_required_populated_from_config() {
        let cfg = config::ProjectConfig {
            refs_required: vec!["feat".into(), "fix".into()],
            ..Default::default()
        };
        let cc = build_commit_config(&cfg, &[]);
        assert_eq!(cc.refs_required, vec!["feat", "fix"]);
    }

    #[test]
    fn classify_file_docs() {
        assert_eq!(classify_file("README.md"), "docs");
        assert_eq!(classify_file("CONTRIBUTING.md"), "docs");
        assert_eq!(classify_file("docs/guide.rst"), "docs");
        assert_eq!(classify_file("AGENTS.md"), "docs");
    }

    #[test]
    fn classify_file_test() {
        assert_eq!(classify_file("tests/lib_test.rs"), "test");
        assert_eq!(classify_file("spec/integration.rs"), "test");
        assert_eq!(classify_file("src/lib_test.rs"), "test");
    }

    #[test]
    fn classify_file_code() {
        assert_eq!(classify_file("src/lib.rs"), "code");
        assert_eq!(classify_file("index.ts"), "code");
        assert_eq!(classify_file("main.py"), "code");
    }

    #[test]
    fn classify_file_chore() {
        assert_eq!(classify_file("CHANGELOG.md"), "chore");
        assert_eq!(classify_file("Cargo.toml"), "chore");
        assert_eq!(classify_file("package.json"), "chore");
        assert_eq!(classify_file("skills/std-commit.md"), "chore");
    }

    #[test]
    fn suggest_type_all_docs() {
        let files = vec!["README.md".to_string(), "docs/guide.rst".to_string()];
        let allowed = vec!["docs".to_string(), "feat".to_string()];
        assert_eq!(suggest_type(&files, &allowed), Some("docs".to_string()));
    }

    #[test]
    fn suggest_type_all_test() {
        let files = vec![
            "tests/lib_test.rs".to_string(),
            "spec/integration.rs".to_string(),
        ];
        let allowed = vec!["test".to_string(), "feat".to_string()];
        assert_eq!(suggest_type(&files, &allowed), Some("test".to_string()));
    }

    #[test]
    fn suggest_type_all_chore() {
        let files = vec!["Cargo.toml".to_string(), "package.json".to_string()];
        let allowed = vec!["chore".to_string(), "feat".to_string()];
        assert_eq!(suggest_type(&files, &allowed), Some("chore".to_string()));
    }

    #[test]
    fn suggest_type_code_with_tests() {
        let files = vec!["src/lib.rs".to_string(), "tests/lib_test.rs".to_string()];
        let allowed = vec!["feat".to_string(), "fix".to_string()];
        assert_eq!(
            suggest_type(&files, &allowed),
            Some("feat or fix".to_string())
        );
    }

    #[test]
    fn suggest_type_code_only() {
        let files = vec!["src/lib.rs".to_string()];
        let allowed = vec!["fix".to_string(), "refactor".to_string()];
        assert_eq!(
            suggest_type(&files, &allowed),
            Some("fix or refactor".to_string())
        );
    }

    #[test]
    fn suggest_type_mixed_no_suggestion() {
        let files = vec!["src/lib.rs".to_string(), "README.md".to_string()];
        let allowed = vec!["feat".to_string(), "docs".to_string()];
        assert_eq!(suggest_type(&files, &allowed), None);
    }

    #[test]
    fn suggest_type_empty_files() {
        let files: Vec<String> = vec![];
        let allowed = vec!["feat".to_string()];
        assert_eq!(suggest_type(&files, &allowed), None);
    }

    #[test]
    fn suggest_type_respects_allowed_types() {
        let files = vec!["src/lib.rs".to_string(), "tests/lib_test.rs".to_string()];
        let allowed = vec!["fix".to_string()]; // feat not allowed
        assert_eq!(suggest_type(&files, &allowed), Some("fix".to_string()));
    }

    #[test]
    fn render_unstaged_caps_at_five() {
        let lines: Vec<String> = (0..7).map(|i| format!("?? file{i}.txt")).collect();
        // capture stdout
        // We can't easily capture stdout in unit tests, so just verify the logic
        assert_eq!(lines.len(), 7);
        let shown = &lines[..lines.len().min(MAX_UNSTAGED_SHOWN)];
        assert_eq!(shown.len(), 5);
        let remaining = lines.len() - MAX_UNSTAGED_SHOWN;
        assert_eq!(remaining, 2);
    }
}
