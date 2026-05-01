//! `git std init` — single maintainer setup command.
//!
//! Consolidates `hook install` and `bootstrap install` into one command.
//! Steps:
//! 1. Create `.githooks/` directory.
//! 2. Set `core.hooksPath` to `.githooks`.
//! 3. Write `.hooks` templates (pre-commit, commit-msg, pre-push).
//! 4. Prompt which hooks to enable, write shims.
//! 5. Generate `./bootstrap` script.
//! 6. Generate `.githooks/bootstrap.hooks`.
//! 7. Create `.git-std.toml` with taplo schema directive (if absent).
//! 8. Append post-clone section to README/AGENTS (if found).
//! 9. Stage everything.

mod bootstrap;
mod scaffold;

use std::io::IsTerminal;
use std::process::Command;

use inquire::MultiSelect;
use yansi::Paint;

use standard_githooks::{KNOWN_HOOKS, generate_hooks_template, generate_shim};

use crate::ui;

use bootstrap::{append_bootstrap_marker, write_bootstrap_hooks, write_bootstrap_script};
use scaffold::{generate_lifecycle_hook_template, write_config_file};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const BOOTSTRAP_HOOKS_FILE: &str = ".githooks/bootstrap.hooks";
const BOOTSTRAP_SCRIPT: &str = "bootstrap";
const CONFIG_FILE: &str = ".git-std.toml";
const MARKER: &str = "<!-- git-std:bootstrap -->";

const LIFECYCLE_HOOKS: &[&str] = &["pre-bump", "post-version", "post-changelog", "post-bump"];

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

enum FileResult {
    Created,
    Skipped,
    Error,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run `git std init`. Returns the process exit code.
///
/// When `refresh` is true, only merges config defaults — skips hook setup,
/// bootstrap, and README markers.
pub fn run(force: bool, refresh: bool) -> i32 {
    let cwd = std::env::current_dir().unwrap_or_default();
    let root = match crate::git::workdir(&cwd) {
        Ok(r) => r,
        Err(_) => {
            ui::error("not inside a git repository");
            return 1;
        }
    };

    if refresh {
        return run_refresh(&root);
    }

    let hooks_dir = root.join(".githooks");

    // ── Step 1: ensure .githooks/ exists ────────────────────────────────────
    if let Err(e) = std::fs::create_dir_all(&hooks_dir) {
        ui::error(&format!("cannot create .githooks/: {e}"));
        return 1;
    }

    // ── Step 2: set core.hooksPath ───────────────────────────────────────────
    let status = Command::new("git")
        .args(["config", "core.hooksPath", ".githooks"])
        .status();

    match status {
        Ok(s) if s.success() => {
            ui::info(&format!("{}  git hooks configured", ui::pass()));
        }
        _ => {
            ui::error("failed to set core.hooksPath");
            ui::hint("ensure you are inside a git repository and have write access");
            return 1;
        }
    }

    // ── Step 3: write .hooks templates for every known hook ──────────────────
    for hook_name in KNOWN_HOOKS {
        let template_path = hooks_dir.join(format!("{hook_name}.hooks"));
        if !template_path.exists() || force {
            let content = generate_hooks_template(hook_name);
            if let Err(e) = std::fs::write(&template_path, &content) {
                ui::error(&format!("cannot write {}: {e}", template_path.display()));
                return 1;
            }
        }
    }

    // ── Step 3b: write lifecycle hook templates ──────────────────────────────
    for hook_name in LIFECYCLE_HOOKS {
        let template_path = hooks_dir.join(format!("{hook_name}.hooks"));
        if !template_path.exists() || force {
            let content = generate_lifecycle_hook_template(hook_name);
            if let Err(e) = std::fs::write(&template_path, &content) {
                ui::error(&format!("cannot write {}: {e}", template_path.display()));
                return 1;
            }
        }
    }

    // ── Step 4: determine which hooks to enable and write shims ─────────────
    let default_enabled = ["pre-commit", "commit-msg"];

    // Test/CI escape hatch — not a supported public API.
    // Accepts "all", "none", or a comma-separated list of hook names.
    let env_enable = std::env::var("GIT_STD_HOOKS_ENABLE").ok();
    let selected: Vec<&str> = if let Some(ref val) = env_enable {
        match val.to_lowercase().as_str() {
            "all" => KNOWN_HOOKS.to_vec(),
            "none" => vec![],
            _ => val
                .split(',')
                .map(|s| s.trim())
                .filter(|s| KNOWN_HOOKS.contains(s))
                .collect(),
        }
    } else if !std::io::stdin().is_terminal() {
        ui::error("interactive prompt requires a TTY");
        ui::hint("set GIT_STD_HOOKS_ENABLE to select hooks non-interactively");
        ui::hint("  GIT_STD_HOOKS_ENABLE=all            enable all hooks");
        ui::hint("  GIT_STD_HOOKS_ENABLE=pre-commit     comma-separated list");
        ui::hint("  GIT_STD_HOOKS_ENABLE=none            skip all hooks");
        return 1;
    } else {
        let options: Vec<&str> = KNOWN_HOOKS.to_vec();
        match MultiSelect::new("Which hooks do you want to enable?", options)
            .with_default(
                &KNOWN_HOOKS
                    .iter()
                    .enumerate()
                    .filter(|(_, h)| default_enabled.contains(h))
                    .map(|(i, _)| i)
                    .collect::<Vec<_>>(),
            )
            .prompt()
        {
            Ok(s) => s,
            Err(_) => {
                ui::error("init cancelled");
                return 1;
            }
        }
    };

    ui::blank();

    // Write shims — active for selected, .off for the rest
    for hook_name in KNOWN_HOOKS {
        let shim_content = generate_shim(hook_name);
        let enabled = selected.contains(hook_name);

        let active_path = hooks_dir.join(hook_name);
        let off_path = hooks_dir.join(format!("{hook_name}.off"));

        // Remove stale counterpart
        if enabled {
            let _ = std::fs::remove_file(&off_path);
        } else {
            let _ = std::fs::remove_file(&active_path);
        }

        let shim_path = if enabled { &active_path } else { &off_path };

        if let Err(e) = std::fs::write(shim_path, &shim_content) {
            ui::error(&format!("cannot write {}: {e}", shim_path.display()));
            return 1;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            if let Err(e) = std::fs::set_permissions(shim_path, perms) {
                ui::error(&format!(
                    "cannot set permissions on {}: {e}",
                    shim_path.display()
                ));
                return 1;
            }
        }

        let status_label = if enabled {
            "enabled ".green().to_string()
        } else {
            "disabled".dim().to_string()
        };

        ui::info(&format!("{}  {hook_name:<22} {status_label}", ui::pass()));
    }

    // ── Step 5 & 6: generate ./bootstrap and .githooks/bootstrap.hooks ───────
    let mut staged: Vec<&str> = Vec::new();

    match write_bootstrap_script(&root, force) {
        FileResult::Created => {
            staged.push(BOOTSTRAP_SCRIPT);
            ui::info(&format!("{}  {BOOTSTRAP_SCRIPT} created", ui::pass()));
        }
        FileResult::Skipped => {
            ui::info(&format!(
                "{}  {BOOTSTRAP_SCRIPT} already exists (use --force to overwrite)",
                ui::warn()
            ));
        }
        FileResult::Error => return 1,
    }

    match write_bootstrap_hooks(&root, force) {
        FileResult::Created => {
            staged.push(BOOTSTRAP_HOOKS_FILE);
            ui::info(&format!("{}  {BOOTSTRAP_HOOKS_FILE} created", ui::pass()));
        }
        FileResult::Skipped => {
            ui::info(&format!(
                "{}  {BOOTSTRAP_HOOKS_FILE} already exists (use --force to overwrite)",
                ui::warn()
            ));
        }
        FileResult::Error => return 1,
    }

    // ── Step 7: create or merge .git-std.toml ──────────────────────────────
    match write_config_file(&root, force) {
        FileResult::Created => {
            staged.push(CONFIG_FILE);
            ui::info(&format!("{}  {CONFIG_FILE} updated", ui::pass()));
        }
        FileResult::Skipped => {
            ui::info(&format!("{}  {CONFIG_FILE} up to date", ui::pass()));
        }
        FileResult::Error => return 1,
    }

    // ── Step 8: append post-clone section to README/AGENTS ───────────────────
    for doc in &["AGENTS.md", "README.md"] {
        let doc_path = root.join(doc);
        if doc_path.exists() {
            if let Err(e) = append_bootstrap_marker(&doc_path) {
                ui::error(&format!("cannot update {doc}: {e}"));
                return 1;
            }
            staged.push(doc);
        }
    }

    // ── Step 9: stage all created/modified files ────────────────────────────
    // Always stage .githooks/ (shims + templates) plus any other created files.
    let mut cmd = Command::new("git");
    cmd.current_dir(&root).arg("add").arg("--").arg(".githooks");
    for f in &staged {
        cmd.arg(f);
    }
    if let Err(e) = cmd.status() {
        ui::warning(&format!("git add failed: {e} — stage files manually"));
    }

    0
}

// ---------------------------------------------------------------------------
// Refresh mode
// ---------------------------------------------------------------------------

/// Refresh config without touching hooks or bootstrap.
fn run_refresh(root: &std::path::Path) -> i32 {
    let mut staged: Vec<&str> = Vec::new();

    // ── Merge config defaults ───────────────────────────────────────────────
    match write_config_file(root, false) {
        FileResult::Created => {
            staged.push(CONFIG_FILE);
            ui::info(&format!("{}  {CONFIG_FILE} updated", ui::pass()));
        }
        FileResult::Skipped => {
            ui::info(&format!("{}  {CONFIG_FILE} up to date", ui::pass()));
        }
        FileResult::Error => return 1,
    }

    // ── Stage updated files ─────────────────────────────────────────────────
    if !staged.is_empty() {
        let mut cmd = Command::new("git");
        cmd.current_dir(root).arg("add").arg("--");
        for f in &staged {
            cmd.arg(f);
        }
        if let Err(e) = cmd.status() {
            ui::warning(&format!("git add failed: {e} — stage files manually"));
        }
    }

    0
}
