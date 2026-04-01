//! Integration tests for monorepo versioning.

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;

/// Helper: run a git command and return stdout.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Helper: get HEAD commit message.
fn head_message(dir: &Path) -> String {
    git(dir, &["log", "-1", "--format=%B"]).trim().to_string()
}

/// Helper: collect all tag names.
fn collect_tag_names(dir: &Path) -> Vec<String> {
    let output = git(dir, &["tag", "-l"]);
    if output.is_empty() {
        vec![]
    } else {
        output.lines().map(|s| s.to_string()).collect()
    }
}

/// Helper: write a file and ensure parent directories exist.
fn write_file(dir: &Path, path: &str, content: &str) {
    let full = dir.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(full, content).unwrap();
}

/// Helper: add a commit to a repo with a file.
fn add_commit(dir: &Path, filename: &str, message: &str) {
    write_file(dir, filename, message);
    git(dir, &["add", filename]);
    git(dir, &["commit", "-m", message]);
}

/// Helper: create an annotated tag.
fn create_tag(dir: &Path, name: &str) {
    git(dir, &["tag", "-a", name, "-m", name]);
}

/// Set up a two-package Cargo monorepo.
fn init_monorepo(dir: &Path) {
    git(dir, &["init"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@test.com"]);

    // Root Cargo.toml with workspace.
    write_file(
        dir,
        "Cargo.toml",
        r#"[workspace]
members = ["crates/core", "crates/cli"]
"#,
    );

    // Package: core
    write_file(
        dir,
        "crates/core/Cargo.toml",
        r#"[package]
name = "core"
version = "0.1.0"
edition = "2021"
"#,
    );
    write_file(dir, "crates/core/src/lib.rs", "");

    // Package: cli (depends on core)
    write_file(
        dir,
        "crates/cli/Cargo.toml",
        r#"[package]
name = "cli"
version = "0.1.0"
edition = "2021"

[dependencies]
core = { path = "../core" }
"#,
    );
    write_file(dir, "crates/cli/src/main.rs", "fn main() {}");

    // Config enabling monorepo.
    write_file(dir, ".git-std.toml", "monorepo = true\n");

    git(dir, &["add", "."]);
    git(dir, &["commit", "-m", "chore: init monorepo"]);
}

// ── Dry-run plan tests ─────────────────────────────────────────────

#[test]
fn monorepo_dry_run_shows_package_plans() {
    let dir = tempfile::tempdir().unwrap();
    init_monorepo(dir.path());
    create_tag(dir.path(), "v0.1.0");
    create_tag(dir.path(), "core@0.1.0");
    create_tag(dir.path(), "cli@0.1.0");

    add_commit(
        dir.path(),
        "crates/core/src/lib.rs",
        "feat: add core feature",
    );

    // Pre-1.0: feat (Minor) → "patch — new feature (pre-1.0)".
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("core"))
        .stderr(predicate::str::contains("0.1.0"))
        .stderr(predicate::str::contains("patch"));
}

#[test]
fn monorepo_dry_run_json_output() {
    let dir = tempfile::tempdir().unwrap();
    init_monorepo(dir.path());
    create_tag(dir.path(), "v0.1.0");
    create_tag(dir.path(), "core@0.1.0");
    create_tag(dir.path(), "cli@0.1.0");

    add_commit(
        dir.path(),
        "crates/core/src/lib.rs",
        "feat: add core feature",
    );

    let output = Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run", "--format", "json"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid JSON output");
    assert!(json.get("packages").is_some());
    assert!(json["dry_run"].as_bool().unwrap());
}

// ── Package filter tests ───────────────────────────────────────────

#[test]
fn monorepo_package_filter() {
    let dir = tempfile::tempdir().unwrap();
    init_monorepo(dir.path());
    create_tag(dir.path(), "v0.1.0");
    create_tag(dir.path(), "core@0.1.0");
    create_tag(dir.path(), "cli@0.1.0");

    add_commit(
        dir.path(),
        "crates/core/src/lib.rs",
        "feat: add core feature",
    );
    add_commit(dir.path(), "crates/cli/src/main.rs", "fix: fix cli bug");

    // Pre-1.0: feat (Minor) → "patch — new feature (pre-1.0)".
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run", "-p", "core"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("core"))
        .stderr(predicate::str::contains("patch"));
}

#[test]
fn monorepo_unknown_package_error() {
    let dir = tempfile::tempdir().unwrap();
    init_monorepo(dir.path());

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run", "-p", "nonexistent"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("no packages matched"));
}

// ── Dependency cascade tests ───────────────────────────────────────

#[test]
fn monorepo_cascade_bumps_dependent() {
    let dir = tempfile::tempdir().unwrap();
    init_monorepo(dir.path());
    create_tag(dir.path(), "v0.1.0");
    create_tag(dir.path(), "core@0.1.0");
    create_tag(dir.path(), "cli@0.1.0");

    // Only change core — cli should cascade.
    add_commit(
        dir.path(),
        "crates/core/src/lib.rs",
        "feat: add core feature",
    );

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("core"))
        .stderr(predicate::str::contains("cli"))
        .stderr(predicate::str::contains("cascade"));
}

#[test]
fn monorepo_cascade_skipped_with_package_filter() {
    let dir = tempfile::tempdir().unwrap();
    init_monorepo(dir.path());
    create_tag(dir.path(), "v0.1.0");
    create_tag(dir.path(), "core@0.1.0");
    create_tag(dir.path(), "cli@0.1.0");

    add_commit(
        dir.path(),
        "crates/core/src/lib.rs",
        "feat: add core feature",
    );

    // With -p, cascade is skipped — only core should appear.
    let output = Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run", "-p", "core"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("core"));
    assert!(!stderr.contains("cascade"));
}

// ── First release tests ────────────────────────────────────────────

#[test]
fn monorepo_first_release_defaults_to_0_1_0() {
    let dir = tempfile::tempdir().unwrap();
    init_monorepo(dir.path());

    // No tags at all — first release.
    add_commit(
        dir.path(),
        "crates/core/src/lib.rs",
        "feat: initial feature",
    );

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("0.1.0"));
}

// ── No changes test ────────────────────────────────────────────────

#[test]
fn monorepo_no_changes_shows_info() {
    let dir = tempfile::tempdir().unwrap();
    init_monorepo(dir.path());
    create_tag(dir.path(), "v0.1.0");
    create_tag(dir.path(), "core@0.1.0");
    create_tag(dir.path(), "cli@0.1.0");

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("no bump-worthy"));
}

// ── Full bump workflow ─────────────────────────────────────────────

#[test]
fn monorepo_full_bump_creates_tags_and_commit() {
    let dir = tempfile::tempdir().unwrap();
    init_monorepo(dir.path());
    create_tag(dir.path(), "v0.1.0");
    create_tag(dir.path(), "core@0.1.0");
    create_tag(dir.path(), "cli@0.1.0");

    add_commit(
        dir.path(),
        "crates/core/src/lib.rs",
        "feat: add core feature",
    );

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Pre-1.0: feat (Minor) downshifts to Patch → core@0.1.1.
    let tags = collect_tag_names(dir.path());
    assert!(tags.iter().any(|t| t == "core@0.1.1"));

    // Verify commit message.
    let msg = head_message(dir.path());
    assert!(msg.starts_with("chore(release):"));
}

#[test]
fn monorepo_no_tag_flag() {
    let dir = tempfile::tempdir().unwrap();
    init_monorepo(dir.path());
    create_tag(dir.path(), "v0.1.0");
    create_tag(dir.path(), "core@0.1.0");
    create_tag(dir.path(), "cli@0.1.0");

    add_commit(
        dir.path(),
        "crates/core/src/lib.rs",
        "feat: add core feature",
    );

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--no-tag"])
        .current_dir(dir.path())
        .assert()
        .success();

    // No new tags should be created (pre-1.0 would be core@0.1.1).
    let tags = collect_tag_names(dir.path());
    assert!(!tags.iter().any(|t| t == "core@0.1.1"));
}

#[test]
fn monorepo_no_commit_flag() {
    let dir = tempfile::tempdir().unwrap();
    init_monorepo(dir.path());
    create_tag(dir.path(), "v0.1.0");
    create_tag(dir.path(), "core@0.1.0");
    create_tag(dir.path(), "cli@0.1.0");

    add_commit(
        dir.path(),
        "crates/core/src/lib.rs",
        "feat: add core feature",
    );

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--no-commit"])
        .current_dir(dir.path())
        .assert()
        .success();

    // HEAD should still be the feature commit, not a release commit.
    let msg = head_message(dir.path());
    assert!(msg.contains("feat: add core feature"));
}

#[test]
fn monorepo_skip_changelog_flag() {
    let dir = tempfile::tempdir().unwrap();
    init_monorepo(dir.path());
    create_tag(dir.path(), "v0.1.0");
    create_tag(dir.path(), "core@0.1.0");
    create_tag(dir.path(), "cli@0.1.0");

    add_commit(
        dir.path(),
        "crates/core/src/lib.rs",
        "feat: add core feature",
    );

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--skip-changelog"])
        .current_dir(dir.path())
        .assert()
        .success();

    // No per-package changelog should be created.
    assert!(!dir.path().join("crates/core/CHANGELOG.md").exists());
}

// ── Changelog command tests ────────────────────────────────────────

#[test]
fn monorepo_changelog_package_flag() {
    let dir = tempfile::tempdir().unwrap();
    init_monorepo(dir.path());
    create_tag(dir.path(), "core@0.1.0");

    add_commit(
        dir.path(),
        "crates/core/src/lib.rs",
        "feat: add core feature",
    );

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["changelog", "-p", "core"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("add core feature"));
}

#[test]
fn monorepo_changelog_unknown_package() {
    let dir = tempfile::tempdir().unwrap();
    init_monorepo(dir.path());

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["changelog", "-p", "nonexistent"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown package"));
}

#[test]
fn changelog_package_requires_monorepo() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["config", "user.email", "test@test.com"]);
    write_file(dir.path(), "f.txt", "init");
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-m", "chore: init"]);

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["changelog", "-p", "core"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("monorepo"));
}

// ── Calver monorepo tests ──────────────────────────────────────────

/// Set up a calver monorepo with two packages.
fn init_calver_monorepo(dir: &Path) {
    git(dir, &["init"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@test.com"]);

    write_file(
        dir,
        "Cargo.toml",
        r#"[workspace]
members = ["crates/core", "crates/cli"]
"#,
    );

    write_file(
        dir,
        "crates/core/Cargo.toml",
        r#"[package]
name = "core"
version = "0.1.0"
edition = "2021"
"#,
    );
    write_file(dir, "crates/core/src/lib.rs", "");

    write_file(
        dir,
        "crates/cli/Cargo.toml",
        r#"[package]
name = "cli"
version = "0.1.0"
edition = "2021"
"#,
    );
    write_file(dir, "crates/cli/src/main.rs", "fn main() {}");

    write_file(
        dir,
        ".git-std.toml",
        r#"monorepo = true
scheme = "calver"
"#,
    );

    git(dir, &["add", "."]);
    git(dir, &["commit", "-m", "chore: init calver monorepo"]);
}

#[test]
fn calver_monorepo_dry_run_shows_date_version() {
    let dir = tempfile::tempdir().unwrap();
    init_calver_monorepo(dir.path());

    add_commit(
        dir.path(),
        "crates/core/src/lib.rs",
        "feat: add core feature",
    );

    // Should produce a date-based version (YYYY.M.0 format).
    let output = Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Version should contain current year.
    let year = chrono_free_current_year();
    assert!(
        stderr.contains(&year),
        "expected calver year {year} in output: {stderr}"
    );
}

#[test]
fn calver_monorepo_patch_increments_same_period() {
    let dir = tempfile::tempdir().unwrap();
    init_calver_monorepo(dir.path());

    let year = chrono_free_current_year();
    let month = chrono_free_current_month();
    let first_tag = format!("core@{year}.{month}.0");
    let root_tag = format!("v{year}.{month}.0");

    add_commit(dir.path(), "crates/core/src/lib.rs", "feat: first feature");
    create_tag(dir.path(), &first_tag);
    create_tag(dir.path(), &root_tag);

    add_commit(dir.path(), "crates/core/src/lib.rs", "feat: second feature");

    let output = Command::cargo_bin("git-std")
        .unwrap()
        .args(["bump", "--dry-run"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let expected_version = format!("{year}.{month}.1");
    assert!(
        stderr.contains(&expected_version),
        "expected {expected_version} in output: {stderr}"
    );
}

/// Get current year as string without chrono dependency.
fn chrono_free_current_year() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let days = secs / 86400;
    // Howard Hinnant algorithm (simplified).
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    y.to_string()
}

/// Get current month (1–12) as string without chrono dependency.
fn chrono_free_current_month() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let days = secs / 86400;
    let z = days + 719468;
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    m.to_string()
}
