mod enable;
mod list;
mod run;
mod stash;

pub use enable::{disable, enable};
pub use list::list;
pub use run::run;

use std::path::{Path, PathBuf};

use standard_githooks::HookCommand;

use crate::{git, ui};

/// Resolve the `.githooks/` directory from the repository root.
///
/// Returns the absolute path to `.githooks/` or prints an error and
/// returns `Err(1)` when not inside a git repository.
pub(super) fn hooks_dir() -> Result<PathBuf, i32> {
    let cwd = std::env::current_dir().unwrap_or_default();
    match git::workdir(&cwd) {
        Ok(root) => Ok(root.join(".githooks")),
        Err(_) => {
            ui::error("not inside a git repository");
            Err(1)
        }
    }
}

/// Returns true if a hook's shim is currently active (named exactly as the hook).
pub(super) fn is_enabled(hooks_dir: &Path, hook_name: &str) -> bool {
    hooks_dir.join(hook_name).exists()
}

/// Read and parse the `.githooks/<hook>.hooks` file.
///
/// Returns `Ok(commands)` on success, or `Err(exit_code)` if the file
/// cannot be read.
pub(super) fn read_and_parse_hooks(
    hooks_dir: &Path,
    hook_name: &str,
) -> Result<Vec<HookCommand>, i32> {
    let hooks_file = hooks_dir.join(format!("{hook_name}.hooks"));
    let content = match std::fs::read_to_string(&hooks_file) {
        Ok(c) => c,
        Err(e) => {
            ui::error(&format!("cannot read {}: {e}", hooks_file.display()));
            return Err(2);
        }
    };
    Ok(standard_githooks::parse(&content))
}
