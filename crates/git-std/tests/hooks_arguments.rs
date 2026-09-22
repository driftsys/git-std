use std::path::Path;
use std::process::Stdio;

use assert_cmd::Command;

#[test]
fn commit_msg_commands_receive_all_git_arguments() {
    let repo = tempfile::tempdir().expect("repository");
    git(repo.path(), &["init"]);

    let hooks_dir = repo.path().join(".githooks");
    std::fs::create_dir(&hooks_dir).expect("hooks directory");
    std::fs::write(
        hooks_dir.join("commit-msg.hooks"),
        "! printf '%s\\n%s\\n%s\\n' \"$1\" \"$2\" \"$#\" > hook-args\n",
    )
    .expect("commit-msg commands");

    Command::cargo_bin("git-std")
        .expect("git-std binary")
        .args([
            "hook",
            "run",
            "commit-msg",
            "--",
            ".git/COMMIT_EDITMSG",
            "message",
        ])
        .current_dir(repo.path())
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(repo.path().join("hook-args")).expect("hook arguments"),
        ".git/COMMIT_EDITMSG\nmessage\n2\n"
    );
}

#[cfg(unix)]
#[test]
fn terminal_pre_push_commands_receive_arguments_and_independent_stdin() {
    use std::io::Write;

    let repo = tempfile::tempdir().expect("repository");
    git(repo.path(), &["init"]);

    let hooks_dir = repo.path().join(".githooks");
    std::fs::create_dir(&hooks_dir).expect("hooks directory");
    std::fs::write(
        hooks_dir.join("pre-push.hooks"),
        "! printf '%s\\n%s\\n%s\\n' \"$1\" \"$2\" \"$#\" > first-args; cat > first-input\n\
         ! printf '%s\\n%s\\n%s\\n' \"$1\" \"$2\" \"$#\" > second-args; cat > second-input\n",
    )
    .expect("pre-push commands");

    let binary = Command::cargo_bin("git-std")
        .expect("git-std binary")
        .get_program()
        .to_owned();
    let (pty_master, pty_slave) = terminal_pair();
    let mut child = std::process::Command::new(binary)
        .args([
            "hook",
            "run",
            "pre-push",
            "--",
            "origin",
            "https://example.com/repo.git",
        ])
        .current_dir(repo.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::from(pty_slave))
        .spawn()
        .expect("terminal hook command");

    let input = b"refs/heads/main 111 refs/heads/main 000\n";
    child
        .stdin
        .take()
        .expect("hook stdin")
        .write_all(input)
        .expect("write hook stdin");
    let status = child.wait().expect("hook status");
    drop(pty_master);
    assert!(status.success());

    let expected_args = "origin\nhttps://example.com/repo.git\n2\n";
    for prefix in ["first", "second"] {
        assert_eq!(
            std::fs::read_to_string(repo.path().join(format!("{prefix}-args")))
                .expect("pre-push arguments"),
            expected_args
        );
        assert_eq!(
            std::fs::read(repo.path().join(format!("{prefix}-input"))).expect("pre-push input"),
            input
        );
    }
}

#[cfg(unix)]
fn terminal_pair() -> (std::fs::File, std::fs::File) {
    use std::os::fd::FromRawFd;

    let mut master = -1;
    let mut slave = -1;
    // SAFETY: openpty initializes both file descriptors. The optional name,
    // terminal settings, and window size pointers may be null.
    let result = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(
        result,
        0,
        "open pseudo-terminal: {}",
        std::io::Error::last_os_error()
    );

    // SAFETY: openpty returned two owned, valid descriptors on success.
    unsafe {
        (
            std::fs::File::from_raw_fd(master),
            std::fs::File::from_raw_fd(slave),
        )
    }
}

fn git(dir: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git command");
    assert!(
        output.status.success(),
        "git {args:?} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
