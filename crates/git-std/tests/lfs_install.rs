#[cfg(unix)]
mod unix {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use assert_cmd::Command;

    fn init(root: &Path) {
        let status = std::process::Command::new("git")
            .arg("init")
            .current_dir(root)
            .status()
            .expect("git init");
        assert!(status.success());
        Command::cargo_bin("git-std")
            .expect("binary")
            .arg("init")
            .env("GIT_STD_HOOKS_ENABLE", "none")
            .current_dir(root)
            .assert()
            .success();
    }

    fn fake_lfs(root: &Path, body: &str) -> String {
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        let executable = bin.join("git-lfs");
        std::fs::write(&executable, body).expect("fake LFS");
        std::fs::set_permissions(executable, std::fs::Permissions::from_mode(0o755))
            .expect("executable LFS");
        format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"))
    }

    #[test]
    fn explicit_install_restores_a_commented_entry_once() {
        let repo = tempfile::tempdir().expect("repository");
        init(repo.path());
        let policy = repo.path().join(".githooks/pre-push.hooks");
        std::fs::write(
            &policy,
            "# existing\n# ! [delete] git lfs pre-push \"$@\"\n! cargo test\n",
        )
        .expect("policy");
        let path = fake_lfs(repo.path(), "#!/bin/sh\nexit 0\n");

        for _ in 0..2 {
            Command::cargo_bin("git-std")
                .expect("binary")
                .args(["lfs", "install"])
                .env("PATH", &path)
                .current_dir(repo.path())
                .assert()
                .success();
        }
        let content = std::fs::read_to_string(policy).expect("policy");
        assert_eq!(
            content
                .lines()
                .filter(|line| *line == "! [delete] git lfs pre-push \"$@\"")
                .count(),
            1
        );
        assert!(!content.contains("# ! [delete] git lfs pre-push \"$@\""));
        assert!(content.contains("! cargo test\n"));
    }

    #[test]
    fn explicit_install_restores_executable_mode_when_enabling_pre_push() {
        let repo = tempfile::tempdir().expect("repository");
        init(repo.path());
        let disabled = repo.path().join(".githooks/pre-push.off");
        std::fs::set_permissions(&disabled, std::fs::Permissions::from_mode(0o644))
            .expect("nonexecutable disabled shim");
        let path = fake_lfs(repo.path(), "#!/bin/sh\nexit 0\n");

        Command::cargo_bin("git-std")
            .expect("binary")
            .args(["lfs", "install"])
            .env("PATH", path)
            .current_dir(repo.path())
            .assert()
            .success();

        let active = repo.path().join(".githooks/pre-push");
        let mode = std::fs::metadata(active)
            .expect("active shim")
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0, "enabled shim must be executable");
    }

    #[test]
    fn explicit_install_restores_executable_mode_on_an_active_managed_shim() {
        let repo = tempfile::tempdir().expect("repository");
        init(repo.path());
        let disabled = repo.path().join(".githooks/pre-push.off");
        let active = repo.path().join(".githooks/pre-push");
        std::fs::rename(disabled, &active).expect("activate managed shim");
        std::fs::set_permissions(&active, std::fs::Permissions::from_mode(0o644))
            .expect("nonexecutable active shim");
        let path = fake_lfs(repo.path(), "#!/bin/sh\nexit 0\n");

        Command::cargo_bin("git-std")
            .expect("binary")
            .args(["lfs", "install"])
            .env("PATH", path)
            .current_dir(repo.path())
            .assert()
            .success();

        let mode = std::fs::metadata(active)
            .expect("active shim")
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0, "managed shim must be executable");
    }

    #[test]
    fn explicit_install_refuses_to_replace_a_custom_shim() {
        let repo = tempfile::tempdir().expect("repository");
        init(repo.path());
        let shim = repo.path().join(".githooks/pre-push.off");
        std::fs::write(&shim, "#!/bin/sh\necho custom\n").expect("custom shim");
        let policy = repo.path().join(".githooks/pre-push.hooks");
        let before = std::fs::read(&policy).expect("policy");
        let path = fake_lfs(repo.path(), "#!/bin/sh\nexit 0\n");

        let output = Command::cargo_bin("git-std")
            .expect("binary")
            .args(["lfs", "install"])
            .env("PATH", path)
            .current_dir(repo.path())
            .output()
            .expect("install");
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("custom"));
        assert_eq!(std::fs::read(policy).expect("policy"), before);
        assert!(!repo.path().join(".githooks/pre-push").exists());
    }

    #[test]
    fn explicit_install_refuses_an_active_custom_shim() {
        let repo = tempfile::tempdir().expect("repository");
        init(repo.path());
        let disabled = repo.path().join(".githooks/pre-push.off");
        let active = repo.path().join(".githooks/pre-push");
        std::fs::rename(&disabled, &active).expect("activate shim");
        let custom = b"#!/bin/sh\necho custom\n";
        std::fs::write(&active, custom).expect("custom shim");
        let policy = repo.path().join(".githooks/pre-push.hooks");
        let before = std::fs::read(&policy).expect("policy");
        let path = fake_lfs(repo.path(), "#!/bin/sh\nexit 0\n");

        let output = Command::cargo_bin("git-std")
            .expect("binary")
            .args(["lfs", "install"])
            .env("PATH", path)
            .current_dir(repo.path())
            .output()
            .expect("install");
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("custom"));
        assert_eq!(std::fs::read(policy).expect("policy"), before);
        assert_eq!(std::fs::read(active).expect("shim"), custom);
    }

    #[test]
    fn explicit_install_requires_git_lfs_before_editing_hooks() {
        let repo = tempfile::tempdir().expect("repository");
        init(repo.path());
        let policy = repo.path().join(".githooks/pre-push.hooks");
        let before = std::fs::read(&policy).expect("policy");
        let path = fake_lfs(repo.path(), "#!/bin/sh\nexit 1\n");

        let output = Command::cargo_bin("git-std")
            .expect("binary")
            .args(["lfs", "install"])
            .env("PATH", path)
            .current_dir(repo.path())
            .output()
            .expect("install");
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("git-lfs is required"));
        assert_eq!(std::fs::read(policy).expect("policy"), before);
        assert!(!repo.path().join(".githooks/pre-push").exists());
    }

    #[test]
    fn failed_lfs_filter_install_preserves_hook_policy() {
        let repo = tempfile::tempdir().expect("repository");
        init(repo.path());
        let policy = repo.path().join(".githooks/pre-push.hooks");
        let before = std::fs::read(&policy).expect("policy");
        let path = fake_lfs(
            repo.path(),
            "#!/bin/sh\nif [ \"$1\" = install ]; then exit 7; fi\nexit 0\n",
        );

        let output = Command::cargo_bin("git-std")
            .expect("binary")
            .args(["lfs", "install"])
            .env("PATH", path)
            .current_dir(repo.path())
            .output()
            .expect("install");
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("git lfs install"));
        assert_eq!(std::fs::read(policy).expect("policy"), before);
        assert!(!repo.path().join(".githooks/pre-push").exists());
    }

    #[test]
    fn explicit_install_keeps_human_tool_output_off_stdout() {
        let repo = tempfile::tempdir().expect("repository");
        init(repo.path());
        let path = fake_lfs(
            repo.path(),
            "#!/bin/sh\nprintf 'LFS tool message\\n'\nexit 0\n",
        );

        let output = Command::cargo_bin("git-std")
            .expect("binary")
            .args(["lfs", "install"])
            .env("PATH", path)
            .current_dir(repo.path())
            .output()
            .expect("install");
        assert!(output.status.success());
        assert!(output.stdout.is_empty(), "stdout should remain pipeable");
        assert!(String::from_utf8_lossy(&output.stderr).contains("Git LFS configured"));
    }
}
