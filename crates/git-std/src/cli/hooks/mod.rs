mod enable;
mod install;
mod list;
mod run;

pub use enable::{disable, enable};
pub use install::install;
pub use list::list;
pub use run::run;

use standard_githooks::HookCommand;

use crate::ui;

/// Returns true if a hook's shim is currently active (named exactly as the hook).
pub(super) fn is_enabled(hooks_dir: &std::path::Path, hook_name: &str) -> bool {
    hooks_dir.join(hook_name).exists()
}

/// Read and parse the `.githooks/<hook>.hooks` file.
///
/// Returns `Ok(commands)` on success, or `Err(exit_code)` if the file
/// cannot be read.
pub(super) fn read_and_parse_hooks(hook_name: &str) -> Result<Vec<HookCommand>, i32> {
    let hooks_file = format!(".githooks/{hook_name}.hooks");
    let content = match std::fs::read_to_string(&hooks_file) {
        Ok(c) => c,
        Err(e) => {
            ui::error(&format!("cannot read {hooks_file}: {e}"));
            return Err(2);
        }
    };
    Ok(standard_githooks::parse(&content))
}
