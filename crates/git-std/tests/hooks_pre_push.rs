use std::io::{Read, Write};
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use assert_cmd::Command;

#[test]
fn deletion_only_push_runs_only_delete_commands() {
    let local = tempfile::tempdir().expect("local repository");
    let remote = tempfile::tempdir().expect("remote repository");

    git(remote.path(), &["init", "--bare"]);
    git(local.path(), &["init"]);
    git(local.path(), &["config", "user.name", "Test"]);
    git(local.path(), &["config", "user.email", "test@example.com"]);
    std::fs::write(local.path().join("initial.txt"), "initial\n").expect("initial file");
    git(local.path(), &["add", "initial.txt"]);
    git(local.path(), &["commit", "-m", "chore: initialize"]);
    git(
        local.path(),
        &["remote", "add", "origin", remote.path().to_str().unwrap()],
    );
    git(local.path(), &["push", "origin", "HEAD:refs/heads/topic"]);

    let hooks_dir = local.path().join(".githooks");
    std::fs::create_dir(&hooks_dir).expect("hooks directory");
    std::fs::write(
        hooks_dir.join("pre-push.hooks"),
        "! touch ordinary-ran\n! [delete] touch deletion-ran\n",
    )
    .expect("pre-push commands");

    let binary = Command::cargo_bin("git-std")
        .expect("git-std binary")
        .get_program()
        .to_owned();
    let shim = format!(
        "#!/bin/sh\nexec \"{}\" hook run pre-push -- \"$@\"\n",
        binary.to_string_lossy()
    );
    let shim_path = hooks_dir.join("pre-push");
    std::fs::write(&shim_path, shim).expect("pre-push shim");
    make_executable(&shim_path);
    git(local.path(), &["config", "core.hooksPath", ".githooks"]);

    git(local.path(), &["push", "origin", "--delete", "topic"]);

    assert!(
        !local.path().join("ordinary-ran").exists(),
        "ordinary checks should be skipped"
    );
    assert!(
        local.path().join("deletion-ran").exists(),
        "[delete] checks should run"
    );
    let deleted_ref = std::process::Command::new("git")
        .args(["show-ref", "--verify", "--quiet", "refs/heads/topic"])
        .current_dir(remote.path())
        .status()
        .expect("inspect remote ref");
    assert!(!deleted_ref.success(), "remote branch should be deleted");
}

#[cfg(unix)]
#[test]
fn json_capture_drains_output_while_replaying_large_stdin() {
    let repo = tempfile::tempdir().expect("repository");
    git(repo.path(), &["init"]);
    let hooks_dir = repo.path().join(".githooks");
    std::fs::create_dir(&hooks_dir).expect("hooks directory");
    std::fs::write(
        hooks_dir.join("pre-push.hooks"),
        "! [delete] head -c 1048576 /dev/zero; cat >/dev/null\n",
    )
    .expect("pre-push commands");

    let deletion = "(delete) 0000000000000000000000000000000000000000 \
                    refs/heads/topic 1111111111111111111111111111111111111111\n";
    let input = deletion.repeat(20_000);
    let binary = Command::cargo_bin("git-std")
        .expect("git-std binary")
        .get_program()
        .to_owned();
    let mut child = std::process::Command::new(binary)
        .args([
            "hook", "run", "pre-push", "--format", "json", "--", "origin", "remote",
        ])
        .current_dir(repo.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("hook command");
    let mut stdin = child.stdin.take().expect("hook stdin");
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));

    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().expect("hook status") {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill deadlocked hook");
            child.wait().expect("reap deadlocked hook");
            panic!("captured hook output deadlocked with replayed stdin");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    writer
        .join()
        .expect("stdin writer")
        .expect("write hook stdin");
    let mut stdout = Vec::new();
    child
        .stdout
        .take()
        .expect("hook stdout")
        .read_to_end(&mut stdout)
        .expect("read hook stdout");

    assert!(status.success());
    let result: serde_json::Value = serde_json::from_slice(&stdout).expect("hook JSON");
    assert_eq!(result["commands"][0]["exit_code"], 0);
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

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .expect("shim metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("executable shim");
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}
