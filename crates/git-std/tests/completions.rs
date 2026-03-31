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
fn completions_bash_includes_git_subcommand_wrapper() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicates::str::contains("_git_std"));
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
fn completions_zsh_includes_git_subcommand_wrapper() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicates::str::contains("user-commands std:"));
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

#[test]
fn completions_fish_includes_git_subcommand_wrapper() {
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicates::str::contains("complete -f -c git"));
}
