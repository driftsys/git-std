#[cfg(unix)]
#[test]
fn managed_push_replays_protocol_to_checks_and_lfs() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::Path;

    fn git(root: &Path, args: &[&str]) {
        let output = std::process::Command::new("git")
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

    let local = tempfile::tempdir().expect("local repository");
    let remote = tempfile::tempdir().expect("remote directory");
    let remote_path = remote.path().join("remote with space.git");
    std::fs::create_dir(&remote_path).expect("remote repository");
    git(&remote_path, &["init", "--bare"]);
    git(local.path(), &["init"]);
    git(local.path(), &["config", "user.name", "Test"]);
    git(local.path(), &["config", "user.email", "test@example.com"]);
    std::fs::write(local.path().join("README.md"), "first commit\n").expect("readme");
    git(local.path(), &["add", "README.md"]);
    git(local.path(), &["commit", "-m", "chore: initialize"]);
    git(
        local.path(),
        &["remote", "add", "origin", remote_path.to_str().unwrap()],
    );

    let binary = assert_cmd::Command::cargo_bin("git-std")
        .expect("git-std binary")
        .get_program()
        .to_owned();
    let bin = local.path().join("bin");
    std::fs::create_dir(&bin).expect("bin");
    symlink(&binary, bin.join("git-std")).expect("git std subcommand");
    let fake_lfs = bin.join("git-lfs");
    std::fs::write(
        &fake_lfs,
        "#!/bin/sh\nif [ \"$1\" = pre-push ]; then\n  printf '%s\\n%s\\n' \"$2\" \"$3\" > lfs-args\n  cat > lfs-input\nfi\nexit 0\n",
    )
    .expect("fake LFS");
    std::fs::set_permissions(&fake_lfs, std::fs::Permissions::from_mode(0o755))
        .expect("executable LFS");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));

    let init = std::process::Command::new(&binary)
        .arg("init")
        .env("GIT_STD_HOOKS_ENABLE", "none")
        .current_dir(local.path())
        .output()
        .expect("init");
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    std::fs::write(
        local.path().join(".githooks/pre-push.hooks"),
        "! printf '%s\\n%s\\n' \"$1\" \"$2\" > check-args; cat > check-input\n",
    )
    .expect("check policy");
    let install = std::process::Command::new(&binary)
        .args(["lfs", "install"])
        .env("PATH", &path)
        .current_dir(local.path())
        .output()
        .expect("LFS install");
    assert!(
        install.status.success(),
        "{}",
        String::from_utf8_lossy(&install.stderr)
    );

    let push = std::process::Command::new("git")
        .args(["push", "origin", "HEAD:refs/heads/main"])
        .env("PATH", path)
        .current_dir(local.path())
        .output()
        .expect("push");
    assert!(
        push.status.success(),
        "{}",
        String::from_utf8_lossy(&push.stderr)
    );

    let expected_args = format!("origin\n{}\n", remote_path.display());
    assert_eq!(
        std::fs::read_to_string(local.path().join("check-args")).expect("check args"),
        expected_args
    );
    assert_eq!(
        std::fs::read_to_string(local.path().join("lfs-args")).expect("LFS args"),
        expected_args
    );
    let check_input = std::fs::read(local.path().join("check-input")).expect("check input");
    let lfs_input = std::fs::read(local.path().join("lfs-input")).expect("LFS input");
    let oid = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(local.path())
        .output()
        .expect("commit OID");
    assert!(oid.status.success());
    let oid = String::from_utf8_lossy(&oid.stdout);
    let expected = format!("HEAD {} refs/heads/main {}\n", oid.trim(), "0".repeat(40));
    assert_eq!(check_input, expected.as_bytes());
    assert_eq!(lfs_input, expected.as_bytes());
}
