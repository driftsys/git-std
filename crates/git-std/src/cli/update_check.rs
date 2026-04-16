//! Non-blocking update check — gh-style background fetch with local cache.
//!
//! On every CLI invocation the cache file is read. When stale (>24 h) or
//! missing, a detached child process is spawned that queries the GitHub
//! releases API via `curl`, writes the result to the cache, and exits.
//! When the cache is fresh and contains a newer version the caller prints
//! a one-line hint *after* command output.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ui;

const STALE_SECS: u64 = 24 * 60 * 60;
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASES_URL: &str = "https://api.github.com/repos/driftsys/git-std/releases/latest";

// ── cache data ──────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct UpdateCache {
    latest_version: String,
    checked_at: u64,
}

// ── public API ──────────────────────────────────────────────────────

/// Spawn a background update check when the cache is missing or stale.
///
/// No-op when `GIT_STD_NO_UPDATE_CHECK=1`, stderr is not a TTY, or the
/// cache is still fresh.
pub fn maybe_spawn_background_check() {
    if is_disabled() || !ui::is_tty() {
        return;
    }
    let Some(path) = cache_path() else { return };
    if let Some(cache) = read_cache_from(&path)
        && !is_stale(cache.checked_at)
    {
        return; // fresh — hint will be printed later
    }
    // Cache missing, corrupt, or stale → spawn background worker.
    let Some(exe) = std::env::current_exe().ok() else {
        return;
    };
    let _ = std::process::Command::new(exe)
        .arg("--update-check-bg")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn(); // fire-and-forget
}

/// Run the actual update check (called as a detached child process).
pub fn run_background_check() {
    let Some(path) = cache_path() else { return };
    let Some(version) = fetch_latest_version() else {
        return;
    };
    let cache = UpdateCache {
        latest_version: version,
        checked_at: now_epoch_secs(),
    };
    let _ = write_cache_to(&path, &cache);
}

/// Print an update hint if the cache is fresh and a newer version exists.
pub fn print_update_hint() {
    if is_disabled() {
        return;
    }
    let Some(path) = cache_path() else { return };
    let Some(cache) = read_cache_from(&path) else {
        return;
    };
    if is_stale(cache.checked_at) {
        return; // stale cache = silence
    }
    if let Some(msg) = build_hint_message(CURRENT_VERSION, &cache.latest_version) {
        ui::blank();
        for line in msg.lines() {
            ui::hint(line);
        }
    }
}

/// Run a self-update of the git-std binary. Returns the exit code.
pub fn run_self_update() -> i32 {
    ui::info("checking for updates…");

    let Some(latest) = fetch_latest_version() else {
        ui::error("could not fetch latest release from GitHub");
        ui::hint("check your network connection and try again");
        return 1;
    };

    let cur = match semver::Version::parse(CURRENT_VERSION) {
        Ok(v) => v,
        Err(_) => {
            ui::error(&format!("cannot parse current version: {CURRENT_VERSION}"));
            return 1;
        }
    };
    let lat = match semver::Version::parse(&latest) {
        Ok(v) => v,
        Err(_) => {
            ui::error(&format!("cannot parse latest version: {latest}"));
            return 1;
        }
    };

    if lat <= cur {
        ui::info(&format!(
            "{} already up to date ({CURRENT_VERSION})",
            ui::pass()
        ));
        return 0;
    }

    ui::info(&format!("updating git-std {CURRENT_VERSION} → {latest}…"));

    let method = detect_install_method();
    let (cmd, args) = update_command_for_method(&method);

    let status = std::process::Command::new(cmd).args(&args).status();

    match status {
        Ok(s) if s.success() => {
            // Update the cache to reflect the new version.
            if let Some(path) = cache_path() {
                let cache = UpdateCache {
                    latest_version: latest.clone(),
                    checked_at: now_epoch_secs(),
                };
                let _ = write_cache_to(&path, &cache);
            }
            ui::info(&format!("{} git-std updated to {latest}", ui::pass()));
            0
        }
        Ok(s) => {
            ui::error("update command failed");
            ui::hint(&format!("try running manually: {method}"));
            s.code().unwrap_or(1)
        }
        Err(e) => {
            ui::error(&format!("could not run update command: {e}"));
            ui::hint(&format!("try running manually: {method}"));
            1
        }
    }
}

// ── private helpers ─────────────────────────────────────────────────

fn is_disabled() -> bool {
    std::env::var("GIT_STD_NO_UPDATE_CHECK")
        .map(|v| v == "1")
        .unwrap_or(false)
}

fn now_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn is_stale(checked_at: u64) -> bool {
    now_epoch_secs().saturating_sub(checked_at) >= STALE_SECS
}

fn cache_path() -> Option<PathBuf> {
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("HOME").ok().map(|h| format!("{h}/.config")))?;
    Some(PathBuf::from(base).join("git-std/update-check.json"))
}

fn read_cache_from(path: &Path) -> Option<UpdateCache> {
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn write_cache_to(path: &Path, cache: &UpdateCache) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string(cache).map_err(std::io::Error::other)?;
    std::fs::write(path, json)
}

fn fetch_latest_version() -> Option<String> {
    let output = std::process::Command::new("curl")
        .args(["-sSf", "--max-time", "10", RELEASES_URL])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let body: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let tag = body.get("tag_name")?.as_str()?;
    let version = tag.strip_prefix('v').unwrap_or(tag);
    semver::Version::parse(version).ok()?;
    Some(version.to_string())
}

fn build_hint_message(current: &str, latest: &str) -> Option<String> {
    let cur = semver::Version::parse(current).ok()?;
    let lat = semver::Version::parse(latest).ok()?;
    if lat <= cur {
        return None;
    }
    let cmd = detect_install_method();
    Some(format!(
        "a new release of git-std is available: {current} \u{2192} {latest}\nto update, run: {cmd}"
    ))
}

fn detect_install_method() -> String {
    let path = std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(String::from))
        .unwrap_or_default();
    install_method_for_path(&path)
}

fn install_method_for_path(path: &str) -> String {
    if path.contains("/.cargo/bin/") {
        "cargo install git-std".to_string()
    } else if path.contains("/.local/bin/") {
        "curl -fsSL https://raw.githubusercontent.com/driftsys/git-std/main/install.sh | sh"
            .to_string()
    } else if path.contains("/nix/store/") {
        "nix profile upgrade git-std".to_string()
    } else {
        "visit https://github.com/driftsys/git-std/releases".to_string()
    }
}

/// Split a human-readable install method string into a command and args
/// suitable for `std::process::Command`.
fn update_command_for_method(method: &str) -> (&str, Vec<&str>) {
    if method.starts_with("cargo install") {
        ("cargo", vec!["install", "git-std"])
    } else if method.starts_with("curl") {
        ("sh", vec!["-c", method])
    } else if method.starts_with("nix") {
        ("nix", vec!["profile", "upgrade", "git-std"])
    } else {
        // Fallback: open the releases page (won't actually work as a command,
        // but run_self_update prints the hint on failure).
        (
            "echo",
            vec!["visit https://github.com/driftsys/git-std/releases"],
        )
    }
}

// ── tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_after_24h() {
        let old = now_epoch_secs() - 25 * 3600;
        assert!(is_stale(old));
    }

    #[test]
    fn fresh_within_24h() {
        let recent = now_epoch_secs() - 3600;
        assert!(!is_stale(recent));
    }

    #[test]
    fn cache_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-check.json");
        let cache = UpdateCache {
            latest_version: "1.2.3".to_string(),
            checked_at: now_epoch_secs(),
        };
        write_cache_to(&path, &cache).unwrap();
        let loaded = read_cache_from(&path).unwrap();
        assert_eq!(loaded.latest_version, "1.2.3");
    }

    #[test]
    fn read_cache_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_cache_from(&dir.path().join("nope.json")).is_none());
    }

    #[test]
    fn read_cache_corrupt_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "not json!!!").unwrap();
        assert!(read_cache_from(&path).is_none());
    }

    #[test]
    fn hint_when_newer() {
        assert!(build_hint_message("0.9.0", "1.0.0").is_some());
    }

    #[test]
    fn no_hint_when_current() {
        assert!(build_hint_message("1.0.0", "1.0.0").is_none());
    }

    #[test]
    fn no_hint_when_ahead() {
        assert!(build_hint_message("2.0.0", "1.0.0").is_none());
    }

    #[test]
    fn method_cargo() {
        let m = install_method_for_path("/home/user/.cargo/bin/git-std");
        assert!(m.contains("cargo install"));
    }

    #[test]
    fn method_local_bin() {
        let m = install_method_for_path("/home/user/.local/bin/git-std");
        assert!(m.contains("curl"));
    }

    #[test]
    fn method_nix() {
        let m = install_method_for_path("/nix/store/abc/bin/git-std");
        assert!(m.contains("nix profile"));
    }

    #[test]
    fn method_other() {
        let m = install_method_for_path("/usr/local/bin/git-std");
        assert!(m.contains("github.com"));
    }

    #[test]
    fn update_cmd_cargo() {
        let (cmd, args) = update_command_for_method("cargo install git-std");
        assert_eq!(cmd, "cargo");
        assert_eq!(args, vec!["install", "git-std"]);
    }

    #[test]
    fn update_cmd_curl() {
        let method =
            "curl -fsSL https://raw.githubusercontent.com/driftsys/git-std/main/install.sh | sh";
        let (cmd, args) = update_command_for_method(method);
        assert_eq!(cmd, "sh");
        assert_eq!(args, vec!["-c", method]);
    }

    #[test]
    fn update_cmd_nix() {
        let (cmd, args) = update_command_for_method("nix profile upgrade git-std");
        assert_eq!(cmd, "nix");
        assert_eq!(args, vec!["profile", "upgrade", "git-std"]);
    }

    #[test]
    fn update_cmd_fallback() {
        let (cmd, _) =
            update_command_for_method("visit https://github.com/driftsys/git-std/releases");
        assert_eq!(cmd, "echo");
    }
}
