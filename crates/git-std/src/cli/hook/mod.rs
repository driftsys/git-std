mod enable;
mod failure;
mod list;
mod output;
mod run;
mod setup;
mod stash;

pub use enable::{disable, enable};
pub use list::list;
pub use run::run;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use standard_githooks::HookCommand;

use crate::{
    app::OutputFormat,
    contract::{Diagnostic, print_json_error},
};
use crate::{git, ui};

/// Execute a shell command via `sh -c`, passing extra positional args.
///
/// Returns the exit code, or `Some(127)` on spawn failure.
pub(crate) fn exec_sh(command: &str, args: &[impl AsRef<std::ffi::OsStr>]) -> Option<i32> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(command)
        .arg("_")
        .args(args)
        .status();
    match status {
        Ok(s) => s.code(),
        Err(_) => Some(127),
    }
}

/// Execute a shell command with bytes supplied on standard input.
pub(crate) fn exec_sh_with_stdin(
    command: &str,
    args: &[impl AsRef<std::ffi::OsStr>],
    stdin: &[u8],
) -> Option<i32> {
    let mut child = match Command::new("sh")
        .arg("-c")
        .arg(command)
        .arg("_")
        .args(args)
        .stdin(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return Some(127),
    };
    let Some(mut pipe) = child.stdin.take() else {
        return Some(127);
    };
    let write_failed = pipe
        .write_all(stdin)
        .is_err_and(|error| error.kind() != std::io::ErrorKind::BrokenPipe);
    drop(pipe);
    let exit_code = child.wait().ok().and_then(|status| status.code());
    if write_failed { Some(127) } else { exit_code }
}

/// Execute a shell command via `sh -c`, capturing stdout+stderr.
///
/// Returns `(exit_code, combined_output)`.
pub(crate) fn exec_sh_capture(
    command: &str,
    args: &[impl AsRef<std::ffi::OsStr>],
) -> (Option<i32>, String) {
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .arg("_")
        .args(args)
        .output();
    match output {
        Ok(o) => {
            let mut combined = String::from_utf8_lossy(&o.stdout).into_owned();
            combined.push_str(&String::from_utf8_lossy(&o.stderr));
            (o.status.code(), combined.trim_end().to_string())
        }
        Err(_) => (Some(127), String::new()),
    }
}

/// Execute a shell command with bytes supplied on standard input, capturing output.
pub(crate) fn exec_sh_capture_with_stdin(
    command: &str,
    args: &[impl AsRef<std::ffi::OsStr>],
    stdin: &[u8],
) -> (Option<i32>, String) {
    let mut child = match Command::new("sh")
        .arg("-c")
        .arg(command)
        .arg("_")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return (Some(127), String::new()),
    };
    let Some(mut pipe) = child.stdin.take() else {
        return (Some(127), String::new());
    };
    let (output, write_failed) = std::thread::scope(|scope| {
        let writer = scope.spawn(move || {
            pipe.write_all(stdin)
                .is_err_and(|error| error.kind() != std::io::ErrorKind::BrokenPipe)
        });
        let output = child.wait_with_output();
        let write_failed = writer.join().unwrap_or(true);
        (output, write_failed)
    });
    match output {
        Ok(output) if !write_failed => {
            let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
            combined.push_str(&String::from_utf8_lossy(&output.stderr));
            (output.status.code(), combined.trim_end().to_string())
        }
        Ok(_) => (Some(127), String::new()),
        Err(_) => (Some(127), String::new()),
    }
}

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

pub(super) fn hooks_dir_for(format: OutputFormat) -> Result<PathBuf, i32> {
    let cwd = std::env::current_dir().unwrap_or_default();
    match git::workdir(&cwd) {
        Ok(root) => Ok(root.join(".githooks")),
        Err(_) if format == OutputFormat::Json => Err(print_json_error(Diagnostic::operational(
            "GITSTD-GIT-OPERATION",
            "not inside a git repository",
        ))),
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

pub(super) fn read_and_parse_hooks_for(
    hooks_dir: &Path,
    hook_name: &str,
    format: OutputFormat,
) -> Result<Vec<HookCommand>, i32> {
    let hooks_file = hooks_dir.join(format!("{hook_name}.hooks"));
    match std::fs::read_to_string(&hooks_file) {
        Ok(content) => Ok(standard_githooks::parse(&content)),
        Err(error) if format == OutputFormat::Json => {
            Err(print_json_error(Diagnostic::operational(
                "GITSTD-IO-READ",
                format!("cannot read {}: {error}", hooks_file.display()),
            )))
        }
        Err(error) => {
            ui::error(&format!("cannot read {}: {error}", hooks_file.display()));
            Err(2)
        }
    }
}
