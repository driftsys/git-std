#[cfg(unix)]
#[test]
fn interactive_init_offers_lfs_setup_for_declared_attributes() {
    use std::io::{Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::PermissionsExt;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    let repo = tempfile::tempdir().expect("repository");
    assert!(
        std::process::Command::new("git")
            .arg("init")
            .current_dir(repo.path())
            .status()
            .expect("git init")
            .success()
    );
    std::fs::write(repo.path().join(".gitattributes"), "*.bin filter=lfs\n").expect("attributes");

    let bin = repo.path().join("bin");
    std::fs::create_dir(&bin).expect("bin");
    let fake_lfs = bin.join("git-lfs");
    std::fs::write(&fake_lfs, "#!/bin/sh\nexit 0\n").expect("fake LFS");
    std::fs::set_permissions(&fake_lfs, std::fs::Permissions::from_mode(0o755))
        .expect("executable LFS");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));

    let mut master = -1;
    let mut slave = -1;
    // SAFETY: openpty initializes two owned descriptors on success.
    let result = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(result, 0, "openpty: {}", std::io::Error::last_os_error());
    // SAFETY: the successful openpty call returned these valid descriptors.
    let mut master = unsafe { std::fs::File::from_raw_fd(master) };
    // SAFETY: the successful openpty call returned these valid descriptors.
    let slave = unsafe { std::fs::File::from_raw_fd(slave) };

    let binary = assert_cmd::Command::cargo_bin("git-std")
        .expect("git-std binary")
        .get_program()
        .to_owned();
    let mut child = std::process::Command::new(binary)
        .arg("init")
        .env("GIT_STD_HOOKS_ENABLE", "none")
        .env("PATH", path)
        .current_dir(repo.path())
        .stdin(Stdio::from(slave.try_clone().expect("clone terminal")))
        .stdout(Stdio::from(
            slave.try_clone().expect("clone terminal output"),
        ))
        .stderr(Stdio::from(slave))
        .spawn()
        .expect("interactive init");
    // SAFETY: master is a valid PTY descriptor owned by this test.
    let flags = unsafe { libc::fcntl(master.as_raw_fd(), libc::F_GETFL) };
    assert_ne!(flags, -1);
    // SAFETY: master remains open for the duration of the test.
    assert_ne!(
        unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) },
        -1
    );
    let prompt_deadline = Instant::now() + Duration::from_secs(5);
    let mut transcript = Vec::new();
    while !String::from_utf8_lossy(&transcript).contains("Configure Git LFS for this repository?") {
        let mut chunk = [0; 1024];
        match master.read(&mut chunk) {
            Ok(n) => transcript.extend_from_slice(&chunk[..n]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("terminal read: {error}"),
        }
        assert!(
            Instant::now() < prompt_deadline,
            "missing LFS choice: {}",
            String::from_utf8_lossy(&transcript)
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    master.write_all(b"y\n").expect("accept LFS offer");

    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().expect("init status") {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().expect("terminate hanging prompt");
            panic!("init did not finish after accepting LFS setup");
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    assert!(status.success(), "init exited {status}");
    assert!(
        std::fs::read_to_string(repo.path().join(".githooks/pre-push.hooks"))
            .expect("pre-push policy")
            .contains("git lfs pre-push")
    );
    assert!(repo.path().join(".githooks/pre-push").exists());
    let bootstrap_policy = std::fs::read_to_string(repo.path().join(".githooks/bootstrap.hooks"))
        .expect("bootstrap policy");
    assert!(
        !bootstrap_policy.contains("git lfs pull"),
        "bootstrap already pulls LFS objects; its template must not invite a second pull"
    );
}

#[cfg(unix)]
#[test]
fn noninteractive_init_does_not_enable_lfs() {
    use std::os::unix::fs::PermissionsExt;

    let repo = tempfile::tempdir().expect("repository");
    let status = std::process::Command::new("git")
        .arg("init")
        .current_dir(repo.path())
        .status()
        .expect("git init");
    assert!(status.success());
    std::fs::write(repo.path().join(".gitattributes"), "*.bin filter=lfs\n").expect("attributes");
    let bin = repo.path().join("bin");
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
    let log = repo.path().join("lfs.log");

    let output = assert_cmd::Command::cargo_bin("git-std")
        .expect("binary")
        .arg("init")
        .env("GIT_STD_HOOKS_ENABLE", "none")
        .env("PATH", path)
        .env("GIT_STD_TEST_LFS_LOG", &log)
        .current_dir(repo.path())
        .output()
        .expect("noninteractive init");
    assert!(output.status.success());
    let policy = std::fs::read_to_string(repo.path().join(".githooks/pre-push.hooks"))
        .expect("pre-push policy");
    assert!(!policy.contains("git lfs pre-push"));
    assert!(repo.path().join(".githooks/pre-push.off").exists());
    assert!(
        !log.exists(),
        "init must leave Git LFS setup to an explicit command"
    );
}

#[cfg(unix)]
#[test]
fn interactive_init_does_not_scaffold_when_chosen_lfs_is_unavailable() {
    use std::io::Write;
    use std::os::fd::FromRawFd;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    let repo = tempfile::tempdir().expect("repository");
    assert!(
        std::process::Command::new("git")
            .arg("init")
            .current_dir(repo.path())
            .status()
            .expect("git init")
            .success()
    );
    std::fs::write(repo.path().join(".gitattributes"), "*.bin filter=lfs\n").expect("attributes");
    let bin = repo.path().join("bin");
    std::fs::create_dir(&bin).expect("bin");
    let fake_lfs = bin.join("git-lfs");
    std::fs::write(&fake_lfs, "#!/bin/sh\nexit 1\n").expect("unavailable LFS");
    std::fs::set_permissions(&fake_lfs, std::fs::Permissions::from_mode(0o755))
        .expect("executable LFS");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));

    let mut master = -1;
    let mut slave = -1;
    // SAFETY: openpty initializes two owned descriptors on success.
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0
    );
    // SAFETY: openpty returned these valid descriptors.
    let mut master = unsafe { std::fs::File::from_raw_fd(master) };
    // SAFETY: openpty returned these valid descriptors.
    let slave = unsafe { std::fs::File::from_raw_fd(slave) };
    let binary = assert_cmd::Command::cargo_bin("git-std")
        .expect("git-std binary")
        .get_program()
        .to_owned();
    let mut child = std::process::Command::new(binary)
        .arg("init")
        .env("GIT_STD_HOOKS_ENABLE", "none")
        .env("PATH", path)
        .current_dir(repo.path())
        .stdin(Stdio::from(slave.try_clone().expect("clone terminal")))
        .stderr(Stdio::from(slave))
        .stdout(Stdio::null())
        .spawn()
        .expect("interactive init");
    master.write_all(b"y\n").expect("accept LFS offer");
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().expect("init status") {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().expect("terminate hanging prompt");
            panic!("init did not finish after accepting LFS setup");
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    assert!(!status.success());
    assert!(!repo.path().join(".githooks").exists());
}
