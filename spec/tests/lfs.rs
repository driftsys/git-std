#[path = "../support/mod.rs"]
mod support;

use std::process::Command;

use support::TestRepo;

#[cfg(unix)]
#[test]
fn lfs_install_adds_upload_to_an_existing_managed_hook() {
    use std::os::unix::fs::PermissionsExt;

    let repo = TestRepo::new();
    let init = Command::new(TestRepo::bin_path())
        .args(["init"])
        .env("GIT_STD_HOOKS_ENABLE", "none")
        .current_dir(repo.path())
        .output()
        .expect("init");
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );

    let hooks = repo.path().join(".githooks/pre-push.hooks");
    std::fs::write(&hooks, "# existing policy\n! cargo test\n").expect("policy");

    let bin = repo.path().join("bin");
    std::fs::create_dir(&bin).expect("bin");
    let fake_lfs = bin.join("git-lfs");
    std::fs::write(
        &fake_lfs,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$GIT_STD_TEST_LFS_LOG\"\nprintf 'tool output\\n'\nexit 0\n",
    )
    .expect("fake LFS");
    std::fs::set_permissions(&fake_lfs, std::fs::Permissions::from_mode(0o755))
        .expect("executable LFS");
    let log = repo.path().join("lfs.log");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));

    for _ in 0..2 {
        let output = Command::new(TestRepo::bin_path())
            .args(["lfs", "install"])
            .env("PATH", &path)
            .env("GIT_STD_TEST_LFS_LOG", &log)
            .current_dir(repo.path())
            .output()
            .expect("LFS install");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let content = std::fs::read_to_string(&hooks).expect("hooks file");
    assert!(content.starts_with("# existing policy\n! cargo test\n"));
    assert_eq!(
        content
            .matches("! [delete] git lfs pre-push \"$@\"")
            .count(),
        1
    );
    assert!(repo.path().join(".githooks/pre-push").exists());
    assert!(!repo.path().join(".githooks/pre-push.off").exists());
    assert_eq!(
        std::fs::read_to_string(log).expect("LFS invocations"),
        "version\ninstall --local --skip-repo\nversion\ninstall --local --skip-repo\n"
    );
}

#[cfg(unix)]
#[test]
fn bootstrap_configures_local_lfs_without_enabling_upload() {
    use std::os::unix::fs::PermissionsExt;

    let repo = TestRepo::new();
    let init = Command::new(TestRepo::bin_path())
        .args(["init"])
        .env("GIT_STD_HOOKS_ENABLE", "none")
        .current_dir(repo.path())
        .output()
        .expect("init");
    assert!(init.status.success());
    std::fs::write(repo.path().join(".gitattributes"), "*.bin filter=lfs\n").expect("attributes");
    let hooks = repo.path().join(".githooks/pre-push.hooks");
    std::fs::write(
        &hooks,
        "# maintainer choice\n# ! [delete] git lfs pre-push \"$@\"\n",
    )
    .expect("disabled LFS policy");
    let original_policy = std::fs::read(&hooks).expect("policy");
    let disabled = repo.path().join(".githooks/pre-push.off");
    let original_shim = std::fs::read(&disabled).expect("disabled shim");

    let bin = repo.path().join("bin");
    std::fs::create_dir(&bin).expect("bin");
    let fake_lfs = bin.join("git-lfs");
    std::fs::write(
        &fake_lfs,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$GIT_STD_TEST_LFS_LOG\"\nprintf 'tool output\\n'\nexit 0\n",
    )
    .expect("fake LFS");
    std::fs::set_permissions(&fake_lfs, std::fs::Permissions::from_mode(0o755))
        .expect("executable LFS");
    let log = repo.path().join("lfs.log");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));

    let output = Command::new(TestRepo::bin_path())
        .arg("bootstrap")
        .env("PATH", path)
        .env("GIT_STD_TEST_LFS_LOG", &log)
        .current_dir(repo.path())
        .output()
        .expect("bootstrap");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "bootstrap writes only human output to stderr"
    );
    assert_eq!(
        std::fs::read_to_string(log).expect("LFS invocations"),
        "version\ninstall --local --skip-repo\npull\n"
    );
    assert_eq!(std::fs::read(hooks).expect("policy"), original_policy);
    assert_eq!(std::fs::read(disabled).expect("shim"), original_shim);
    assert!(!repo.path().join(".githooks/pre-push").exists());
}

#[cfg(unix)]
#[test]
fn bootstrap_detects_nested_lfs_declarations() {
    use std::os::unix::fs::PermissionsExt;

    let repo = TestRepo::new();
    std::fs::create_dir(repo.path().join(".githooks")).expect("managed hooks");
    std::fs::create_dir(repo.path().join("assets")).expect("assets");
    std::fs::write(
        repo.path().join("assets/.gitattributes"),
        "*.psd filter=lfs diff=lfs merge=lfs -text\n",
    )
    .expect("nested attributes");

    let bin = repo.path().join("bin");
    std::fs::create_dir(&bin).expect("bin");
    let fake_lfs = bin.join("git-lfs");
    std::fs::write(
        &fake_lfs,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$GIT_STD_TEST_LFS_LOG\"\nexit 0\n",
    )
    .expect("fake LFS");
    std::fs::set_permissions(&fake_lfs, std::fs::Permissions::from_mode(0o755))
        .expect("executable LFS");
    let log = repo.path().join("lfs.log");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));

    let output = Command::new(TestRepo::bin_path())
        .arg("bootstrap")
        .env("PATH", path)
        .env("GIT_STD_TEST_LFS_LOG", &log)
        .current_dir(repo.path())
        .output()
        .expect("bootstrap");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(log).expect("LFS invocations"),
        "version\ninstall --local --skip-repo\npull\n"
    );
}

#[test]
fn doctor_explains_missing_lfs_upload_integration() {
    let repo = TestRepo::new();
    let init = Command::new(TestRepo::bin_path())
        .args(["init"])
        .env("GIT_STD_HOOKS_ENABLE", "none")
        .current_dir(repo.path())
        .output()
        .expect("init");
    assert!(init.status.success());
    std::fs::write(repo.path().join(".gitattributes"), "*.bin filter=lfs\n").expect("attributes");

    let output = Command::new(TestRepo::bin_path())
        .arg("doctor")
        .current_dir(repo.path())
        .output()
        .expect("doctor");
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostics.contains("git std lfs install"),
        "doctor should explain how to enable LFS uploads: {diagnostics}"
    );
}

#[cfg(unix)]
#[test]
fn doctor_recognizes_managed_lfs_upload_integration() {
    use std::os::unix::fs::PermissionsExt;

    let repo = TestRepo::new();
    let init = Command::new(TestRepo::bin_path())
        .arg("init")
        .env("GIT_STD_HOOKS_ENABLE", "none")
        .current_dir(repo.path())
        .output()
        .expect("init");
    assert!(init.status.success());
    std::fs::write(repo.path().join(".gitattributes"), "*.bin filter=lfs\n").expect("attributes");
    let bin = repo.path().join("bin");
    std::fs::create_dir(&bin).expect("bin");
    let executable = bin.join("git-lfs");
    std::fs::write(&executable, "#!/bin/sh\nexit 0\n").expect("fake LFS");
    std::fs::set_permissions(executable, std::fs::Permissions::from_mode(0o755))
        .expect("executable LFS");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    let install = Command::new(TestRepo::bin_path())
        .args(["lfs", "install"])
        .env("PATH", &path)
        .current_dir(repo.path())
        .output()
        .expect("install");
    assert!(install.status.success());

    let doctor = Command::new(TestRepo::bin_path())
        .arg("doctor")
        .env("PATH", path)
        .current_dir(repo.path())
        .output()
        .expect("doctor");
    let diagnostics = String::from_utf8_lossy(&doctor.stderr);
    assert!(
        !diagnostics.contains("LFS uploads are not enabled"),
        "{diagnostics}"
    );
    assert!(!diagnostics.contains("unverified"), "{diagnostics}");
}

#[cfg(unix)]
#[test]
fn bootstrap_ignores_comments_and_similar_filter_names() {
    use std::os::unix::fs::PermissionsExt;

    let repo = TestRepo::new();
    std::fs::write(
        repo.path().join(".gitattributes"),
        "# *.psd filter=lfs\n*.bin filter=lfs-other\n",
    )
    .expect("attributes");
    let bin = repo.path().join("bin");
    std::fs::create_dir(&bin).expect("bin");
    let fake_lfs = bin.join("git-lfs");
    std::fs::write(
        &fake_lfs,
        "#!/bin/sh\nprintf 'called' > lfs-called\nexit 0\n",
    )
    .expect("fake LFS");
    std::fs::set_permissions(fake_lfs, std::fs::Permissions::from_mode(0o755))
        .expect("executable LFS");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));

    let output = Command::new(TestRepo::bin_path())
        .arg("bootstrap")
        .env("PATH", path)
        .current_dir(repo.path())
        .output()
        .expect("bootstrap");
    assert!(output.status.success());
    assert!(!repo.path().join("lfs-called").exists());
}

#[cfg(unix)]
#[test]
fn bootstrap_reports_missing_lfs_for_declared_rules() {
    use std::os::unix::fs::PermissionsExt;

    let repo = TestRepo::new();
    std::fs::write(repo.path().join(".gitattributes"), "*.bin filter=lfs\n").expect("attributes");
    let bin = repo.path().join("bin");
    std::fs::create_dir(&bin).expect("bin");
    let fake_lfs = bin.join("git-lfs");
    std::fs::write(&fake_lfs, "#!/bin/sh\nexit 1\n").expect("unavailable LFS");
    std::fs::set_permissions(fake_lfs, std::fs::Permissions::from_mode(0o755))
        .expect("executable LFS");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));

    let output = Command::new(TestRepo::bin_path())
        .arg("bootstrap")
        .env("PATH", path)
        .current_dir(repo.path())
        .output()
        .expect("bootstrap");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("git-lfs is required"));
}

#[test]
fn doctor_marks_custom_pre_push_lfs_integration_as_unverified() {
    let repo = TestRepo::new();
    let init = Command::new(TestRepo::bin_path())
        .args(["init"])
        .env("GIT_STD_HOOKS_ENABLE", "none")
        .current_dir(repo.path())
        .output()
        .expect("init");
    assert!(init.status.success());
    std::fs::write(repo.path().join(".gitattributes"), "*.bin filter=lfs\n").expect("attributes");
    std::fs::write(
        repo.path().join(".githooks/pre-push"),
        "#!/bin/sh\ngit lfs pre-push \"$@\"\n",
    )
    .expect("custom shim");

    let output = Command::new(TestRepo::bin_path())
        .arg("doctor")
        .current_dir(repo.path())
        .output()
        .expect("doctor");
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostics.contains("unverified"), "{diagnostics}");
    assert!(
        !diagnostics.contains("LFS uploads are not enabled"),
        "{diagnostics}"
    );
}
