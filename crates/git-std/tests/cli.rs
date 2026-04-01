use assert_cmd::Command;

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
    for sub in ["commit", "lint", "bump", "changelog", "hook", "config"] {
        assert!(
            stdout.contains(sub),
            "help output should list '{sub}' subcommand"
        );
    }
    assert!(
        stdout.contains("--completions"),
        "help output should list '--completions' global flag"
    );
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
fn hook_requires_subcommand() {
    Command::cargo_bin("git-std")
        .unwrap()
        .arg("hook")
        .assert()
        .code(2);
}
