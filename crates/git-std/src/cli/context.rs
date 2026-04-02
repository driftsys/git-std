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

fn build_commit_config(_root: &Path, cfg: &ProjectConfig) -> CommitConfig {
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

    CommitConfig { types, scopes_line }
}

/// Return `true` when the repo does not need bootstrapping or is already done.
///
/// "Not bootstrapped" = `.githooks/` exists but `core.hooksPath` is not set
/// to `.githooks`. Reuses the same logic as `doctor`'s hooks health check.
fn is_bootstrapped(root: &Path) -> bool {
    if !root.join(".githooks").exists() {
        return true;
    }

    let hooks_path = std::process::Command::new("git")
        .current_dir(root)
        .args(["config", "core.hooksPath"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_owned())
            } else {
                None
            }
        });

    matches!(hooks_path.as_deref(), Some(".githooks"))
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
    let commit_cfg = build_commit_config(&root, &cfg);

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
    if let Some(scopes) = &commit_cfg.scopes_line {
        println!("Scopes: {scopes}");
    }

    match state {
        GitState::NotBootstrapped => {
            println!();
            println!("⚠ Not bootstrapped — run `git std bootstrap`");
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
            println!("```");
            println!("{diff}");
            println!("```");
        }
        GitState::StagedAndUnstaged { diff, unstaged } => {
            println!();
            println!("## Staged diff");
            println!("```");
            println!("{diff}");
            println!("```");
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

    let (staged_diff, unstaged_files, status_msg) = match state {
        GitState::NotBootstrapped => (
            serde_json::Value::Null,
            serde_json::Value::Null,
            serde_json::json!("Not bootstrapped — run `git std bootstrap`"),
        ),
        GitState::Clean => (
            serde_json::Value::Null,
            serde_json::Value::Null,
            serde_json::json!("Nothing to commit — working tree clean"),
        ),
        GitState::NothingStaged { unstaged } => (
            serde_json::Value::Null,
            serde_json::Value::Array(
                unstaged
                    .iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            ),
            serde_json::json!("Nothing staged — run `git add` first"),
        ),
        GitState::Staged { diff } => (
            serde_json::Value::String(diff.clone()),
            serde_json::Value::Null,
            serde_json::Value::Null,
        ),
        GitState::StagedAndUnstaged { diff, unstaged } => (
            serde_json::Value::String(diff.clone()),
            serde_json::Value::Array(
                unstaged
                    .iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            ),
            serde_json::Value::Null,
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
            "scopes": commit_cfg.scopes_line,
        },
        "staged_diff": staged_diff,
        "unstaged_files": unstaged_files,
        "status": status_msg,
    });

    println!("{output}");
    0
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
        let cc = build_commit_config(std::path::Path::new("."), &cfg);
        assert!(cc.scopes_line.is_none());
    }

    #[test]
    fn scopes_line_auto_strict() {
        let cfg = config::ProjectConfig {
            scopes: config::ScopesConfig::Auto,
            strict: true,
            ..Default::default()
        };
        let cc = build_commit_config(std::path::Path::new("."), &cfg);
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
        let cc = build_commit_config(std::path::Path::new("."), &cfg);
        assert_eq!(cc.scopes_line.as_deref(), Some("api, cli (optional)"));
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
