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

fn bootstrap_section(_root: &Path) -> Section {
    Section {
        name: "bootstrap",
        checks: vec![],
    }
}

fn config_section(_root: &Path) -> Section {
    Section {
        name: "config",
        checks: vec![],
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
    // Stub output — replaced by story #326.
    let any_fail = sections
        .iter()
        .any(|s| s.checks.iter().any(|c| matches!(c.status, CheckStatus::Fail)));
    println!("{{\"sections\":[]}}");
    if any_fail { 1 } else { 0 }
}
