#[path = "../support/mod.rs"]
mod support;

use serde_json::Value;
use std::path::Path;
use std::process::Command;
use support::TestRepo;

fn bare_remote() -> tempfile::TempDir {
    let remote = tempfile::tempdir().unwrap();
    let initialized = Command::new("git")
        .args(["init", "--bare", "--initial-branch=main"])
        .arg(remote.path())
        .output()
        .unwrap();
    assert!(initialized.status.success());
    remote
}

fn remote_ref(directory: &Path, reference: &str) -> String {
    let output = Command::new("git")
        .arg("--git-dir")
        .arg(directory)
        .args(["rev-parse", "--verify", reference])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn remote_refs(directory: &Path) -> Vec<u8> {
    let output = Command::new("git")
        .arg("--git-dir")
        .arg(directory)
        .args(["for-each-ref", "--format=%(refname) %(objectname)"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    output.stdout
}

fn run_json(repo: &TestRepo, args: &[&str]) -> Value {
    let output = Command::new(TestRepo::bin_path())
        .args(args)
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn guarded_push_results_match_the_declared_remote_and_published_refs() {
    for remote_name in ["origin", "upstream"] {
        let mut repo = TestRepo::new().with_config(
        "monorepo = false\n\n[[version_files]]\npath = \"VERSION\"\nregex = '^(\\d+\\.\\d+\\.\\d+)$'\n",
    );
        repo.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);
        repo.write("VERSION", "1.0.0\n");
        repo.git(&["add", "."]);
        repo.add_commit("chore: init");
        repo.create_tag("v1.0.0");
        repo.add_commit("feat: add feature");
        let remote = bare_remote();
        repo.git(&[
            "remote",
            "add",
            remote_name,
            remote.path().to_str().unwrap(),
        ]);
        repo.git(&["push", "-u", remote_name, "main"]);
        let decoy = (remote_name != "origin").then(bare_remote);
        if let Some(decoy) = &decoy {
            repo.git(&["remote", "add", "origin", decoy.path().to_str().unwrap()]);
            repo.git(&["push", "origin", "main"]);
        }
        let decoy_refs = decoy
            .as_ref()
            .map(|directory| remote_refs(directory.path()));
        let before = remote_ref(remote.path(), "refs/heads/main");
        let refs_before = remote_refs(remote.path());

        let plan = run_json(
            &repo,
            &[
                "bump",
                "--dry-run",
                "--push",
                remote_name,
                "--format",
                "json",
            ],
        );
        let push = plan["effects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|effect| effect["kind"] == "push")
            .expect("planned push effect");
        assert_eq!(push["target"], remote_name);
        assert_eq!(push["fidelity"], "conditional");
        assert_eq!(remote_ref(remote.path(), "refs/heads/main"), before);
        assert_eq!(remote_refs(remote.path()), refs_before);
        assert_eq!(
            decoy
                .as_ref()
                .map(|directory| remote_refs(directory.path())),
            decoy_refs
        );
        let result = run_json(
            &repo,
            &[
                "bump",
                "--push",
                remote_name,
                "--expect-plan",
                plan["plan_id"].as_str().unwrap(),
                "--yes",
                "--format",
                "json",
            ],
        );

        assert_eq!(result["plan_id"], plan["plan_id"]);
        assert_eq!(result["pushed_to"], push["target"]);
        let commit = result["commit_oid"].as_str().unwrap();
        let tag = result["tag_oid"].as_str().unwrap();
        assert_ne!(commit, before);
        assert_eq!(commit, remote_ref(remote.path(), "refs/heads/main"));
        assert_eq!(tag, remote_ref(remote.path(), "refs/tags/v1.1.0"));
        assert_eq!(commit, remote_ref(remote.path(), "refs/tags/v1.1.0^{}"));
        assert_eq!(commit, repo.git(&["rev-parse", "HEAD"]));
        assert_eq!(tag, repo.git(&["rev-parse", "refs/tags/v1.1.0"]));
        assert_eq!(
            decoy
                .as_ref()
                .map(|directory| remote_refs(directory.path())),
            decoy_refs
        );
    }
}
