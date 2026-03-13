use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn version_flag_prints_version() {
    Command::cargo_bin("git-std")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_lists_subcommands() {
    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .arg("--help")
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    for sub in [
        "commit",
        "check",
        "bump",
        "changelog",
        "hooks",
        "self-update",
    ] {
        assert!(
            stdout.contains(sub),
            "help output should list '{sub}' subcommand"
        );
    }
}

#[test]
fn unknown_subcommand_exits_2() {
    Command::cargo_bin("git-std")
        .unwrap()
        .arg("does-not-exist")
        .assert()
        .code(2);
}

#[test]
fn stub_subcommands_are_recognized() {
    for sub in ["bump", "changelog", "hooks", "self-update"] {
        Command::cargo_bin("git-std")
            .unwrap()
            .arg(sub)
            .assert()
            .code(1)
            .stderr(predicates::str::contains("not yet implemented"));
    }
}

#[test]
fn commit_dry_run_prints_message() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["commit", "--type", "feat", "-m", "add login", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("feat: add login"));
}

#[test]
fn commit_dry_run_with_scope() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "commit",
            "--type",
            "fix",
            "--scope",
            "auth",
            "-m",
            "handle tokens",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("fix(auth): handle tokens"));
}

#[test]
fn commit_dry_run_with_breaking() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "commit",
            "--type",
            "feat",
            "-m",
            "remove legacy API",
            "--breaking",
            "removed v1 endpoints",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("feat!: remove legacy API"))
        .stdout(predicate::str::contains(
            "BREAKING CHANGE: removed v1 endpoints",
        ));
}

#[test]
fn commit_help_shows_flags() {
    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["commit", "--help"])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    for flag in [
        "--type",
        "--scope",
        "--message",
        "--breaking",
        "--dry-run",
        "--amend",
        "--sign",
        "--all",
    ] {
        assert!(
            stdout.contains(flag),
            "commit help should list '{flag}' flag"
        );
    }
}

#[test]
fn commit_short_flags() {
    // -m alias for --message
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["commit", "--type", "feat", "-m", "short flag", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("feat: short flag"));
}
