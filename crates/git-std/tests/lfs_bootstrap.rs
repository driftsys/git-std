#[cfg(unix)]
mod unix {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::process::Command;

    fn repo() -> tempfile::TempDir {
        let repo = tempfile::tempdir().expect("repository");
        let status = Command::new("git")
            .arg("init")
            .current_dir(repo.path())
            .status()
            .expect("git init");
        assert!(status.success());
        repo
    }

    fn fake_lfs(root: &Path) -> String {
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        let executable = bin.join("git-lfs");
        std::fs::write(
            &executable,
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$GIT_STD_TEST_LFS_LOG\"\nexit 0\n",
        )
        .expect("fake LFS");
        std::fs::set_permissions(executable, std::fs::Permissions::from_mode(0o755))
            .expect("executable LFS");
        format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"))
    }

    #[test]
    fn bootstrap_uses_local_install_even_without_managed_hooks() {
        let repo = repo();
        std::fs::write(repo.path().join(".gitattributes"), "*.bin filter=lfs\n")
            .expect("attributes");
        let path = fake_lfs(repo.path());
        let log = repo.path().join("lfs.log");

        let output = assert_cmd::Command::cargo_bin("git-std")
            .expect("binary")
            .arg("bootstrap")
            .env("PATH", path)
            .env("GIT_STD_TEST_LFS_LOG", &log)
            .current_dir(repo.path())
            .output()
            .expect("bootstrap");
        assert!(output.status.success());
        assert_eq!(
            std::fs::read_to_string(log).expect("LFS commands"),
            "version\ninstall --local --skip-repo\npull\n"
        );
        assert!(!repo.path().join(".githooks").exists());
    }

    #[test]
    fn bootstrap_dry_run_does_not_install_or_pull_lfs() {
        let repo = repo();
        std::fs::write(repo.path().join(".gitattributes"), "*.bin filter=lfs\n")
            .expect("attributes");
        let path = fake_lfs(repo.path());
        let log = repo.path().join("lfs.log");

        let output = assert_cmd::Command::cargo_bin("git-std")
            .expect("binary")
            .args(["bootstrap", "--dry-run"])
            .env("PATH", path)
            .env("GIT_STD_TEST_LFS_LOG", &log)
            .current_dir(repo.path())
            .output()
            .expect("bootstrap dry-run");
        assert!(output.status.success());
        assert_eq!(
            std::fs::read_to_string(log).expect("LFS commands"),
            "version\n"
        );
        assert!(!repo.path().join(".githooks").exists());
        let local = Command::new("git")
            .args(["config", "--local", "--get", "filter.lfs.clean"])
            .current_dir(repo.path())
            .output()
            .expect("local config");
        assert!(!local.status.success());
    }

    #[test]
    fn deleted_tracked_attributes_do_not_block_bootstrap() {
        let repo = repo();
        let attributes = repo.path().join(".gitattributes");
        std::fs::write(&attributes, "*.bin filter=lfs\n").expect("attributes");
        let add = Command::new("git")
            .args(["add", ".gitattributes"])
            .current_dir(repo.path())
            .status()
            .expect("git add");
        assert!(add.success());
        std::fs::remove_file(attributes).expect("delete attributes");

        let output = assert_cmd::Command::cargo_bin("git-std")
            .expect("binary")
            .args(["bootstrap", "--dry-run"])
            .current_dir(repo.path())
            .output()
            .expect("bootstrap");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn bootstrap_reports_filter_install_and_pull_failures() {
        for (failing_command, expected_message) in [
            ("install", "git lfs install failed"),
            ("pull", "git lfs pull failed"),
        ] {
            let repo = repo();
            std::fs::write(repo.path().join(".gitattributes"), "*.bin filter=lfs\n")
                .expect("attributes");
            let path = fake_lfs(repo.path());
            std::fs::write(
                repo.path().join("bin/git-lfs"),
                format!("#!/bin/sh\nif [ \"$1\" = {failing_command} ]; then exit 7; fi\nexit 0\n"),
            )
            .expect("failing LFS");

            let output = assert_cmd::Command::cargo_bin("git-std")
                .expect("binary")
                .arg("bootstrap")
                .env("PATH", path)
                .current_dir(repo.path())
                .output()
                .expect("bootstrap");
            assert!(!output.status.success(), "{failing_command}");
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(expected_message),
                "{failing_command}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
