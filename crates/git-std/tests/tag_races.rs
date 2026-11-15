mod tag_support;
use tag_support::*;

#[cfg(unix)]
#[test]
fn disappearing_new_tag_is_an_operational_failure() {
    use std::os::unix::fs::PermissionsExt;
    let r = repo("", "1.2.3", "chore(release): 1.2.3");
    let original = std::process::Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .unwrap();
    let original = String::from_utf8(original.stdout).unwrap();
    let bin = r.path().join(".git/bin");
    std::fs::create_dir(&bin).unwrap();
    let wrapper = bin.join("git");
    std::fs::write(&wrapper,format!("#!/bin/sh\n'{}' \"$@\"\nstatus=$?\nfor argument do if [ \"$argument\" = tag ] && [ $status = 0 ]; then '{}' update-ref -d refs/tags/v1.2.3; fi; done\nexit $status\n",original.trim(),original.trim())).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_git-std"))
        .current_dir(r.path())
        .args(["tag", "HEAD", "--format", "json"])
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(git(r.path(), &["tag", "-l"]).is_empty());
    ok(tag(r.path(), &["HEAD"]));
}

#[cfg(unix)]
#[test]
fn publication_uses_the_validated_object_when_local_ref_moves() {
    use std::os::unix::fs::PermissionsExt;
    let r = repo("", "1.2.3", "chore(release): 1.2.3");
    let release = git(r.path(), &["rev-parse", "HEAD"]);
    git(
        r.path(),
        &["commit", "--allow-empty", "-m", "fix: later change"],
    );
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
    let wrapper = bin.join("git");
    std::fs::write(&wrapper,format!("#!/bin/sh\nfor argument do if [ \"$argument\" = push ]; then '{}' -c tag.gpgSign=false tag -f -a v1.2.3 -m 1.2.3 HEAD; fi; done\nexec '{}' \"$@\"\n",original.trim(),original.trim())).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_git-std"))
        .current_dir(r.path())
        .args(["tag", &release, "--push", "origin"])
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert_eq!(
        git(bare.path(), &["rev-parse", "v1.2.3^{commit}"]),
        release,
        "{out:?}"
    );
}
