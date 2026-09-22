//! Git LFS setup for repositories managed by git-std.

use std::path::Path;
use std::process::Command;

use standard_githooks::generate_shim;

use crate::{git, ui};

const ENTRY: &str = "! [delete] git lfs pre-push \"$@\"";

/// The observable pre-push LFS integration state.
#[derive(PartialEq, Eq)]
pub(crate) enum UploadState {
    /// A managed shim runs the canonical LFS command.
    Managed,
    /// An active custom shim may provide its own LFS integration.
    Custom,
    /// No active managed LFS upload command exists.
    Missing,
}

/// Inspect the active pre-push shim and declarative command.
pub(crate) fn upload_state(root: &Path) -> UploadState {
    let hooks = root.join(".githooks");
    let active = hooks.join("pre-push");
    if !active.exists() {
        return UploadState::Missing;
    }
    if !std::fs::read_to_string(active).is_ok_and(|shim| shim == generate_shim("pre-push")) {
        return UploadState::Custom;
    }
    if std::fs::read_to_string(hooks.join("pre-push.hooks"))
        .is_ok_and(|policy| policy.lines().any(|line| line.trim() == ENTRY))
    {
        UploadState::Managed
    } else {
        UploadState::Missing
    }
}

/// Whether repository attribute files declare the LFS filter.
pub(crate) fn has_declarations(root: &Path) -> std::io::Result<bool> {
    let output = Command::new("git")
        .current_dir(root)
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ])
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::other("cannot list repository attributes"));
    }
    for raw_path in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        let path = String::from_utf8_lossy(raw_path);
        if !path.ends_with(".gitattributes")
            || std::path::Path::new(path.as_ref())
                .file_name()
                .is_none_or(|name| name != ".gitattributes")
        {
            continue;
        }
        let attributes = match std::fs::read_to_string(root.join(path.as_ref())) {
            Ok(attributes) => attributes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if attributes.lines().any(|line| {
            let line = line.trim_start();
            !line.starts_with('#')
                && line
                    .split_whitespace()
                    .skip(1)
                    .any(|attribute| attribute == "filter=lfs")
        }) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Configure local LFS filters and the managed pre-push command.
pub fn install() -> i32 {
    let cwd = std::env::current_dir().unwrap_or_default();
    let root = match git::workdir(&cwd) {
        Ok(root) => root,
        Err(_) => {
            ui::error("not inside a git repository");
            return 1;
        }
    };
    install_at(&root)
}

/// Install LFS integration in an already initialized repository.
pub(crate) fn install_at(root: &Path) -> i32 {
    let hooks = root.join(".githooks");
    let active = hooks.join("pre-push");
    let disabled = hooks.join("pre-push.off");
    let expected = generate_shim("pre-push");
    let shim = if active.exists() {
        &active
    } else if disabled.exists() {
        &disabled
    } else {
        ui::error("pre-push hook not found — run 'git std init' first");
        return 1;
    };
    match std::fs::read_to_string(shim) {
        Ok(content) if content == expected => {}
        Ok(_) => {
            ui::error("pre-push hook is custom — configure LFS in that hook manually");
            return 1;
        }
        Err(error) => {
            ui::error(&format!("cannot read pre-push hook: {error}"));
            return 1;
        }
    }

    let hooks_path = Command::new("git")
        .current_dir(root)
        .args(["config", "--get", "core.hooksPath"])
        .output();
    if !hooks_path.is_ok_and(|output| {
        output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == ".githooks"
    }) {
        ui::error("managed hooks are not active — run 'git std init' first");
        return 1;
    }

    if !is_available(root) {
        ui::error("git-lfs is required but not installed");
        ui::hint("install Git LFS, then retry 'git std lfs install'");
        return 1;
    }

    let install = Command::new("git")
        .current_dir(root)
        .args(["lfs", "install", "--local", "--skip-repo"])
        .output();
    if !install.is_ok_and(|output| output.status.success()) {
        ui::error("git lfs install --local --skip-repo failed");
        return 1;
    }

    let policy = hooks.join("pre-push.hooks");
    let content = match std::fs::read_to_string(&policy) {
        Ok(content) => content,
        Err(error) => {
            ui::error(&format!("cannot read {}: {error}", policy.display()));
            return 1;
        }
    };
    let updated = with_lfs_entry(&content);
    if updated != content
        && let Err(error) = std::fs::write(&policy, updated)
    {
        ui::error(&format!("cannot write {}: {error}", policy.display()));
        return 1;
    }

    if !active.exists() && super::hook::enable("pre-push") != 0 {
        return 1;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if let Err(error) =
            std::fs::set_permissions(&active, std::fs::Permissions::from_mode(0o755))
        {
            ui::error(&format!("cannot make pre-push executable: {error}"));
            return 1;
        }
    }
    ui::info("Git LFS configured for managed pre-push hooks");
    0
}

/// Whether the Git LFS executable is available to Git in this repository.
pub(crate) fn is_available(root: &Path) -> bool {
    Command::new("git")
        .current_dir(root)
        .args(["lfs", "version"])
        .output()
        .is_ok_and(|output| output.status.success())
}

fn with_lfs_entry(content: &str) -> String {
    let mut lines = Vec::new();
    let mut has_entry = false;
    for line in content.lines() {
        let trimmed = line.trim();
        let is_canonical = trimmed == ENTRY
            || trimmed
                .strip_prefix('#')
                .is_some_and(|rest| rest.trim() == ENTRY);
        if is_canonical {
            if !has_entry {
                lines.push(ENTRY);
                has_entry = true;
            }
        } else {
            lines.push(line);
        }
    }
    if !has_entry {
        lines.push(ENTRY);
    }
    format!("{}\n", lines.join("\n"))
}
