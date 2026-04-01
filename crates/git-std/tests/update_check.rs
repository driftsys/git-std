use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use tempfile::TempDir;

#[test]
fn update_check_bg_exits_cleanly() {
    let config_dir = TempDir::new().unwrap();
    Command::cargo_bin("git-std")
        .unwrap()
        .arg("--update-check-bg")
        .env("XDG_CONFIG_HOME", config_dir.path())
        .timeout(std::time::Duration::from_secs(15))
        .assert()
        .success();
}

#[test]
fn opt_out_suppresses_hint() {
    let config_dir = TempDir::new().unwrap();
    prepopulate_cache(config_dir.path(), "99.99.99");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["lint", "feat: test"])
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("GIT_STD_NO_UPDATE_CHECK", "1")
        .assert()
        .success()
        .stderr(predicates::str::contains("hint:").not());
}

#[test]
fn hint_printed_when_cache_has_newer_version() {
    let config_dir = TempDir::new().unwrap();
    prepopulate_cache(config_dir.path(), "99.99.99");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["lint", "feat: test"])
        .env("XDG_CONFIG_HOME", config_dir.path())
        .assert()
        .success()
        .stderr(predicates::str::contains(
            "a new release of git-std is available",
        ));
}

#[test]
fn no_hint_when_cache_is_stale() {
    let config_dir = TempDir::new().unwrap();
    prepopulate_cache_with_age(config_dir.path(), "99.99.99", 25 * 3600);

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["lint", "feat: test"])
        .env("XDG_CONFIG_HOME", config_dir.path())
        .assert()
        .success()
        .stderr(predicates::str::contains("a new release").not());
}

#[test]
fn no_hint_when_up_to_date() {
    let config_dir = TempDir::new().unwrap();
    let current = env!("CARGO_PKG_VERSION");
    prepopulate_cache(config_dir.path(), current);

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["lint", "feat: test"])
        .env("XDG_CONFIG_HOME", config_dir.path())
        .assert()
        .success()
        .stderr(predicates::str::contains("a new release").not());
}

// ── helpers ─────────────────────────────────────────────────────────

fn prepopulate_cache(config_dir: &std::path::Path, version: &str) {
    prepopulate_cache_with_age(config_dir, version, 0);
}

fn prepopulate_cache_with_age(config_dir: &std::path::Path, version: &str, age_secs: u64) {
    let cache_dir = config_dir.join("git-std");
    std::fs::create_dir_all(&cache_dir).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let cache = serde_json::json!({
        "latest_version": version,
        "checked_at": now - age_secs,
    });
    std::fs::write(
        cache_dir.join("update-check.json"),
        serde_json::to_string(&cache).unwrap(),
    )
    .unwrap();
}
