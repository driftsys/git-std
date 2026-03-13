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
    for sub in [
        "commit",
        "check",
        "bump",
        "changelog",
        "hooks",
        "self-update",
    ] {
        Command::cargo_bin("git-std")
            .unwrap()
            .arg(sub)
            .assert()
            .code(1)
            .stderr(predicates::str::contains("not yet implemented"));
    }
}
