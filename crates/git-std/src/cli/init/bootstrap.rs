//! Bootstrap file generation for `git std init`.
//!
//! Owns: `./bootstrap` script, `.githooks/bootstrap.hooks` template,
//! post-clone marker injection into README/AGENTS docs.

use std::path::Path;

use crate::ui;

use super::{BOOTSTRAP_HOOKS_FILE, BOOTSTRAP_SCRIPT, FileResult, MARKER};

// ---------------------------------------------------------------------------
// Writers
// ---------------------------------------------------------------------------

/// Write the `./bootstrap` shell wrapper.
pub fn write_bootstrap_script(root: &Path, force: bool) -> FileResult {
    let path = root.join(BOOTSTRAP_SCRIPT);
    if path.exists() && !force {
        return FileResult::Skipped;
    }

    let script = generate_bootstrap_script();
    if let Err(e) = std::fs::write(&path, &script) {
        ui::error(&format!("cannot write {BOOTSTRAP_SCRIPT}: {e}"));
        return FileResult::Error;
    }

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        if let Err(e) = std::fs::set_permissions(&path, perms) {
            ui::error(&format!(
                "cannot set permissions on {BOOTSTRAP_SCRIPT}: {e}"
            ));
            return FileResult::Error;
        }
    }

    FileResult::Created
}

/// Write `.githooks/bootstrap.hooks` template.
pub fn write_bootstrap_hooks(root: &Path, force: bool) -> FileResult {
    let path = root.join(BOOTSTRAP_HOOKS_FILE);
    if path.exists() && !force {
        return FileResult::Skipped;
    }

    let attrs_path = root.join(".gitattributes");
    let has_lfs = attrs_path
        .exists()
        .then(|| std::fs::read_to_string(&attrs_path).unwrap_or_default())
        .map(|c| c.lines().any(|l| l.contains("filter=lfs")))
        .unwrap_or(false);

    let template = generate_bootstrap_hooks_template(has_lfs);
    if let Err(e) = std::fs::write(&path, &template) {
        ui::error(&format!("cannot write {BOOTSTRAP_HOOKS_FILE}: {e}"));
        return FileResult::Error;
    }

    FileResult::Created
}

/// Append a post-clone reminder to a documentation file, idempotently.
pub fn append_bootstrap_marker(path: &Path) -> std::io::Result<()> {
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

  # Install man pages if present in the tarball.
  local man_dir="${{GIT_STD_MAN_DIR:-$INSTALL_DIR/../share/man/man1}}"
  if ls "$tmp_dir"/git-std*.1 >/dev/null 2>&1; then
    mkdir -p "$man_dir"
    cp "$tmp_dir"/git-std*.1 "$man_dir/"
    printf 'installed man pages to %s\n' "$man_dir"
    printf "hint: if 'man git-std' doesn't work, add to your shell profile:\n"
    printf "      export MANPATH=\"\$HOME/.local/share/man:\${{MANPATH:-}}\"\n"
  fi
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
