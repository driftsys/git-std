//! `git std bootstrap` — post-clone environment setup.
//!
//! Two entry points:
//! - `run(dry_run)` — detect convention files and configure the local environment.
//! - `install(force)` — scaffold `./bootstrap` and `.githooks/bootstrap.hooks`.

use std::path::Path;
use std::process::Command;

use crate::ui;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const BOOTSTRAP_HOOKS_FILE: &str = ".githooks/bootstrap.hooks";
const BOOTSTRAP_SCRIPT: &str = "bootstrap";
const MARKER: &str = "<!-- git-std:bootstrap -->";
const LFS_INSTALL_URL: &str = "https://git-lfs.github.com";

// ---------------------------------------------------------------------------
// git std bootstrap (run)
// ---------------------------------------------------------------------------

/// Run the built-in bootstrap checks and any custom `bootstrap.hooks`.
///
/// Returns the process exit code (`0` success, `1` failure).
pub fn run(dry_run: bool) -> i32 {
    let mut failed = false;

    // Tier 1 — built-in checks
    if !check_hooks_path(dry_run) {
        failed = true;
    }
    if !check_lfs(dry_run) {
        return 1; // hard failure — git-lfs missing
    }
    if !check_blame_ignore_revs(dry_run) {
        failed = true;
    }

    // Tier 2 — custom bootstrap.hooks
    if Path::new(BOOTSTRAP_HOOKS_FILE).exists() {
        if dry_run {
            ui::info(&format!("{}  custom bootstrap hooks executed", ui::pass()));
        } else {
            let code = super::hooks::run("bootstrap", &[], crate::app::OutputFormat::Text);
            if code != 0 {
                failed = true;
            }
        }
    }

    if failed { 1 } else { 0 }
}

/// Detect `.githooks/` and set `core.hooksPath`.
fn check_hooks_path(dry_run: bool) -> bool {
    let hooks_dir = Path::new(".githooks");
    if !hooks_dir.exists() {
        return true;
    }

    if dry_run {
        ui::info(&format!("{}  git hooks configured", ui::pass()));
        return true;
    }

    let status = Command::new("git")
        .args(["config", "core.hooksPath", ".githooks"])
        .status();

    match status {
        Ok(s) if s.success() => {
            ui::info(&format!("{}  git hooks configured", ui::pass()));
            true
        }
        _ => {
            ui::error("failed to set core.hooksPath");
            false
        }
    }
}

/// Detect `filter=lfs` in `.gitattributes` and run LFS setup.
///
/// Returns `false` only when LFS rules are detected but `git-lfs` is not
/// installed — this is a hard failure (exit 1).
fn check_lfs(dry_run: bool) -> bool {
    let attrs = Path::new(".gitattributes");
    if !attrs.exists() {
        return true;
    }

    let content = match std::fs::read_to_string(attrs) {
        Ok(c) => c,
        Err(e) => {
            ui::error(&format!("cannot read .gitattributes: {e}"));
            return false;
        }
    };

    if !content.lines().any(|line| line.contains("filter=lfs")) {
        return true;
    }

    // LFS rules detected — check if git-lfs is installed
    let lfs_available = Command::new("git")
        .args(["lfs", "version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !lfs_available {
        ui::error("git-lfs is required but not installed");
        ui::hint(&format!("install from {LFS_INSTALL_URL}"));
        return false;
    }

    if dry_run {
        ui::info(&format!("{}  LFS objects downloaded", ui::pass()));
        return true;
    }

    // Run git lfs install
    let install_ok = Command::new("git")
        .args(["lfs", "install"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !install_ok {
        ui::error("git lfs install failed");
        return false;
    }

    // Run git lfs pull
    let pull_ok = Command::new("git")
        .args(["lfs", "pull"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !pull_ok {
        ui::error("git lfs pull failed");
        return false;
    }

    ui::info(&format!("{}  LFS objects downloaded", ui::pass()));
    true
}

/// Detect `.git-blame-ignore-revs` and set `blame.ignoreRevsFile`.
fn check_blame_ignore_revs(dry_run: bool) -> bool {
    let path = Path::new(".git-blame-ignore-revs");
    if !path.exists() {
        return true;
    }

    if dry_run {
        ui::info(&format!("{}  blame ignore revs configured", ui::pass()));
        return true;
    }

    let status = Command::new("git")
        .args(["config", "blame.ignoreRevsFile", ".git-blame-ignore-revs"])
        .status();

    match status {
        Ok(s) if s.success() => {
            ui::info(&format!("{}  blame ignore revs configured", ui::pass()));
            true
        }
        _ => {
            ui::error("failed to set blame.ignoreRevsFile");
            false
        }
    }
}

// ---------------------------------------------------------------------------
// git std bootstrap install
// ---------------------------------------------------------------------------

/// Generate `./bootstrap` script and `.githooks/bootstrap.hooks` template.
///
/// Returns the process exit code (`0` success, `1` failure).
pub fn install(force: bool) -> i32 {
    let mut created = Vec::new();
    let mut skipped = Vec::new();

    // 1. Generate ./bootstrap
    match write_bootstrap_script(force) {
        FileResult::Created => created.push(BOOTSTRAP_SCRIPT),
        FileResult::Skipped => skipped.push(BOOTSTRAP_SCRIPT),
        FileResult::Error => return 1,
    }

    // 2. Generate .githooks/bootstrap.hooks
    if let Err(e) = std::fs::create_dir_all(".githooks") {
        ui::error(&format!("cannot create .githooks/: {e}"));
        return 1;
    }
    match write_bootstrap_hooks(force) {
        FileResult::Created => created.push(BOOTSTRAP_HOOKS_FILE),
        FileResult::Skipped => skipped.push(BOOTSTRAP_HOOKS_FILE),
        FileResult::Error => return 1,
    }

    // 3. Append post-clone reminder to AGENTS.md and README.md
    let mut modified_docs: Vec<&str> = Vec::new();
    for doc in &["AGENTS.md", "README.md"] {
        if Path::new(doc).exists()
            && let Err(e) = append_bootstrap_marker(doc)
        {
            ui::error(&format!("cannot update {doc}: {e}"));
            return 1;
        }
        if Path::new(doc).exists() {
            modified_docs.push(doc);
        }
    }

    // 4. Stage all created/modified files so the executable bit is tracked
    let mut stage_files: Vec<&str> = created.to_vec();
    stage_files.extend(modified_docs);
    if !stage_files.is_empty() {
        let mut cmd = Command::new("git");
        cmd.arg("add").arg("--");
        for f in &stage_files {
            cmd.arg(f);
        }
        if let Err(e) = cmd.status() {
            ui::warning(&format!("git add failed: {e} — stage files manually"));
        }
    }

    // Print summary
    ui::blank();
    for path in &created {
        ui::info(&format!("{}  {path} created", ui::pass()));
    }
    for path in &skipped {
        ui::info(&format!(
            "{}  {path} already exists (use --force to overwrite)",
            ui::warn()
        ));
    }

    0
}

enum FileResult {
    Created,
    Skipped,
    Error,
}

/// Write the `./bootstrap` shell wrapper.
fn write_bootstrap_script(force: bool) -> FileResult {
    let path = Path::new(BOOTSTRAP_SCRIPT);
    if path.exists() && !force {
        return FileResult::Skipped;
    }

    let script = generate_bootstrap_script();
    if let Err(e) = std::fs::write(path, &script) {
        ui::error(&format!("cannot write {BOOTSTRAP_SCRIPT}: {e}"));
        return FileResult::Error;
    }

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        if let Err(e) = std::fs::set_permissions(path, perms) {
            ui::error(&format!(
                "cannot set permissions on {BOOTSTRAP_SCRIPT}: {e}"
            ));
            return FileResult::Error;
        }
    }

    FileResult::Created
}

/// Write `.githooks/bootstrap.hooks` template.
fn write_bootstrap_hooks(force: bool) -> FileResult {
    let path = Path::new(BOOTSTRAP_HOOKS_FILE);
    if path.exists() && !force {
        return FileResult::Skipped;
    }

    let has_lfs = Path::new(".gitattributes")
        .exists()
        .then(|| std::fs::read_to_string(".gitattributes").unwrap_or_default())
        .map(|c| c.lines().any(|l| l.contains("filter=lfs")))
        .unwrap_or(false);

    let template = generate_bootstrap_hooks_template(has_lfs);
    if let Err(e) = std::fs::write(path, &template) {
        ui::error(&format!("cannot write {BOOTSTRAP_HOOKS_FILE}: {e}"));
        return FileResult::Error;
    }

    FileResult::Created
}

/// Append a post-clone reminder to a documentation file, idempotently.
fn append_bootstrap_marker(path: &str) -> std::io::Result<()> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    if content.contains(MARKER) {
        return Ok(());
    }

    use std::io::Write;
    let note = format!(
        "\n{MARKER}\n\
         ## Post-clone setup\n\
         \n\
         Run `./bootstrap` after `git clone` or `git worktree add`.\n"
    );
    let mut file = std::fs::OpenOptions::new().append(true).open(path)?;
    file.write_all(note.as_bytes())
}

// ---------------------------------------------------------------------------
// Generated content
// ---------------------------------------------------------------------------

/// Generate the `./bootstrap` bash script content.
fn generate_bootstrap_script() -> String {
    let version = env!("CARGO_PKG_VERSION");
    format!(
        r##"#!/usr/bin/env bash
set -euo pipefail

# Minimum git-std version required by this project.
MIN_VERSION="{version}"
REPO="driftsys/git-std"
INSTALL_DIR="${{GIT_STD_INSTALL_DIR:-$HOME/.local/bin}}"

die() {{ printf 'error: %s\n' "$1" >&2; exit 1; }}

sha256_check() {{
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$1"
  else
    shasum -a 256 -c "$1"
  fi
}}

detect_target() {{
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)
      case "$arch" in
        x86_64)  echo "x86_64-unknown-linux-gnu" ;;
        aarch64) echo "aarch64-unknown-linux-gnu" ;;
        *)       die "unsupported architecture: $arch" ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        x86_64)  echo "x86_64-apple-darwin" ;;
        arm64)   echo "aarch64-apple-darwin" ;;
        *)       die "unsupported architecture: $arch" ;;
      esac
      ;;
    *)
      die "unsupported OS: $os (use WSL on Windows)"
      ;;
  esac
}}

# Compare two semver strings. Returns 0 if $1 >= $2, 1 otherwise.
version_gte() {{
  local IFS=.
  local i a=($1) b=($2)
  for ((i = 0; i < 3; i++)); do
    local ai="${{a[i]:-0}}" bi="${{b[i]:-0}}"
    if ((ai > bi)); then return 0; fi
    if ((ai < bi)); then return 1; fi
  done
  return 0
}}

ensure_git_std() {{
  if command -v git-std >/dev/null 2>&1; then
    local current
    current="$(git-std --version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')"
    if version_gte "$current" "$MIN_VERSION"; then
      return 0
    fi
    printf 'git-std %s found, need >= %s — upgrading\n' "$current" "$MIN_VERSION"
  else
    printf 'git-std not found — installing\n'
  fi

  local target base download_url version tmp_dir
  target="$(detect_target)"

  version="$(curl -sSf "https://api.github.com/repos/$REPO/releases/latest" \
    | grep '"tag_name"' | head -1 | cut -d'"' -f4)"
  [ -n "$version" ] || die "could not determine latest release"

  base="git-std-$target"
  download_url="https://github.com/$REPO/releases/download/$version/$base.tar.gz"
  printf 'downloading %s\n' "$download_url"

  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "${{tmp_dir:-}}"' EXIT

  curl -sSfL "$download_url" -o "$tmp_dir/$base.tar.gz" \
    || die "download failed — check that the release exists for $target"
  curl -sSfL "$download_url.sha256" -o "$tmp_dir/$base.tar.gz.sha256" \
    || die "checksum download failed"

  (cd "$tmp_dir" && sha256_check "$base.tar.gz.sha256") \
    || die "checksum verification failed"

  tar -xzf "$tmp_dir/$base.tar.gz" -C "$tmp_dir"

  mkdir -p "$INSTALL_DIR"
  mv "$tmp_dir/git-std" "$INSTALL_DIR/git-std"
  chmod +x "$INSTALL_DIR/git-std"

  printf 'installed git-std %s to %s/git-std\n' "$version" "$INSTALL_DIR"
}}

ensure_git_std
exec git std bootstrap
"##
    )
}

/// Generate the `.githooks/bootstrap.hooks` template.
fn generate_bootstrap_hooks_template(has_lfs: bool) -> String {
    let lfs_example = if has_lfs {
        "# LFS detected — uncomment to pull large files:\n\
         # ! git lfs pull\n\
         #\n"
    } else {
        ""
    };

    format!(
        "# git-std hooks — bootstrap.hooks\n\
         #\n\
         # Commands run by `git std bootstrap` after built-in checks.\n\
         # Prefix controls behavior:\n\
         #\n\
         #   !  required   fail bootstrap on failure\n\
         #   ?  advisory   run command, never fail bootstrap\n\
         #\n\
         # Examples:\n\
         #   ! npm install          # install dependencies\n\
         #   ! pip install -r requirements.txt\n\
         #   ? pre-commit install   # optional tool setup\n\
         #\n\
         {lfs_example}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_script_contains_min_version() {
        let script = generate_bootstrap_script();
        let version = env!("CARGO_PKG_VERSION");
        assert!(script.contains(&format!("MIN_VERSION=\"{version}\"")));
    }

    #[test]
    fn bootstrap_script_starts_with_shebang() {
        let script = generate_bootstrap_script();
        assert!(script.starts_with("#!/usr/bin/env bash"));
    }

    #[test]
    fn bootstrap_script_delegates_to_git_std() {
        let script = generate_bootstrap_script();
        assert!(script.contains("exec git std bootstrap"));
    }

    #[test]
    fn bootstrap_hooks_template_has_header() {
        let t = generate_bootstrap_hooks_template(false);
        assert!(t.contains("bootstrap.hooks"));
        assert!(t.contains("!  required"));
        assert!(t.contains("?  advisory"));
    }

    #[test]
    fn bootstrap_hooks_template_includes_lfs_when_detected() {
        let t = generate_bootstrap_hooks_template(true);
        assert!(t.contains("LFS detected"));
        assert!(t.contains("git lfs pull"));
    }

    #[test]
    fn bootstrap_hooks_template_no_lfs_when_absent() {
        let t = generate_bootstrap_hooks_template(false);
        assert!(!t.contains("LFS detected"));
    }

    #[test]
    fn marker_is_html_comment() {
        assert!(MARKER.starts_with("<!--"));
        assert!(MARKER.ends_with("-->"));
    }
}
