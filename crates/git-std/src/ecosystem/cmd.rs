//! Shared helper for running ecosystem CLI tools.

use std::path::Path;
use std::process::{Command, ExitStatus};

/// Run an external tool in `root`.
///
/// Returns `Err` if the binary is not found on PATH (spawn failure).
/// Returns `Ok(status)` otherwise — the caller decides how to interpret
/// the exit code.
pub fn run_tool(root: &Path, tool: &str, args: &[&str]) -> std::io::Result<ExitStatus> {
    Command::new(tool).args(args).current_dir(root).status()
}
