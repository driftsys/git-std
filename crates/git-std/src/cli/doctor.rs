//! `git std doctor` — repo health check.

use std::path::Path;

use standard_githooks::KNOWN_HOOKS;

use crate::app::OutputFormat;
use crate::git::workdir;
use crate::ui;

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

pub struct Check {
    pub label: String,
    pub status: CheckStatus,
    pub hint: Option<String>,
}

pub struct Section {
    pub name: &'static str,
    pub checks: Vec<Check>,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run `git std doctor`. Returns the process exit code.
pub fn run(cwd: &Path, format: OutputFormat) -> i32 {
    let root = match workdir(cwd) {
        Ok(p) => p,
        Err(_) => {
            ui::error("not a git repository");
            return 2;
        }
    };

    let sections = vec![
        hooks_section(&root),
        bootstrap_section(&root),
        config_section(&root),
    ];

    if format == OutputFormat::Json {
        return run_json(&sections);
    }

    print_sections(&sections)
}

// ---------------------------------------------------------------------------
// Sections (stubs — filled in by later stories)
// ---------------------------------------------------------------------------

fn hooks_section(root: &Path) -> Section {
    let mut checks: Vec<Check> = Vec::new();
    let githooks_dir = root.join(".githooks");

    // 1. .githooks/ directory exists
    let dir_exists = githooks_dir.is_dir();
    checks.push(Check {
        label: ".githooks/ directory exists".to_owned(),
        status: if dir_exists {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        hint: if dir_exists {
            None
        } else {
            Some("run 'git std hooks install'".to_owned())
        },
    });

    // 2. core.hooksPath is configured correctly
    let hooks_path_value = std::process::Command::new("git")
        .current_dir(root)
        .args(["config", "--get", "core.hooksPath"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).trim().to_owned())
            } else {
                None
            }
        });
    let hooks_path_ok = hooks_path_value.as_deref() == Some(".githooks");
    checks.push(Check {
        label: "core.hooksPath = .githooks".to_owned(),
        status: if hooks_path_ok {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail
        },
        hint: if hooks_path_ok {
            None
        } else {
            Some("run 'git std hooks install'".to_owned())
        },
    });

    // 3. Bootstrap shim present (warn if missing, not fail)
    let bootstrap_path = githooks_dir.join("bootstrap.hooks");
    let bootstrap_exists = bootstrap_path.exists();
    checks.push(Check {
        label: "bootstrap shim present (.githooks/bootstrap.hooks)".to_owned(),
        status: if bootstrap_exists {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        hint: None,
    });

    // 4. Hook shims are executable (unix only)
    #[cfg(unix)]
    if dir_exists {
        use std::os::unix::fs::PermissionsExt;

        for hook_name in KNOWN_HOOKS {
            let shim_path = githooks_dir.join(hook_name);
            if !shim_path.exists() {
                continue;
            }
            let is_executable = std::fs::metadata(&shim_path)
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false);
            if !is_executable {
                checks.push(Check {
                    label: format!("hook shim is executable: {hook_name}"),
                    status: CheckStatus::Fail,
                    hint: Some(format!("run 'chmod +x .githooks/{hook_name}'")),
                });
            }
        }
    }

    Section {
        name: "hooks",
        checks,
    }
}

fn bootstrap_section(root: &Path) -> Section {
    let mut checks: Vec<Check> = Vec::new();

    // 1. .gitattributes present (optional — warn if absent)
    let gitattributes_path = root.join(".gitattributes");
    let gitattributes_exists = gitattributes_path.exists();
    checks.push(Check {
        label: ".gitattributes present".to_owned(),
        status: if gitattributes_exists {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        hint: None,
    });

    // 2. LFS check — only if .gitattributes exists AND contains filter=lfs
    if gitattributes_exists {
        let has_lfs = std::fs::read_to_string(&gitattributes_path)
            .map(|c| c.lines().any(|l| l.contains("filter=lfs")))
            .unwrap_or(false);

        if has_lfs {
            let lfs_available = std::process::Command::new("git")
                .current_dir(root)
                .args(["lfs", "version"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            checks.push(Check {
                label: "git-lfs installed".to_owned(),
                status: if lfs_available {
                    CheckStatus::Pass
                } else {
                    CheckStatus::Fail
                },
                hint: if lfs_available {
                    None
                } else {
                    Some("install from https://git-lfs.github.com".to_owned())
                },
            });
        }
    }

    // 3. .git-blame-ignore-revs present (optional — warn if absent)
    let blame_ignore_path = root.join(".git-blame-ignore-revs");
    let blame_ignore_exists = blame_ignore_path.exists();
    checks.push(Check {
        label: ".git-blame-ignore-revs present".to_owned(),
        status: if blame_ignore_exists {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        hint: None,
    });

    // 4. blame.ignoreRevsFile configured — only when .git-blame-ignore-revs exists
    if blame_ignore_exists {
        let configured_value = std::process::Command::new("git")
            .current_dir(root)
            .args(["config", "--get", "blame.ignoreRevsFile"])
            .output()
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    Some(String::from_utf8_lossy(&out.stdout).trim().to_owned())
                } else {
                    None
                }
            });
        let absolute_form = root
            .join(".git-blame-ignore-revs")
            .to_string_lossy()
            .into_owned();
        let blame_ok = configured_value.as_deref() == Some(".git-blame-ignore-revs")
            || configured_value.as_deref() == Some(absolute_form.as_str());
        checks.push(Check {
            label: "blame.ignoreRevsFile = .git-blame-ignore-revs".to_owned(),
            status: if blame_ok {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            hint: if blame_ok {
                None
            } else {
                Some("run 'git std bootstrap'".to_owned())
            },
        });
    }

    Section {
        name: "bootstrap",
        checks,
    }
}

fn config_section(root: &Path) -> Section {
    let mut checks: Vec<Check> = Vec::new();

    // 1. .git-std.toml present (optional — absence is a warning, not a failure)
    let config_path = root.join(".git-std.toml");
    let config_exists = config_path.exists();
    checks.push(Check {
        label: ".git-std.toml present".to_owned(),
        status: if config_exists {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        hint: None,
    });

    // 2. Config is valid TOML — only when the file is present
    if config_exists {
        let content = std::fs::read_to_string(&config_path).unwrap_or_default();
        match toml::from_str::<toml::Value>(&content) {
            Ok(_) => checks.push(Check {
                label: ".git-std.toml is valid".to_owned(),
                status: CheckStatus::Pass,
                hint: None,
            }),
            Err(e) => checks.push(Check {
                label: ".git-std.toml parse error".to_owned(),
                status: CheckStatus::Fail,
                hint: Some(e.to_string()),
            }),
        }
    }

    Section {
        name: "config",
        checks,
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn print_sections(sections: &[Section]) -> i32 {
    let mut any_fail = false;

    for section in sections {
        if section.checks.is_empty() {
            continue;
        }
        ui::info(section.name);
        for check in &section.checks {
            let symbol = match check.status {
                CheckStatus::Pass => ui::pass().to_string(),
                CheckStatus::Warn => ui::warn().to_string(),
                CheckStatus::Fail => {
                    any_fail = true;
                    ui::fail().to_string()
                }
            };
            ui::detail(&format!("{symbol}  {}", check.label));
            if let Some(hint) = &check.hint {
                ui::detail(&format!("   hint: {hint}"));
            }
        }
        ui::blank();
    }

    if any_fail { 1 } else { 0 }
}

fn run_json(sections: &[Section]) -> i32 {
    let any_fail = sections.iter().any(|s| {
        s.checks
            .iter()
            .any(|c| matches!(c.status, CheckStatus::Fail))
    });

    let sections_json: Vec<serde_json::Value> = sections
        .iter()
        .map(|s| {
            let section_fail = s
                .checks
                .iter()
                .any(|c| matches!(c.status, CheckStatus::Fail));
            let checks_json: Vec<serde_json::Value> = s
                .checks
                .iter()
                .map(|c| {
                    let status = match c.status {
                        CheckStatus::Pass => "pass",
                        CheckStatus::Warn => "warn",
                        CheckStatus::Fail => "fail",
                    };
                    let mut obj = serde_json::json!({
                        "name": c.label,
                        "status": status,
                    });
                    if let Some(hint) = &c.hint {
                        obj["hint"] = serde_json::Value::String(hint.clone());
                    }
                    obj
                })
                .collect();

            serde_json::json!({
                "name": s.name,
                "status": if section_fail { "fail" } else { "pass" },
                "checks": checks_json,
            })
        })
        .collect();

    let output = serde_json::json!({
        "status": if any_fail { "fail" } else { "pass" },
        "sections": sections_json,
    });

    println!("{output}");
    if any_fail { 1 } else { 0 }
}
