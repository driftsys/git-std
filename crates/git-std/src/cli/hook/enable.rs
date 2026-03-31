use standard_githooks::KNOWN_HOOKS;

use crate::ui;

/// Run the `hook enable <hook>` subcommand. Returns the process exit code.
pub fn enable(hook_name: &str) -> i32 {
    if !KNOWN_HOOKS.contains(&hook_name) {
        ui::error(&format!(
            "unknown hook '{hook_name}' — known hooks: {}",
            KNOWN_HOOKS.join(", ")
        ));
        return 1;
    }

    let hooks_dir = match super::hooks_dir() {
        Ok(d) => d,
        Err(code) => return code,
    };
    let active_path = hooks_dir.join(hook_name);
    let off_path = hooks_dir.join(format!("{hook_name}.off"));

    if active_path.exists() {
        ui::info(&format!("{} {hook_name} is already enabled", ui::warn()));
        return 0;
    }

    if !off_path.exists() {
        ui::error(&format!(
            "{hook_name}.off not found — run 'git std hook install' first"
        ));
        return 1;
    }

    if let Err(e) = std::fs::rename(&off_path, &active_path) {
        ui::error(&format!("cannot enable {hook_name}: {e}"));
        return 1;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        let _ = std::fs::set_permissions(&active_path, perms);
    }

    ui::info(&format!("{}  {hook_name} enabled", ui::pass()));
    0
}

/// Run the `hook disable <hook>` subcommand. Returns the process exit code.
pub fn disable(hook_name: &str) -> i32 {
    if !KNOWN_HOOKS.contains(&hook_name) {
        ui::error(&format!(
            "unknown hook '{hook_name}' — known hooks: {}",
            KNOWN_HOOKS.join(", ")
        ));
        return 1;
    }

    let hooks_dir = match super::hooks_dir() {
        Ok(d) => d,
        Err(code) => return code,
    };
    let active_path = hooks_dir.join(hook_name);
    let off_path = hooks_dir.join(format!("{hook_name}.off"));

    if off_path.exists() {
        ui::info(&format!("{} {hook_name} is already disabled", ui::warn()));
        return 0;
    }

    if !active_path.exists() {
        ui::error(&format!(
            "{hook_name} not found — run 'git std hook install' first"
        ));
        return 1;
    }

    if let Err(e) = std::fs::rename(&active_path, &off_path) {
        ui::error(&format!("cannot disable {hook_name}: {e}"));
        return 1;
    }

    ui::info(&format!("{}  {hook_name} disabled", ui::pass()));
    0
}
