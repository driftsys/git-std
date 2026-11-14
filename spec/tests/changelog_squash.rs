#[path = "../support/mod.rs"]
mod support;

use std::process::Command;
use support::TestRepo;

const NOTE: &str = "SignalReader requires checkWith.\n\nKeep the last good value.";

fn fixture() -> TestRepo {
    let mut repo = TestRepo::new()
        .with_config("monorepo = false\n")
        .with_project_toml("1.0.0");
    repo.git(&["add", "."]);
    repo.add_commit("chore: initial release");
    repo.create_tag("v1.0.0");
    repo.git(&[
        "remote",
        "add",
        "origin",
        "https://github.com/example/current.git",
    ]);
    repo.add_commit(&format!("feat: generate Flows (#112)\n\nBREAKING CHANGE: {NOTE}\nRefs #106\n\n* feat: nested commit\n\nPasted discussion of driftsys/ridl#1177.\n"));
    repo.add_commit(&format!("fix: adapt driftsys/ridl#1177\n\nBREAKING CHANGE: {NOTE}\nRefs: #1177, driftsys/ridl#1177, driftsys/ridl#2088\n"));
    repo
}

fn assert_section(output: &str) {
    assert!(!output.contains("nested commit"), "{output}");
    assert!(!output.contains("Pasted discussion"), "{output}");
    assert_eq!(output.matches(NOTE).count(), 1, "{output}");
    assert!(output.contains("refs [#106]"), "{output}");
    assert!(output.contains("adapt driftsys/ridl#1177"), "{output}");
    assert!(!output.contains("driftsys/ridl[#1177]"), "{output}");
    assert_eq!(output.matches("driftsys/ridl#1177").count(), 2, "{output}");
    assert_eq!(output.matches("driftsys/ridl#2088").count(), 1, "{output}");
    assert!(!output.contains("issues/2088"), "{output}");
    assert!(
        output.contains("[#1177]: https://github.com/example/current/issues/1177"),
        "{output}"
    );
}

#[test]
fn changelog_extracts_squash_references_without_body_leakage() {
    let repo = fixture();
    let output = Command::new(TestRepo::bin_path())
        .current_dir(repo.path())
        .args(["changelog"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_section(&String::from_utf8(output.stdout).unwrap());
}

#[test]
fn bump_uses_the_same_squash_changelog_extraction() {
    let repo = fixture();
    let output = Command::new(TestRepo::bin_path())
        .current_dir(repo.path())
        .args(["bump", "--yes", "--no-commit", "--release-as", "patch"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_section(&repo.read("CHANGELOG.md"));
}
