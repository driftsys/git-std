use assert_cmd::Command;

#[test]
fn completions_bash_outputs_script() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicates::str::contains("complete"));
}

#[test]
fn completions_zsh_outputs_script() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicates::str::contains("_git-std"));
}

#[test]
fn completions_fish_outputs_script() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicates::str::contains("complete"));
}
