use std::io::IsTerminal;
use std::process::Command;

use inquire::MultiSelect;
use yansi::Paint;

use standard_githooks::{KNOWN_HOOKS, generate_hooks_template, generate_shim};

use crate::ui;

/// Run the `hook install` subcommand. Returns the process exit code.
///
/// - Sets core.hooksPath
/// - Creates .githooks/ directory
/// - Writes .hooks template for every known hook (skips existing)
/// - Prompts which hooks to enable (interactive multi-select)
/// - Writes active shims for selected hooks, .off shims for the rest
pub fn install() -> i32 {
    // Resolve hooks dir from repo root so it works from subdirectories.
    let hooks_dir = match super::hooks_dir() {
        Ok(d) => d,
        Err(code) => return code,
    };

    // 1. Set core.hooksPath (git resolves relative to repo root)
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

    // 2. Ensure .githooks/ exists
    if !hooks_dir.exists()
        && let Err(e) = std::fs::create_dir_all(&hooks_dir)
    {
        ui::error(&format!("cannot create .githooks/: {e}"));
        return 1;
    }

    // 3. Write .hooks templates for every known hook (skip if already exists)
    for hook_name in KNOWN_HOOKS {
        let template_path = hooks_dir.join(format!("{hook_name}.hooks"));
        if !template_path.exists() {
            let content = generate_hooks_template(hook_name);
            if let Err(e) = std::fs::write(&template_path, &content) {
                ui::error(&format!("cannot write {}: {e}", template_path.display()));
                return 1;
            }
        }
    }

    // 4. Determine which hooks to enable — via env var (for tests/CI) or interactive prompt
    let default_enabled = ["pre-commit", "commit-msg"];

    // Test/CI escape hatch — not a supported public API.
    // Accepts "all", "none", or a comma-separated list of hook names.
    let env_enable = std::env::var("GIT_STD_HOOKS_ENABLE").ok();
    let selected: Vec<&str> = if let Some(ref val) = env_enable {
        // Comma-separated list of hook names, or "all" or "none"
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
                ui::error("install cancelled");
                return 1;
            }
        }
    };

    ui::blank();

    // 5. Write shims — active for selected, .off for the rest
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

    0
}
