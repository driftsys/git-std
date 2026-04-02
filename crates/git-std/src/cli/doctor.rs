//! `git std doctor` — single "show me everything" command.
//!
//! Three sections: **Status**, **Hooks**, **Configuration**.
//! Problems surface as hints at the bottom. Absorbs the old `config` command.

use std::path::Path;

use yansi::Paint;

use standard_githooks::{HookCommand, KNOWN_HOOKS, Prefix};

use crate::app::OutputFormat;
use crate::config::{self, ScopesConfig};
use crate::git::workdir;
use crate::ui;

// ── constants ────────────────────────────────────────────────────────────────

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Bump lifecycle hooks invoked directly by `git std bump` (not by git).
const LIFECYCLE_HOOKS: &[&str] = &["pre-bump", "post-version", "post-changelog", "post-bump"];

// ── Data model ───────────────────────────────────────────────────────────────

/// A collected hint to print after all sections.
struct Hint(String);

// ── Status section ────────────────────────────────────────────────────────────

struct ToolVersion {
    name: &'static str,
    version: Option<String>,
    /// Optional update notice: "update available: X.Y.Z"
    update_notice: Option<String>,
}

fn git_version() -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["--version"])
        .output()
        .ok()?;
    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout);
        // "git version 2.43.0" → "2.43.0"
        s.trim()
            .strip_prefix("git version ")
            .map(str::trim)
            .map(String::from)
    } else {
        None
    }
}

fn git_lfs_version() -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["lfs", "version"])
        .output()
        .ok()?;
    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout);
        // "git-lfs/3.4.1 ..." → "3.4.1"
        let first = s.split_whitespace().next()?;
        first
            .strip_prefix("git-lfs/")
            .map(str::trim)
            .map(String::from)
    } else {
        None
    }
}

fn has_lfs_in_gitattributes(root: &Path) -> bool {
    let path = root.join(".gitattributes");
    std::fs::read_to_string(path)
        .map(|c| c.lines().any(|l| l.contains("filter=lfs")))
        .unwrap_or(false)
}

fn read_update_cache() -> Option<String> {
    // Read cached latest version from XDG_CONFIG_HOME/git-std/update-check.json
    // (written by the background update check from PR #383).
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("HOME").ok().map(|h| format!("{h}/.config")))?;
    let path = std::path::PathBuf::from(base).join("git-std/update-check.json");
    let data = std::fs::read_to_string(path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&data).ok()?;
    val.get("latest_version")?.as_str().map(String::from)
}

fn build_status_section(root: &Path) -> (Vec<ToolVersion>, Vec<Hint>) {
    let mut tools = Vec::new();
    let mut hints = Vec::new();

    // git
    let git_ver = git_version();
    if git_ver.is_none() {
        hints.push(Hint(
            "git not found — install from https://git-scm.com".to_owned(),
        ));
    }
    tools.push(ToolVersion {
        name: "git",
        version: git_ver,
        update_notice: None,
    });

    // git-lfs (only when .gitattributes has filter=lfs)
    if has_lfs_in_gitattributes(root) {
        let lfs_ver = git_lfs_version();
        if lfs_ver.is_none() {
            hints.push(Hint(
                "git-lfs not found — required by .gitattributes".to_owned(),
            ));
        }
        tools.push(ToolVersion {
            name: "git-lfs",
            version: lfs_ver,
            update_notice: None,
        });
    }

    // git-std with optional update notice
    let update_notice = read_update_cache().and_then(|latest| {
        let cur = semver::Version::parse(CURRENT_VERSION).ok()?;
        let lat = semver::Version::parse(&latest).ok()?;
        if lat > cur {
            Some(format!("update available: {latest}"))
        } else {
            None
        }
    });

    tools.push(ToolVersion {
        name: "git-std",
        version: Some(CURRENT_VERSION.to_owned()),
        update_notice,
    });

    (tools, hints)
}

// ── Hooks section ─────────────────────────────────────────────────────────────

/// A single hook entry for display.
struct HookEntry {
    name: &'static str,
    enabled: bool,
    commands: Vec<HookCommand>,
}

fn build_hooks_section(root: &Path) -> (Vec<HookEntry>, Vec<Hint>) {
    let hooks_dir = root.join(".githooks");
    let mut hints = Vec::new();

    // Health checks — only emit hints when something is wrong.
    if !hooks_dir.exists() {
        hints.push(Hint(".githooks/ not found — run 'git std init'".to_owned()));
    } else {
        // Check core.hooksPath is set to .githooks/
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
        match hooks_path.as_deref() {
            Some(".githooks") => {}
            Some(other) => hints.push(Hint(format!(
                "core.hooksPath is '{other}', expected '.githooks' — run 'git std init'"
            ))),
            None => hints.push(Hint(
                "core.hooksPath not configured — run 'git std init'".to_owned(),
            )),
        }

        // Check shim executability for enabled hooks.
        for hook_name in KNOWN_HOOKS {
            let shim = hooks_dir.join(hook_name);
            if shim.exists() {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = std::fs::metadata(&shim) {
                    if meta.permissions().mode() & 0o111 == 0 {
                        hints.push(Hint(format!("{hook_name} shim is not executable")));
                    }
                }
            }
        }
    }

    // Build hook entries from all known git hooks + lifecycle hooks.
    let all_hooks: Vec<&str> = KNOWN_HOOKS
        .iter()
        .copied()
        .chain(LIFECYCLE_HOOKS.iter().copied())
        .collect();

    let entries = all_hooks
        .iter()
        .filter_map(|hook_name| {
            let template = hooks_dir.join(format!("{hook_name}.hooks"));
            if !template.exists() {
                return None; // skip hooks with no .hooks file
            }
            let content = std::fs::read_to_string(&template).unwrap_or_default();
            let commands = standard_githooks::parse(&content);
            // Lifecycle hooks have no shim — always show as n/a (not enabled/disabled).
            let is_lifecycle = LIFECYCLE_HOOKS.contains(hook_name);
            let enabled = !is_lifecycle && hooks_dir.join(hook_name).exists();
            Some(HookEntry {
                name: hook_name,
                enabled,
                commands,
            })
        })
        .collect();

    (entries, hints)
}

// ── Configuration section ─────────────────────────────────────────────────────

/// A single configuration row.
struct ConfigRow {
    key: &'static str,
    value: String,
    /// `true` = came from file (bold), `false` = default (plain/dim).
    from_file: bool,
}

fn build_config_section(root: &Path) -> (Vec<ConfigRow>, Vec<Hint>) {
    let mut hints = Vec::new();

    // Bootstrap health check: .git-blame-ignore-revs present but blame.ignoreRevsFile not set.
    let ignore_revs = root.join(".git-blame-ignore-revs");
    if ignore_revs.exists() {
        let configured = std::process::Command::new("git")
            .current_dir(root)
            .args(["config", "blame.ignoreRevsFile"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_owned())
                } else {
                    None
                }
            });
        if configured.as_deref() != Some(".git-blame-ignore-revs") {
            hints.push(Hint(
                ".git-blame-ignore-revs found but blame.ignoreRevsFile not configured \
                 — run 'git std bootstrap'"
                    .to_owned(),
            ));
        }
    }

    // Try to load config file to get both effective config and raw table.
    // We need to detect parse errors to show as hints.
    let config_path = root.join(".git-std.toml");
    let has_file = config_path.exists();

    // Check for invalid TOML — if so, add a hint but still display defaults.
    // We parse the file ourselves to avoid load_with_raw emitting an eprintln!
    // warning to stderr (which would break JSON output).
    let mut toml_is_valid = true;
    if has_file {
        let content = std::fs::read_to_string(&config_path).unwrap_or_default();
        if let Err(e) = content.parse::<toml::Table>() {
            hints.push(Hint(format!(".git-std.toml invalid: {e}")));
            toml_is_valid = false;
        }
    }

    // When TOML is invalid we use defaults directly to avoid the warning
    // eprintln! inside load_with_raw/load. When valid (or absent) call normally.
    let (cfg, raw) = if toml_is_valid {
        config::load_with_raw(root)
    } else {
        // Return defaults without any stderr output.
        (config::ProjectConfig::default(), None)
    };
    let raw = raw.unwrap_or_default();

    let has_key = |key: &str| has_file && raw.contains_key(key);
    let has_versioning_key = |key: &str| {
        has_file
            && raw
                .get("versioning")
                .and_then(|v| v.as_table())
                .is_some_and(|t| t.contains_key(key))
    };
    let has_changelog_key = |key: &str| {
        has_file
            && raw
                .get("changelog")
                .and_then(|v| v.as_table())
                .is_some_and(|t| t.contains_key(key))
    };

    let default_cl = standard_changelog::ChangelogConfig::default();

    let scheme_label = match cfg.scheme {
        config::Scheme::Semver => "semver",
        config::Scheme::Calver => "calver",
        config::Scheme::Patch => "patch",
    };

    let scopes_value = match &cfg.scopes {
        ScopesConfig::None => "none".to_owned(),
        ScopesConfig::Auto => "auto".to_owned(),
        ScopesConfig::List(list) => format!("[{}]", list.len()),
    };

    let types_value = format!("[{}]", cfg.types.len());

    let hidden_value = {
        let h = cfg.changelog.hidden.as_ref().unwrap_or(&default_cl.hidden);
        format!("[{}]", h.len())
    };

    let rows = vec![
        ConfigRow {
            key: "scheme",
            value: scheme_label.to_owned(),
            from_file: has_key("scheme"),
        },
        ConfigRow {
            key: "strict",
            value: cfg.strict.to_string(),
            from_file: has_key("strict"),
        },
        ConfigRow {
            key: "scopes",
            value: scopes_value,
            from_file: has_key("scopes"),
        },
        ConfigRow {
            key: "tag_prefix",
            value: cfg.versioning.tag_prefix.clone(),
            from_file: has_versioning_key("tag_prefix"),
        },
        ConfigRow {
            key: "prerelease_tag",
            value: cfg.versioning.prerelease_tag.clone(),
            from_file: has_versioning_key("prerelease_tag"),
        },
        ConfigRow {
            key: "calver_format",
            value: cfg.versioning.calver_format.clone(),
            from_file: has_versioning_key("calver_format"),
        },
        ConfigRow {
            key: "types",
            value: types_value,
            from_file: has_key("types"),
        },
        ConfigRow {
            key: "hidden",
            value: hidden_value,
            from_file: has_changelog_key("hidden"),
        },
    ];

    (rows, hints)
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Run `git std doctor`. Returns the process exit code.
pub fn run(cwd: &Path, format: OutputFormat) -> i32 {
    let root = match workdir(cwd) {
        Ok(p) => p,
        Err(_) => {
            ui::error("not a git repository");
            return 2;
        }
    };

    // Resolve the actual repo root for config/hooks (handles worktrees + subfolders).
    let repo_root = root.clone();

    let (status_tools, status_hints) = build_status_section(&repo_root);
    let (hooks, hooks_hints) = build_hooks_section(&repo_root);
    let (config_rows, config_hints) = build_config_section(&repo_root);

    // Collect all hints
    let mut all_hints: Vec<Hint> = Vec::new();
    all_hints.extend(status_hints);
    all_hints.extend(hooks_hints);
    all_hints.extend(config_hints);

    if format == OutputFormat::Json {
        return render_json(&status_tools, &hooks, &config_rows, &all_hints);
    }

    render_text(&status_tools, &hooks, &config_rows, &all_hints)
}

// ── Text rendering ────────────────────────────────────────────────────────────

fn render_text(
    tools: &[ToolVersion],
    hooks: &[HookEntry],
    config_rows: &[ConfigRow],
    hints: &[Hint],
) -> i32 {
    // Status section
    ui::blank();
    ui::info("Status");
    for tool in tools {
        match &tool.version {
            Some(ver) => {
                if let Some(notice) = &tool.update_notice {
                    ui::detail(&format!("{} {} ({})", tool.name, ver, notice));
                } else {
                    ui::detail(&format!("{} {}", tool.name, ver));
                }
            }
            None => {
                ui::detail(&format!("{} (not found)", tool.name));
            }
        }
    }

    // Hooks section
    if !hooks.is_empty() {
        ui::blank();
        ui::info("Hooks");
        for hook in hooks {
            let header = if hook.enabled {
                hook.name.to_owned()
            } else {
                format!("{} (disabled)", hook.name)
            };
            ui::detail(&header);
            for cmd in &hook.commands {
                let sigil = match cmd.prefix {
                    Prefix::FailFast => "!",
                    Prefix::Advisory => "?",
                    Prefix::Fix => "~",
                    Prefix::Default => " ",
                };
                // 6-space indent for commands within a hook
                eprintln!("      {}  {}", sigil, cmd.command);
            }
        }
    }

    // Configuration section
    ui::blank();
    ui::info("Configuration");

    // Compute column alignment
    let key_width = config_rows.iter().map(|r| r.key.len()).max().unwrap_or(0);

    for row in config_rows {
        let key_padded = format!("{:<width$}", row.key, width = key_width);
        if row.from_file {
            // Bold for explicit file values
            eprintln!("    {}   {}", key_padded.bold(), row.value.bold());
        } else {
            // Dim for defaults
            eprintln!("    {}   {}", key_padded.dim(), row.value.dim());
        }
    }

    // Hints at the bottom
    if !hints.is_empty() {
        ui::blank();
        for hint in hints {
            ui::hint(&hint.0);
        }
    }

    if hints.is_empty() { 0 } else { 1 }
}

// ── JSON rendering ────────────────────────────────────────────────────────────

fn render_json(
    tools: &[ToolVersion],
    hooks: &[HookEntry],
    config_rows: &[ConfigRow],
    hints: &[Hint],
) -> i32 {
    let has_problems = !hints.is_empty();

    let status_json: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| {
            let mut obj = serde_json::json!({
                "name": t.name,
            });
            if let Some(ref ver) = t.version {
                obj["version"] = serde_json::Value::String(ver.clone());
            }
            if let Some(ref notice) = t.update_notice {
                obj["update_notice"] = serde_json::Value::String(notice.clone());
            }
            obj
        })
        .collect();

    let hooks_json: Vec<serde_json::Value> = hooks
        .iter()
        .map(|h| {
            let commands_json: Vec<serde_json::Value> = h
                .commands
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "command": c.command,
                        "sigil": match c.prefix {
                            Prefix::FailFast => "!",
                            Prefix::Advisory => "?",
                            Prefix::Fix => "~",
                            Prefix::Default => " ",
                        },
                    })
                })
                .collect();
            serde_json::json!({
                "name": h.name,
                "enabled": h.enabled,
                "commands": commands_json,
            })
        })
        .collect();

    let config_json: Vec<serde_json::Value> = config_rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "key": r.key,
                "value": r.value,
                "source": if r.from_file { "file" } else { "default" },
            })
        })
        .collect();

    let hints_json: Vec<serde_json::Value> = hints
        .iter()
        .map(|h| serde_json::Value::String(h.0.clone()))
        .collect();

    let output = serde_json::json!({
        "status": if has_problems { "fail" } else { "pass" },
        "sections": {
            "status": status_json,
            "hooks": hooks_json,
            "configuration": config_json,
        },
        "hints": hints_json,
    });

    println!("{output}");
    if has_problems { 1 } else { 0 }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_status_shows_git_std_version() {
        // Verify git-std always appears with its version
        let dir = tempfile::tempdir().unwrap();
        let (tools, _hints) = build_status_section(dir.path());
        let git_std = tools.iter().find(|t| t.name == "git-std");
        assert!(git_std.is_some(), "git-std must appear in status");
        assert_eq!(git_std.unwrap().version.as_deref(), Some(CURRENT_VERSION));
    }

    #[test]
    fn build_status_skips_lfs_without_gitattributes() {
        let dir = tempfile::tempdir().unwrap();
        let (tools, hints) = build_status_section(dir.path());
        assert!(
            tools.iter().all(|t| t.name != "git-lfs"),
            "git-lfs should not appear without .gitattributes filter=lfs"
        );
        assert!(
            hints.iter().all(|h| !h.0.contains("lfs")),
            "no lfs hint without filter=lfs"
        );
    }

    #[test]
    fn build_status_includes_lfs_when_gitattributes_has_filter() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitattributes"), "*.bin filter=lfs\n").unwrap();
        let (tools, _hints) = build_status_section(dir.path());
        assert!(
            tools.iter().any(|t| t.name == "git-lfs"),
            "git-lfs should appear when .gitattributes has filter=lfs"
        );
    }

    #[test]
    fn build_hooks_section_shows_only_hooks_with_files() {
        let dir = tempfile::tempdir().unwrap();
        let hooks_dir = dir.path().join(".githooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();
        std::fs::write(hooks_dir.join("pre-commit.hooks"), "! cargo fmt --check\n").unwrap();

        let (entries, _hints) = build_hooks_section(dir.path());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "pre-commit");
        assert_eq!(entries[0].commands.len(), 1);
    }

    #[test]
    fn build_hooks_section_disabled_when_no_shim() {
        let dir = tempfile::tempdir().unwrap();
        let hooks_dir = dir.path().join(".githooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();
        std::fs::write(hooks_dir.join("pre-commit.hooks"), "! cargo fmt\n").unwrap();
        // No shim file → disabled

        let (entries, _hints) = build_hooks_section(dir.path());
        assert!(!entries[0].enabled);
    }

    #[test]
    fn build_hooks_section_enabled_when_shim_exists() {
        let dir = tempfile::tempdir().unwrap();
        let hooks_dir = dir.path().join(".githooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();
        std::fs::write(hooks_dir.join("pre-commit.hooks"), "! cargo fmt\n").unwrap();
        std::fs::write(hooks_dir.join("pre-commit"), "#!/bin/sh\n").unwrap();

        let (entries, _hints) = build_hooks_section(dir.path());
        assert!(entries[0].enabled);
    }

    #[test]
    fn build_config_section_defaults_have_from_file_false() {
        let dir = tempfile::tempdir().unwrap();
        let (rows, hints) = build_config_section(dir.path());
        assert!(hints.is_empty(), "no hints for valid config");
        let scheme = rows.iter().find(|r| r.key == "scheme").unwrap();
        assert!(!scheme.from_file, "scheme should be default when no file");
        assert_eq!(scheme.value, "semver");
    }

    #[test]
    fn build_config_section_file_values_have_from_file_true() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".git-std.toml"), "scheme = \"calver\"\n").unwrap();
        let (rows, _hints) = build_config_section(dir.path());
        let scheme = rows.iter().find(|r| r.key == "scheme").unwrap();
        assert!(scheme.from_file, "scheme from file should be true");
        assert_eq!(scheme.value, "calver");
    }

    #[test]
    fn build_config_section_invalid_toml_produces_hint() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".git-std.toml"), "[[invalid\n").unwrap();
        let (_rows, hints) = build_config_section(dir.path());
        assert!(
            hints.iter().any(|h| h.0.contains(".git-std.toml invalid")),
            "should produce hint for invalid TOML"
        );
    }
}
