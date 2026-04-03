//! `git std init` — single maintainer setup command.
//!
//! Consolidates `hook install` and `bootstrap install` into one command.
//! Steps:
//! 1. Create `.githooks/` directory.
//! 2. Set `core.hooksPath` to `.githooks`.
//! 3. Write `.hooks` templates (pre-commit, commit-msg, pre-push).
//! 4. Prompt which hooks to enable, write shims.
//! 5. Generate `./bootstrap` script.
//! 6. Generate `.githooks/bootstrap.hooks`.
//! 7. Create `.git-std.toml` with taplo schema directive (if absent).
//! 8. Append post-clone section to README/AGENTS (if found).
//! 9. Stage everything.

use std::io::IsTerminal;
use std::path::Path;
use std::process::Command;

use inquire::MultiSelect;
use yansi::Paint;

use standard_githooks::{KNOWN_HOOKS, generate_hooks_template, generate_shim};

use crate::ui;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const BOOTSTRAP_HOOKS_FILE: &str = ".githooks/bootstrap.hooks";
const BOOTSTRAP_SCRIPT: &str = "bootstrap";
const CONFIG_FILE: &str = ".git-std.toml";
const MARKER: &str = "<!-- git-std:bootstrap -->";

const LIFECYCLE_HOOKS: &[&str] = &["pre-bump", "post-version", "post-changelog", "post-bump"];

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run `git std init`. Returns the process exit code.
pub fn run(force: bool) -> i32 {
    let cwd = std::env::current_dir().unwrap_or_default();
    let root = match crate::git::workdir(&cwd) {
        Ok(r) => r,
        Err(_) => {
            ui::error("not inside a git repository");
            return 1;
        }
    };

    let hooks_dir = root.join(".githooks");

    // ── Step 1: ensure .githooks/ exists ────────────────────────────────────
    if let Err(e) = std::fs::create_dir_all(&hooks_dir) {
        ui::error(&format!("cannot create .githooks/: {e}"));
        return 1;
    }

    // ── Step 2: set core.hooksPath ───────────────────────────────────────────
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

    // ── Step 3: write .hooks templates for every known hook ──────────────────
    for hook_name in KNOWN_HOOKS {
        let template_path = hooks_dir.join(format!("{hook_name}.hooks"));
        if !template_path.exists() || force {
            let content = generate_hooks_template(hook_name);
            if let Err(e) = std::fs::write(&template_path, &content) {
                ui::error(&format!("cannot write {}: {e}", template_path.display()));
                return 1;
            }
        }
    }

    // ── Step 3b: write lifecycle hook templates ──────────────────────────────
    for hook_name in LIFECYCLE_HOOKS {
        let template_path = hooks_dir.join(format!("{hook_name}.hooks"));
        if !template_path.exists() || force {
            let content = generate_lifecycle_hook_template(hook_name);
            if let Err(e) = std::fs::write(&template_path, &content) {
                ui::error(&format!("cannot write {}: {e}", template_path.display()));
                return 1;
            }
        }
    }

    // ── Step 4: determine which hooks to enable and write shims ─────────────
    let default_enabled = ["pre-commit", "commit-msg"];

    // Test/CI escape hatch — not a supported public API.
    // Accepts "all", "none", or a comma-separated list of hook names.
    let env_enable = std::env::var("GIT_STD_HOOKS_ENABLE").ok();
    let selected: Vec<&str> = if let Some(ref val) = env_enable {
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
                ui::error("init cancelled");
                return 1;
            }
        }
    };

    ui::blank();

    // Write shims — active for selected, .off for the rest
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

    // ── Step 5 & 6: generate ./bootstrap and .githooks/bootstrap.hooks ───────
    let mut staged: Vec<&str> = Vec::new();

    match write_bootstrap_script(&root, force) {
        FileResult::Created => {
            staged.push(BOOTSTRAP_SCRIPT);
            ui::info(&format!("{}  {BOOTSTRAP_SCRIPT} created", ui::pass()));
        }
        FileResult::Skipped => {
            ui::info(&format!(
                "{}  {BOOTSTRAP_SCRIPT} already exists (use --force to overwrite)",
                ui::warn()
            ));
        }
        FileResult::Error => return 1,
    }

    match write_bootstrap_hooks(&root, force) {
        FileResult::Created => {
            staged.push(BOOTSTRAP_HOOKS_FILE);
            ui::info(&format!("{}  {BOOTSTRAP_HOOKS_FILE} created", ui::pass()));
        }
        FileResult::Skipped => {
            ui::info(&format!(
                "{}  {BOOTSTRAP_HOOKS_FILE} already exists (use --force to overwrite)",
                ui::warn()
            ));
        }
        FileResult::Error => return 1,
    }

    // ── Step 7: create .git-std.toml with taplo schema directive ────────────
    match write_config_file(&root, force) {
        FileResult::Created => {
            staged.push(CONFIG_FILE);
            ui::info(&format!("{}  {CONFIG_FILE} created", ui::pass()));
        }
        FileResult::Skipped => {
            ui::info(&format!(
                "{}  {CONFIG_FILE} already exists (use --force to overwrite)",
                ui::warn()
            ));
        }
        FileResult::Error => return 1,
    }

    // ── Step 8: append post-clone section to README/AGENTS ───────────────────
    for doc in &["AGENTS.md", "README.md"] {
        let doc_path = root.join(doc);
        if doc_path.exists() {
            if let Err(e) = append_bootstrap_marker(&doc_path) {
                ui::error(&format!("cannot update {doc}: {e}"));
                return 1;
            }
            staged.push(doc);
        }
    }

    // ── Step 9: stage all created/modified files ─────────────────────────────
    // Always stage .githooks/ (shims + templates) plus any other created files.
    let mut cmd = Command::new("git");
    cmd.current_dir(&root).arg("add").arg("--").arg(".githooks");
    for f in &staged {
        cmd.arg(f);
    }
    if let Err(e) = cmd.status() {
        ui::warning(&format!("git add failed: {e} — stage files manually"));
    }

    0
}

// ---------------------------------------------------------------------------
// File helpers
// ---------------------------------------------------------------------------

enum FileResult {
    Created,
    Skipped,
    Error,
}

/// Write the `./bootstrap` shell wrapper.
fn write_bootstrap_script(root: &Path, force: bool) -> FileResult {
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
fn write_bootstrap_hooks(root: &Path, force: bool) -> FileResult {
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

/// Write `.git-std.toml` starter config with taplo schema directive.
fn write_config_file(root: &Path, force: bool) -> FileResult {
    let path = root.join(CONFIG_FILE);
    if path.exists() && !force {
        return FileResult::Skipped;
    }

    let template = generate_config_template();
    if let Err(e) = std::fs::write(&path, &template) {
        ui::error(&format!("cannot write {CONFIG_FILE}: {e}"));
        return FileResult::Error;
    }

    FileResult::Created
}

/// Append a post-clone reminder to a documentation file, idempotently.
fn append_bootstrap_marker(path: &Path) -> std::io::Result<()> {
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

/// Generate the `.git-std.toml` starter config content.
fn generate_config_template() -> String {
    "\
#:schema https://driftsys.github.io/git-std/schemas/v1/git-std.schema.json

# scheme = \"semver\"          # semver | calver | patch
# strict = false             # enforce types/scopes without --strict flag
# types = [\"feat\", \"fix\", \"docs\", \"style\", \"refactor\",
#           \"perf\", \"test\", \"chore\", \"ci\", \"build\", \"revert\"]
# scopes = \"auto\"            # \"auto\" | [\"scope1\", \"scope2\"] | omit
"
    .to_string()
}

/// Generate a bump lifecycle hook template for the given hook name.
fn generate_lifecycle_hook_template(hook_name: &str) -> String {
    match hook_name {
        "pre-bump" => "\
# git-std hooks — pre-bump.hooks
#
# Runs before version detection. Non-zero exit aborts the bump.
# Use for: guard checks (clean tree, correct branch, tests pass).
#
#   !  required   abort bump on failure
#   ?  advisory   warn on failure, never abort
#
# Examples:
#   ! cargo test --workspace
#   ! git diff --exit-code   # abort if working tree is dirty
#
"
        .to_string(),
        "post-version" => "\
# git-std hooks — post-version.hooks
#
# Runs after version files are updated. $1 is the new version string.
# Use for: building artifacts, stamping binaries, generating manifests.
#
#   !  required   abort bump on failure
#   ?  advisory   warn on failure, never abort
#
# Examples:
#   ! cargo build --release
#   ? cp target/release/mybin dist/
#
"
        .to_string(),
        "post-changelog" => "\
# git-std hooks — post-changelog.hooks
#
# Runs after CHANGELOG.md is written, before staging and commit.
# Use for: linting or reformatting the changelog.
#
#   !  required   abort bump on failure
#   ?  advisory   warn on failure, never abort
#
# Examples:
#   ? npx markdownlint CHANGELOG.md
#
"
        .to_string(),
        "post-bump" => "\
# git-std hooks — post-bump.hooks
#
# Runs after commit and tag are created (and after push if --push).
# Use for: publishing, deployment, notifications.
#
#   !  required   report failure
#   ?  advisory   warn on failure, always continues
#
# Examples:
#   ! cargo publish
#   ? curl -X POST https://hooks.slack.com/...
#
"
        .to_string(),
        _ => format!("# git-std hooks — {hook_name}.hooks\n"),
    }
}

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
    fn config_template_has_schema_directive() {
        let t = generate_config_template();
        assert!(t.starts_with("#:schema "));
        assert!(t.contains("git-std.schema.json"));
    }

    #[test]
    fn config_template_has_commented_fields() {
        let t = generate_config_template();
        assert!(t.contains("# scheme"));
        assert!(t.contains("# strict"));
        assert!(t.contains("# types"));
        assert!(t.contains("# scopes"));
    }

    #[test]
    fn marker_is_html_comment() {
        assert!(MARKER.starts_with("<!--"));
        assert!(MARKER.ends_with("-->"));
    }

    #[test]
    fn lifecycle_hook_templates_have_headers() {
        for hook in LIFECYCLE_HOOKS {
            let t = generate_lifecycle_hook_template(hook);
            assert!(
                t.contains(&format!("# git-std hooks — {hook}.hooks")),
                "{hook}.hooks template should have header"
            );
            assert!(
                t.contains("!  required"),
                "{hook}.hooks should document ! sigil"
            );
            assert!(
                t.contains("?  advisory"),
                "{hook}.hooks should document ? sigil"
            );
        }
    }

    #[test]
    fn pre_bump_template_mentions_when_it_runs() {
        let t = generate_lifecycle_hook_template("pre-bump");
        assert!(t.contains("before version detection"));
        assert!(t.contains("abort bump on failure"));
    }

    #[test]
    fn post_version_template_mentions_version_arg() {
        let t = generate_lifecycle_hook_template("post-version");
        assert!(t.contains("$1 is the new version string"));
    }

    #[test]
    fn post_changelog_template_mentions_when_it_runs() {
        let t = generate_lifecycle_hook_template("post-changelog");
        assert!(t.contains("after CHANGELOG.md is written"));
    }

    #[test]
    fn post_bump_template_mentions_when_it_runs() {
        let t = generate_lifecycle_hook_template("post-bump");
        assert!(t.contains("after commit and tag are created"));
    }
}
