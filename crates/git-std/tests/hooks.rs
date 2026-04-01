use std::path::Path;

use assert_cmd::Command;

/// Helper: run a git command and return stdout.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Helper: get HEAD commit message.
fn head_message(dir: &Path) -> String {
    git(dir, &["log", "-1", "--format=%B"]).trim().to_string()
}

/// Helper: initialise a git repo for hooks tests.
fn init_hooks_repo(dir: &Path) {
    git(dir, &["init"]);
    git(dir, &["config", "user.name", "Test"]);
    git(dir, &["config", "user.email", "test@test.com"]);
}

// --- Hooks run integration tests (#32–#35) ---

/// #32 — Argument passthrough: `{msg}` token gets substituted with the path
/// passed after `--`.
#[test]
fn hooks_run_arg_passthrough_substitutes_msg_token() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // Use `echo {msg}` so the substituted path appears in output.
    std::fs::write(hooks_dir.join("commit-msg.hooks"), "! echo {msg}\n").unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "--color",
            "never",
            "hook",
            "run",
            "commit-msg",
            "--",
            "/tmp/test-msg",
        ])
        .current_dir(dir.path())
        .assert()
        .success();

    // The substituted path should appear in stdout (from echo) or stderr
    // (from the hook runner summary line).
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("/tmp/test-msg"),
        "output should contain the substituted path, got:\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// #33 — Pre-commit workflow: mix of passing, failing, and advisory commands.
#[test]
fn hooks_run_pre_commit_workflow() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // The advisory command (`? false`) fails but is advisory, so it gets ⚠.
    std::fs::write(
        hooks_dir.join("pre-commit.hooks"),
        "echo \"lint ok\"\nfalse\n? false\n",
    )
    .unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(dir.path())
        .assert()
        .code(1);

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    // Check mark for the passing `echo "lint ok"` command.
    assert!(
        stderr.contains('\u{2713}'),
        "should contain check mark for passing command, got: {stderr}"
    );
    // Cross mark for the failing `false` command.
    assert!(
        stderr.contains('\u{2717}'),
        "should contain cross mark for failing command, got: {stderr}"
    );
    // Warning mark for the advisory `echo "advisory warning"` command.
    assert!(
        stderr.contains('\u{26a0}'),
        "should contain warning mark for advisory command, got: {stderr}"
    );
}

/// #34 — Commit-msg workflow: bad message fails validation.
#[test]
fn hooks_run_commit_msg_bad_message_fails() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    // Use the cargo-built binary path so the hook command works in CI
    // where `git std` isn't on PATH.
    let bin = Command::cargo_bin("git-std")
        .unwrap()
        .get_program()
        .to_owned();
    let bin_str = bin.to_string_lossy();

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(
        hooks_dir.join("commit-msg.hooks"),
        format!("! {bin_str} lint --file {{msg}}\n"),
    )
    .unwrap();

    // Write a bad commit message to a temp file.
    let msg_file = dir.path().join("COMMIT_MSG");
    std::fs::write(&msg_file, "bad message\n").unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "--color",
            "never",
            "hook",
            "run",
            "commit-msg",
            "--",
            msg_file.to_str().unwrap(),
        ])
        .current_dir(dir.path())
        .assert()
        .code(1);
}

/// #34 — Commit-msg workflow: good message passes validation.
#[test]
fn hooks_run_commit_msg_good_message_passes() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    // Use the cargo-built binary path so the hook command works in CI.
    let bin = Command::cargo_bin("git-std")
        .unwrap()
        .get_program()
        .to_owned();
    let bin_str = bin.to_string_lossy();

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(
        hooks_dir.join("commit-msg.hooks"),
        format!("! {bin_str} lint --file {{msg}}\n"),
    )
    .unwrap();

    // Write a valid conventional commit message to a temp file.
    let msg_file = dir.path().join("COMMIT_MSG");
    std::fs::write(&msg_file, "feat: valid commit\n").unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args([
            "--color",
            "never",
            "hook",
            "run",
            "commit-msg",
            "--",
            msg_file.to_str().unwrap(),
        ])
        .current_dir(dir.path())
        .assert()
        .success();
}

/// #35 — Full install cycle: install hooks, then commit through git which
/// triggers the shims.
#[test]
fn hooks_full_install_cycle() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["config", "user.email", "test@test.com"]);

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(
        hooks_dir.join("pre-commit.hooks"),
        "echo \"pre-commit ok\"\n",
    )
    .unwrap();
    // Use the cargo-built binary path for the commit-msg hook so it works
    // in CI where `git std` isn't on PATH.
    let bin = Command::cargo_bin("git-std")
        .unwrap()
        .get_program()
        .to_owned();
    let bin_str = bin.to_string_lossy();
    std::fs::write(
        hooks_dir.join("commit-msg.hooks"),
        format!("! {bin_str} lint --file {{msg}}\n"),
    )
    .unwrap();

    // Run `git std init`.
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["init"])
        .env("GIT_STD_HOOKS_ENABLE", "pre-commit,commit-msg")
        .current_dir(dir.path())
        .assert()
        .success();

    // Verify core.hooksPath is set.
    let hooks_path = git(dir.path(), &["config", "core.hooksPath"]);
    assert_eq!(hooks_path, ".githooks");

    // Verify shims exist and are executable.
    let pre_commit_shim = hooks_dir.join("pre-commit");
    let commit_msg_shim = hooks_dir.join("commit-msg");
    assert!(pre_commit_shim.exists(), "pre-commit shim should exist");
    assert!(commit_msg_shim.exists(), "commit-msg shim should exist");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::metadata(&pre_commit_shim).unwrap().permissions();
        assert!(
            perms.mode() & 0o111 != 0,
            "pre-commit shim should be executable"
        );
        let perms = std::fs::metadata(&commit_msg_shim).unwrap().permissions();
        assert!(
            perms.mode() & 0o111 != 0,
            "commit-msg shim should be executable"
        );
    }

    // The shims call `git std hook run ...` which invokes `git-std` as a
    // git subcommand. For this to work, the `git-std` binary must be on
    // PATH. Locate the cargo-built binary and prepend its directory.
    let bin_path = Command::cargo_bin("git-std")
        .unwrap()
        .get_program()
        .to_owned();
    let bin_dir = Path::new(&bin_path).parent().unwrap();
    let path_env = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    // Stage a file and commit with a valid conventional message.
    // The hooks fire (pre-commit + commit-msg) via the installed shims.
    std::fs::write(dir.path().join("hello.txt"), "hello\n").unwrap();

    let status = std::process::Command::new("git")
        .args(["add", "hello.txt"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(status.success(), "git add should succeed");

    let output = std::process::Command::new("git")
        .args(["commit", "-m", "feat: add hello"])
        .current_dir(dir.path())
        .env("PATH", &path_env)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git commit with valid message should succeed when hooks are installed.\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    // Verify the commit was created.
    let msg = head_message(dir.path());
    assert!(
        msg.starts_with("feat: add hello"),
        "commit message should start with 'feat: add hello', got: {msg:?}",
    );
}

// --- Fail-fast mode integration test (#114) ---

/// #114 — Fail-fast mode stops on first failure and skips remaining commands.
///
/// Uses `pre-push` which defaults to fail-fast mode. The first command
/// succeeds, the second fails, and the third should be skipped.
#[test]
fn hooks_run_fail_fast_skips_remaining_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // pre-push defaults to fail-fast mode:
    //   1. `true`  — succeeds
    //   2. `false` — fails  (should trigger abort)
    //   3. `echo should-not-run` — should be skipped
    std::fs::write(
        hooks_dir.join("pre-push.hooks"),
        "true\nfalse\necho should-not-run\n",
    )
    .unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-push"])
        .current_dir(dir.path())
        .assert()
        .code(1);

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let combined = format!("{stdout}{stderr}");

    // The first command should pass.
    assert!(
        combined.contains('\u{2713}'),
        "should contain check mark for passing command, got: {combined}"
    );
    // The second command should fail.
    assert!(
        combined.contains('\u{2717}'),
        "should contain cross mark for failing command, got: {combined}"
    );
    // The runner should report that remaining commands were skipped.
    assert!(
        combined.contains("skipped (fail-fast)"),
        "should report skipped commands, got: {combined}"
    );
    // The skipped command should NOT have run.
    assert!(
        !combined.contains("should-not-run"),
        "skipped command output should not appear, got: {combined}"
    );
}

/// #114 — Fail-fast with explicit `!` prefix on a collect-mode hook.
///
/// Uses `pre-commit` (collect mode by default) but the failing command
/// has a `!` prefix, forcing fail-fast for that command.
#[test]
fn hooks_run_fail_fast_prefix_overrides_collect_mode() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // pre-commit defaults to collect mode, but `!false` forces fail-fast
    // for that specific command:
    //   1. `true`  — succeeds
    //   2. `!false` — fails with fail-fast prefix (should abort)
    //   3. `echo should-not-run` — should be skipped
    std::fs::write(
        hooks_dir.join("pre-commit.hooks"),
        "true\n!false\necho should-not-run\n",
    )
    .unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(dir.path())
        .assert()
        .code(1);

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let combined = format!("{stdout}{stderr}");

    // Should report skipped commands due to fail-fast.
    assert!(
        combined.contains("skipped (fail-fast)"),
        "should report skipped commands when ! prefix triggers fail-fast, got: {combined}"
    );
    // The skipped command should NOT have run.
    assert!(
        !combined.contains("should-not-run"),
        "skipped command output should not appear, got: {combined}"
    );
}

// --- Fix-mode (~) integration tests (#197) ---

/// #197 — $@ is populated with staged file paths for pre-commit.
///
/// The `~` hook command runs with staged files as positional parameters.
/// We verify $@ is non-empty by echoing "$@" and checking the file name
/// appears in output.
#[test]
fn hooks_run_fix_mode_staged_files_passed_as_positional_args() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    // Create and stage a file so $@ is non-empty.
    std::fs::write(dir.path().join("hello.txt"), "hello\n").unwrap();
    git(dir.path(), &["add", "hello.txt"]);

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // The ~ command echoes $@ — staged file names should appear in output.
    std::fs::write(
        hooks_dir.join("pre-commit.hooks"),
        "~ echo \"staged: $@\"\n",
    )
    .unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("hello.txt"),
        "$@ should contain staged file names, got:\n{combined}"
    );
}

/// #197 — Normal commands also receive $@ with staged file paths.
#[test]
fn hooks_run_staged_files_passed_to_normal_commands() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    std::fs::write(dir.path().join("world.txt"), "world\n").unwrap();
    git(dir.path(), &["add", "world.txt"]);

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // Plain command (no prefix) also gets $@ with staged files.
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "echo \"files: $@\"\n").unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("world.txt"),
        "$@ should contain staged file names for plain commands, got:\n{combined}"
    );
}

/// #197 — `~` in pre-commit: stash dance runs, staged content re-staged.
///
/// We stage a file, then a `~` formatter appends a line to it.
/// After the hook runs, the staged version should include the formatter's change.
#[test]
fn hooks_run_fix_mode_pre_commit_restages_formatted_content() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    // Write and stage the initial file.
    std::fs::write(dir.path().join("fmt.txt"), "line1\n").unwrap();
    git(dir.path(), &["add", "fmt.txt"]);

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // The formatter appends "formatted" to each staged file passed via $@.
    std::fs::write(
        hooks_dir.join("pre-commit.hooks"),
        "~ for f in \"$@\"; do echo 'formatted' >> \"$f\"; done\n",
    )
    .unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(dir.path())
        .assert()
        .success();

    // After the hook, the staged version of fmt.txt should contain "formatted".
    let staged_content = std::process::Command::new("git")
        .args(["show", ":fmt.txt"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let staged_str = String::from_utf8_lossy(&staged_content.stdout);
    assert!(
        staged_str.contains("formatted"),
        "staged content should include formatter output, got: {staged_str}"
    );
}

/// #197 — `~` in a non-pre-commit hook prints a warning and treats as `!`.
#[test]
fn hooks_run_fix_mode_non_pre_commit_warns_and_treats_as_fail_fast() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // Use commit-msg with ~ prefix on a passing command.
    std::fs::write(hooks_dir.join("commit-msg.hooks"), "~ true\n").unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "commit-msg"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("warning:"),
        "should print a warning for ~ in non-pre-commit, got:\n{stderr}"
    );
    assert!(
        stderr.contains("pre-commit"),
        "warning should mention pre-commit, got:\n{stderr}"
    );
}

/// #197 — `~` in a non-pre-commit hook: failing command causes non-zero exit.
#[test]
fn hooks_run_fix_mode_non_pre_commit_failing_command_fails() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // commit-msg with ~ prefix on a failing command — should fail.
    std::fs::write(hooks_dir.join("commit-msg.hooks"), "~ false\n").unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "commit-msg"])
        .current_dir(dir.path())
        .assert()
        .code(1);
}

/// #268 — Fix mode preserves staged deletions.
///
/// When a file is staged for deletion (`git rm`), a `~` fix command must not
/// undo the deletion. After the hook runs, the file should still be staged
/// for deletion in the index.
#[test]
fn hooks_run_fix_mode_preserves_staged_deletions() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    // Create and commit a file so we can delete it later.
    std::fs::write(dir.path().join("to-delete.txt"), "content\n").unwrap();
    std::fs::write(dir.path().join("to-keep.txt"), "keep\n").unwrap();
    git(dir.path(), &["add", "to-delete.txt", "to-keep.txt"]);
    git(dir.path(), &["commit", "-m", "initial"]);

    // Stage the file for deletion.
    git(dir.path(), &["rm", "to-delete.txt"]);

    // Also stage a modification so the fix command has something to work with.
    std::fs::write(dir.path().join("to-keep.txt"), "modified\n").unwrap();
    git(dir.path(), &["add", "to-keep.txt"]);

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // A no-op formatter that succeeds without modifying files.
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "~ true\n").unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Verify the deletion is still staged in the index.
    let status_output = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=D"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let deleted = String::from_utf8_lossy(&status_output.stdout);
    assert!(
        deleted.contains("to-delete.txt"),
        "to-delete.txt should still be staged for deletion after fix-mode hook, got:\n{deleted}"
    );

    // Verify the kept file is still staged.
    let status_output = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=M"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let modified = String::from_utf8_lossy(&status_output.stdout);
    assert!(
        modified.contains("to-keep.txt"),
        "to-keep.txt should still be staged after fix-mode hook, got:\n{modified}"
    );
}

/// #268 — Deleted files are not passed as $@ to fix commands.
///
/// Files staged for deletion should not appear in the positional parameters
/// passed to fix-mode commands, since formatters cannot operate on deleted files.
#[test]
fn hooks_run_fix_mode_excludes_deleted_files_from_args() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    // Create and commit files.
    std::fs::write(dir.path().join("deleted.txt"), "gone\n").unwrap();
    std::fs::write(dir.path().join("kept.txt"), "here\n").unwrap();
    git(dir.path(), &["add", "deleted.txt", "kept.txt"]);
    git(dir.path(), &["commit", "-m", "initial"]);

    // Stage one for deletion, modify the other.
    git(dir.path(), &["rm", "deleted.txt"]);
    std::fs::write(dir.path().join("kept.txt"), "modified\n").unwrap();
    git(dir.path(), &["add", "kept.txt"]);

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // Echo $@ so we can verify which files are passed.
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "~ echo \"args: $@\"\n").unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    let combined = format!("{stdout}{stderr}");

    // kept.txt should be in $@.
    assert!(
        combined.contains("kept.txt"),
        "kept.txt should appear in $@, got:\n{combined}"
    );
    // deleted.txt should NOT be in $@ (already excluded by ACMR filter).
    assert!(
        !combined.contains("deleted.txt"),
        "deleted.txt should not appear in $@, got:\n{combined}"
    );
}

/// #197 — No stash dance when no `~` commands are present.
///
/// A plain pre-commit hook with no `~` commands should not attempt any stash
/// operations (the hook runs normally).
#[test]
fn hooks_run_no_stash_dance_without_fix_commands() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    std::fs::write(dir.path().join("file.txt"), "content\n").unwrap();
    git(dir.path(), &["add", "file.txt"]);

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "true\n").unwrap();

    // Should succeed without any stash-related warnings.
    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        !stderr.contains("stash"),
        "no stash messages expected for hook without ~ commands, got:\n{stderr}"
    );
}

/// #268 — Fix mode preserves staged deletions on fail-fast early exit.
///
/// When a `~` fix command fails, the hook aborts early (fail-fast). Staged
/// deletions must still be preserved in the index after the early exit.
#[test]
fn hooks_run_fix_mode_preserves_staged_deletions_on_failfast() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    // Create and commit two files so we can delete one and modify the other.
    std::fs::write(dir.path().join("to-delete.txt"), "content\n").unwrap();
    std::fs::write(dir.path().join("to-keep.txt"), "keep\n").unwrap();
    git(dir.path(), &["add", "to-delete.txt", "to-keep.txt"]);
    git(dir.path(), &["commit", "-m", "initial"]);

    // Stage the file for deletion.
    git(dir.path(), &["rm", "to-delete.txt"]);

    // Stage a modification so the fix command has something to work with.
    std::fs::write(dir.path().join("to-keep.txt"), "modified\n").unwrap();
    git(dir.path(), &["add", "to-keep.txt"]);

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // A fix command that always fails — triggers fail-fast early exit.
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "~ false\n").unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(dir.path())
        .assert()
        .code(1);

    // Verify the deletion is still staged in the index after fail-fast.
    let status_output = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=D"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let deleted = String::from_utf8_lossy(&status_output.stdout);
    assert!(
        deleted.contains("to-delete.txt"),
        "to-delete.txt should still be staged for deletion after fail-fast, got:\n{deleted}"
    );
}

// --- Stash dance corner cases (#280–#282) ---

/// #280 — Renamed files survive the stash dance.
///
/// A `git mv` rename should be preserved through the fix-mode stash/unstash
/// cycle. After the hook runs, the index should still show the rename (R status)
/// with the new name staged and the old name removed.
#[test]
fn hooks_run_fix_mode_preserves_renamed_files() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    // Create and commit a file so we can rename it.
    std::fs::write(dir.path().join("old.txt"), "hello").unwrap();
    git(dir.path(), &["add", "old.txt"]);
    git(dir.path(), &["commit", "-m", "initial"]);

    // Rename the file (git mv stages the rename automatically).
    git(dir.path(), &["mv", "old.txt", "new.txt"]);

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // A no-op formatter that succeeds without modifying files.
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "~ true\n").unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Verify the rename is still staged (R status with new.txt present).
    let status_output = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-status"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let name_status = String::from_utf8_lossy(&status_output.stdout);
    assert!(
        name_status.contains("new.txt"),
        "new.txt should be in the staged files after stash dance, got:\n{name_status}"
    );

    // old.txt should NOT be in staged files (it was renamed away).
    let staged_output = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=ACMR"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let staged_names = String::from_utf8_lossy(&staged_output.stdout);
    assert!(
        !staged_names.contains("old.txt"),
        "old.txt should not be in the staged files after rename, got:\n{staged_names}"
    );
}

/// #281 — Formatter-created files are NOT staged.
///
/// When a `~` fix command creates a new file that was not originally staged,
/// the stash dance must not add it to the index. Only the originally-staged
/// files should remain staged.
#[test]
fn hooks_run_fix_mode_does_not_stage_formatter_created_files() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    // Create and stage a file.
    std::fs::write(dir.path().join("src.txt"), "source\n").unwrap();
    git(dir.path(), &["add", "src.txt"]);

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // Formatter that creates a new file "extra.txt" as a side effect.
    std::fs::write(
        hooks_dir.join("pre-commit.hooks"),
        "~ sh -c \"echo created > extra.txt\"\n",
    )
    .unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(dir.path())
        .assert()
        .success();

    // extra.txt should exist on disk (the formatter created it).
    assert!(
        dir.path().join("extra.txt").exists(),
        "extra.txt should exist on disk after formatter ran"
    );

    // extra.txt should NOT be staged.
    let staged_output = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let staged = String::from_utf8_lossy(&staged_output.stdout);
    assert!(
        !staged.contains("extra.txt"),
        "extra.txt should not be staged (formatter side effect), got:\n{staged}"
    );

    // src.txt SHOULD still be staged.
    assert!(
        staged.contains("src.txt"),
        "src.txt should still be staged after hook, got:\n{staged}"
    );
}

/// #282 — Binary files survive the stash dance.
///
/// Binary file content must be preserved byte-for-byte through the
/// stash/unstash cycle. We stage a file with raw bytes (PNG header) and
/// verify the staged content is identical after the hook runs.
#[test]
fn hooks_run_fix_mode_preserves_binary_files() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    // Create a binary file with a PNG header signature.
    let png_header: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    std::fs::write(dir.path().join("image.bin"), png_header).unwrap();
    git(dir.path(), &["add", "image.bin"]);

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // A no-op formatter that succeeds without modifying files.
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "~ true\n").unwrap();

    Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(dir.path())
        .assert()
        .success();

    // The staged content of image.bin should be byte-identical to the original.
    let show_output = std::process::Command::new("git")
        .args(["show", ":image.bin"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        show_output.status.success(),
        "git show :image.bin should succeed"
    );
    assert_eq!(
        show_output.stdout, png_header,
        "staged binary content should be byte-identical after stash dance"
    );
}

/// #277 + #278 — Fix-mode failure prints actionable hints.
///
/// When a `~` fix command fails, the output must include hint lines telling
/// the user how to skip the hook, skip all hooks, and disable the command.
#[test]
fn hooks_run_fix_mode_failure_prints_hints() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    std::fs::write(dir.path().join("file.txt"), "content\n").unwrap();
    git(dir.path(), &["add", "file.txt"]);

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "~ false\n").unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(dir.path())
        .assert()
        .code(1);

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);

    assert!(
        stderr.contains("hint: to skip this hook:"),
        "should print skip-hook hint, got:\n{stderr}"
    );
    assert!(
        stderr.contains("--no-verify"),
        "should mention --no-verify, got:\n{stderr}"
    );
    assert!(
        stderr.contains("GIT_STD_SKIP_HOOKS=1"),
        "should mention GIT_STD_SKIP_HOOKS, got:\n{stderr}"
    );
    assert!(
        stderr.contains(".githooks/pre-commit.hooks"),
        "should reference the hooks file, got:\n{stderr}"
    );
}

/// #279 — Formatter-deleted files are not re-staged as deletions.
///
/// If a fix-mode formatter deletes a file that was staged for modification,
/// `restage_files()` should skip it with a warning instead of silently
/// staging a deletion.
#[test]
fn hooks_run_fix_mode_skips_restage_of_formatter_deleted_files() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    // Create and commit a file.
    std::fs::write(dir.path().join("victim.txt"), "original\n").unwrap();
    std::fs::write(dir.path().join("survivor.txt"), "keep\n").unwrap();
    git(dir.path(), &["add", "victim.txt", "survivor.txt"]);
    git(dir.path(), &["commit", "-m", "initial"]);

    // Stage modifications.
    std::fs::write(dir.path().join("victim.txt"), "modified\n").unwrap();
    std::fs::write(dir.path().join("survivor.txt"), "also modified\n").unwrap();
    git(dir.path(), &["add", "victim.txt", "survivor.txt"]);

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // Formatter that deletes victim.txt.
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "~ rm -f victim.txt\n").unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(dir.path())
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);

    // Warning about the deleted file.
    assert!(
        stderr.contains("victim.txt") && stderr.contains("deleted by formatter"),
        "should warn about formatter-deleted file, got:\n{stderr}"
    );

    // victim.txt should NOT be staged as a deletion.
    let deletions = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=D"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let deleted = String::from_utf8_lossy(&deletions.stdout);
    assert!(
        !deleted.contains("victim.txt"),
        "victim.txt should not be staged as deletion, got:\n{deleted}"
    );

    // survivor.txt should still be staged.
    let staged = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=ACMR"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let staged_files = String::from_utf8_lossy(&staged.stdout);
    assert!(
        staged_files.contains("survivor.txt"),
        "survivor.txt should still be staged, got:\n{staged_files}"
    );
}

/// #283 — Fix mode rejects submodule entries.
///
/// When a submodule is staged and fix-mode commands exist, the hook
/// should reject execution with a clear error and hint.
#[test]
fn hooks_run_fix_mode_rejects_staged_submodules() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    // Create a bare repo to use as a submodule source.
    let sub_source = tempfile::tempdir().unwrap();
    git(sub_source.path(), &["init", "--bare"]);

    // We need a commit in the bare repo for submodule add to work.
    let sub_work = tempfile::tempdir().unwrap();
    git(sub_work.path(), &["init"]);
    git(sub_work.path(), &["config", "user.name", "Test"]);
    git(sub_work.path(), &["config", "user.email", "test@test.com"]);
    std::fs::write(sub_work.path().join("readme.txt"), "sub\n").unwrap();
    git(sub_work.path(), &["add", "readme.txt"]);
    git(sub_work.path(), &["commit", "-m", "init sub"]);
    git(
        sub_work.path(),
        &[
            "remote",
            "add",
            "origin",
            sub_source.path().to_str().unwrap(),
        ],
    );
    git(sub_work.path(), &["push", "origin", "HEAD"]);

    // Add the submodule to the main repo.
    let sub_url = sub_source.path().to_str().unwrap();
    let output = std::process::Command::new("git")
        .args(["submodule", "add", sub_url, "submod"])
        .current_dir(dir.path())
        .env("GIT_ALLOW_PROTOCOL", "file")
        .output()
        .unwrap();
    if !output.status.success() {
        // If submodule add is unsupported in this git version, skip the test.
        eprintln!(
            "skipping: git submodule add failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "~ true\n").unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(dir.path())
        .assert()
        .code(1);

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);

    // Should report that fix mode doesn't support submodules.
    assert!(
        stderr.contains("submodule"),
        "should mention submodules in error, got:\n{stderr}"
    );
}

// ── repo-root resolution (#320) ─────────────────────────────────

#[test]
fn hooks_run_from_subdirectory() {
    let dir = tempfile::tempdir().unwrap();
    init_hooks_repo(dir.path());

    let hooks_dir = dir.path().join(".githooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::write(hooks_dir.join("pre-commit.hooks"), "echo ok\n").unwrap();

    // Create an active shim for pre-commit so `hooks run` finds it.
    Command::cargo_bin("git-std")
        .unwrap()
        .args(["init"])
        .env("GIT_STD_HOOKS_ENABLE", "pre-commit")
        .current_dir(dir.path())
        .assert()
        .success();

    let subdir = dir.path().join("src").join("nested");
    std::fs::create_dir_all(&subdir).unwrap();

    let assert = Command::cargo_bin("git-std")
        .unwrap()
        .args(["--color", "never", "hook", "run", "pre-commit"])
        .current_dir(&subdir)
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains('\u{2713}'),
        "hooks run from subdirectory should succeed and show check mark, got:\n{combined}"
    );
}
