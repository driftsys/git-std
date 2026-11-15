mod tag_support;
use tag_support::*;

#[cfg(unix)]
fn executable(path: &std::path::Path, content: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, content).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(unix)]
#[test]
fn signed_retry_without_trust_configuration_preserves_tag() {
    let r = repo("", "1.2.3", "chore(release): 1.2.3");
    let key = r.path().join(".git/signing-key");
    let o = std::process::Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-f", key.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(o.status.success(), "{o:?}");
    git(r.path(), &["config", "gpg.format", "ssh"]);
    git(
        r.path(),
        &["config", "user.signingkey", key.to_str().unwrap()],
    );
    ok(tag(r.path(), &["HEAD", "--sign"]));
    let oid = git(r.path(), &["rev-parse", "v1.2.3"]);
    let v = ok(tag(r.path(), &["HEAD", "--sign", "--format", "json"]));
    assert_eq!(v["tags"][0]["signature"], "unverified");
    assert_eq!(git(r.path(), &["rev-parse", "v1.2.3"]), oid);
    ok(tag(r.path(), &["HEAD"]));
}

#[test]
fn explicit_unsigned_mode_overrides_git_signing_default() {
    let r = repo("", "1.2.3", "chore(release): 1.2.3");
    git(r.path(), &["config", "tag.gpgSign", "true"]);
    let v = ok(tag(r.path(), &["HEAD", "--format", "json"]));
    assert_eq!(v["tags"][0]["signature"], "unsigned");
}

#[cfg(unix)]
#[test]
fn partial_local_creation_reports_state_and_retry_finishes_missing_tag() {
    let r = repo(
        "monorepo=true\n[[packages]]\nname='api'\npath='api'",
        "1.2.3",
        "chore: initial",
    );
    std::fs::create_dir(r.path().join("api")).unwrap();
    std::fs::write(r.path().join("api/VERSION"), "2.0.0").unwrap();
    git(r.path(), &["add", "."]);
    git(
        r.path(),
        &["commit", "-m", "chore(release): v1.2.3, api@2.0.0"],
    );
    let hook = r.path().join(".git/hooks/reference-transaction");
    executable(
        &hook,
        "#!/bin/sh\nif [ \"$1\" = prepared ]; then while read old new ref; do case \"$ref\" in refs/tags/api*) exit 1;; esac; done; fi\nexit 0\n",
    );
    let out = tag(r.path(), &["HEAD", "--format", "json"]);
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["tags"][0]["local"], "created");
    let oid = git(r.path(), &["rev-parse", "v1.2.3"]);
    std::fs::remove_file(hook).unwrap();
    ok(tag(r.path(), &["HEAD"]));
    assert_eq!(git(r.path(), &["rev-parse", "v1.2.3"]), oid);
    assert_eq!(
        git(r.path(), &["rev-parse", "api@2.0.0^{commit}"]),
        git(r.path(), &["rev-parse", "HEAD"])
    );
}

#[cfg(unix)]
#[test]
fn lost_push_response_is_recovered_without_another_bump() {
    let r = repo("", "1.2.3", "chore(release): 1.2.3");
    let bare = tempfile::tempdir().unwrap();
    git(bare.path(), &["init", "--bare"]);
    git(
        r.path(),
        &["remote", "add", "origin", bare.path().to_str().unwrap()],
    );
    git(r.path(), &["push", "origin", "main"]);
    let original = std::process::Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .unwrap();
    let original = String::from_utf8(original.stdout).unwrap();
    let bin = r.path().join(".git/bin");
    std::fs::create_dir(&bin).unwrap();
    executable(
        &bin.join("git"),
        &format!(
            "#!/bin/sh\n'{}' \"$@\"\nstatus=$?\nfor argument do if [ \"$argument\" = push ] && [ $status = 0 ]; then exit 1; fi; done\nexit $status\n",
            original.trim()
        ),
    );
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_git-std"))
        .current_dir(r.path())
        .args(["tag", "HEAD", "--push", "origin", "--format", "json"])
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["tags"][0]["remote"], "unknown");
    let oid = git(bare.path(), &["rev-parse", "v1.2.3"]);
    let head = git(r.path(), &["rev-parse", "HEAD"]);
    ok(tag(r.path(), &["HEAD", "--push", "origin"]));
    assert_eq!(git(r.path(), &["rev-parse", "HEAD"]), head);
    assert_eq!(git(r.path(), &["rev-parse", "v1.2.3"]), oid);
}
