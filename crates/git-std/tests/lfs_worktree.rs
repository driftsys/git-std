#[cfg(unix)]
#[test]
fn lfs_install_and_bootstrap_work_from_a_linked_worktree_subdirectory() {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::process::Command;

    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("git invocation");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let parent = tempfile::tempdir().expect("parent");
    let source = parent.path().join("source");
    let linked = parent.path().join("linked");
    std::fs::create_dir(&source).expect("source");
    git(&source, &["init"]);
    git(&source, &["config", "user.name", "Test"]);
    git(&source, &["config", "user.email", "test@example.com"]);
    std::fs::write(source.join(".gitattributes"), "*.bin filter=lfs\n").expect("attributes");
    git(&source, &["add", ".gitattributes"]);
    git(&source, &["commit", "-m", "chore: declare LFS"]);
    git(
        &source,
        &["worktree", "add", "--detach", linked.to_str().unwrap()],
    );
    let nested = linked.join("nested");
    std::fs::create_dir(&nested).expect("nested directory");

    let bin = linked.join("bin");
    std::fs::create_dir(&bin).expect("bin");
    let executable = bin.join("git-lfs");
    std::fs::write(
        &executable,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$GIT_STD_TEST_LFS_LOG\"\nexit 0\n",
    )
    .expect("fake LFS");
    std::fs::set_permissions(executable, std::fs::Permissions::from_mode(0o755))
        .expect("executable LFS");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    let log = linked.join("lfs.log");

    let init = assert_cmd::Command::cargo_bin("git-std")
        .expect("binary")
        .arg("init")
        .env("GIT_STD_HOOKS_ENABLE", "none")
        .current_dir(&nested)
        .output()
        .expect("init");
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    let install = assert_cmd::Command::cargo_bin("git-std")
        .expect("binary")
        .args(["lfs", "install"])
        .env("PATH", &path)
        .env("GIT_STD_TEST_LFS_LOG", &log)
        .current_dir(&nested)
        .output()
        .expect("install");
    assert!(
        install.status.success(),
        "{}",
        String::from_utf8_lossy(&install.stderr)
    );
    let bootstrap = assert_cmd::Command::cargo_bin("git-std")
        .expect("binary")
        .arg("bootstrap")
        .env("PATH", &path)
        .env("GIT_STD_TEST_LFS_LOG", &log)
        .current_dir(&nested)
        .output()
        .expect("bootstrap");
    assert!(
        bootstrap.status.success(),
        "{}",
        String::from_utf8_lossy(&bootstrap.stderr)
    );
    assert!(linked.join(".githooks/pre-push").exists());
    assert!(
        std::fs::read_to_string(linked.join(".githooks/pre-push.hooks"))
            .expect("policy")
            .contains("! [delete] git lfs pre-push \"$@\"")
    );
    assert_eq!(
        std::fs::read_to_string(log).expect("LFS calls"),
        "version\ninstall --local --skip-repo\nversion\ninstall --local --skip-repo\npull\n"
    );
}
