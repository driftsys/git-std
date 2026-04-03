use std::process::Command;

use standard_githooks::{HookCommand, Prefix};

use crate::{git, ui};

/// Run a bump lifecycle hook file (`.githooks/<hook>.hooks`).
///
/// Returns `Ok(())` if all required commands passed (or the file does not
/// exist), or `Err(exit_code)` if a required command failed and the bump
/// should be aborted.
///
/// - Missing hook file → silently skipped (not an error).
/// - Advisory commands (`?`) → warning printed, bump continues.
/// - Required commands (`!` or default) → non-zero exit aborts bump.
///
/// `extra_args` are passed as positional arguments after the command
/// (used by `post-version` to pass the new version string).
pub(super) fn run_lifecycle_hook(hook_name: &str, extra_args: &[&str]) -> Result<(), i32> {
    // Honour the same skip-all-hooks escape hatch as `git std hook run`.
    if let Ok(val) = std::env::var("GIT_STD_SKIP_HOOKS")
        && (val == "1" || val.eq_ignore_ascii_case("true"))
    {
        return Ok(());
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let hooks_dir = match git::workdir(&cwd) {
        Ok(root) => root.join(".githooks"),
        Err(_) => {
            ui::error("bare repository not supported");
            return Err(1);
        }
    };

    let hooks_file = hooks_dir.join(format!("{hook_name}.hooks"));

    // Silently skip if the hook file does not exist.
    if !hooks_file.exists() {
        return Ok(());
    }

    let content = match std::fs::read_to_string(&hooks_file) {
        Ok(c) => c,
        Err(e) => {
            ui::error(&format!("cannot read {}: {e}", hooks_file.display()));
            return Err(2);
        }
    };

    let commands = standard_githooks::parse(&content);
    if commands.is_empty() {
        return Ok(());
    }

    for cmd in &commands {
        let exit_code = execute_hook_command(cmd, extra_args);
        let success = exit_code == Some(0);
        let is_advisory = cmd.prefix == Prefix::Advisory;

        if success {
            ui::info(&format!("{} {}", ui::pass(), cmd.command));
        } else if is_advisory {
            let info = match exit_code {
                Some(code) => format!("(advisory, exit {code})"),
                None => "(advisory, killed)".to_string(),
            };
            ui::info(&format!("{} {} {}", ui::warn(), cmd.command, info));
        } else {
            let info = match exit_code {
                Some(code) => format!("(exit {code})"),
                None => "(killed)".to_string(),
            };
            ui::error(&format!("hook command failed: {} {info}", cmd.command));
            ui::hint(&format!(
                "to disable this command: comment it out in .githooks/{hook_name}.hooks"
            ));
            return Err(1);
        }
    }

    Ok(())
}

/// Execute a single hook command and return its exit code.
fn execute_hook_command(cmd: &HookCommand, extra_args: &[&str]) -> Option<i32> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(&cmd.command)
        .arg("_")
        .args(extra_args)
        .status();

    match status {
        Ok(s) => s.code(),
        Err(_) => Some(127),
    }
}
