//! `git std doctor` — repo health check.

use std::path::Path;

use crate::app::OutputFormat;
use crate::git::workdir;
use crate::ui;

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[allow(dead_code)] // Variants constructed by later stories (#323–#325); unused in skeleton.
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

pub struct Check {
    pub label: &'static str,
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

fn hooks_section(_root: &Path) -> Section {
    Section {
        name: "hooks",
        checks: vec![],
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
    let any_fail = sections.iter().any(|s| {
        s.checks
            .iter()
            .any(|c| matches!(c.status, CheckStatus::Fail))
    });
    println!("{{\"sections\":[]}}");
    if any_fail { 1 } else { 0 }
}
