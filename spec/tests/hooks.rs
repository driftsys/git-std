#[path = "../support/mod.rs"]
mod support;

use serde_json::Value;
use snapbox::cmd::Command;
use snapbox::file;
use support::TestRepo;

const MIXED_PUSH_INPUT: &str = "(delete) 0000000000000000000000000000000000000000 \
                                refs/heads/old 1111111111111111111111111111111111111111\n\
                                refs/heads/main 2222222222222222222222222222222222222222 \
                                refs/heads/main 1111111111111111111111111111111111111111\n";

/// `hook list` displays configured hooks with their mode and commands.
#[test]
fn hooks_list_shows_configured_hooks() {
    let repo = TestRepo::new().with_hooks_file(
        "pre-commit",
        "dprint check\ncargo clippy --workspace -- -D warnings *.rs\n",
    );

    Command::new(TestRepo::bin_path())
        .args(["hook", "list"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/list_shows_configured_hooks.stderr.expected"
        ]);
}

/// `hook list` shows fail-fast mode for pre-push hooks.
#[test]
fn hooks_list_fail_fast_mode() {
    let repo = TestRepo::new().with_hooks_file(
        "pre-push",
        "!cargo build --workspace\n! [delete] check-ref-policy\n",
    );

    Command::new(TestRepo::bin_path())
        .args(["hook", "list"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/list_fail_fast_mode.stderr.expected"
        ]);
}

/// `hook list --format json` exposes the delete marker without changing the command.
#[test]
fn hooks_list_json_marks_delete_commands() {
    let repo = TestRepo::new().with_hooks_file("pre-push", "! [delete] check-ref-policy\n");

    let output = Command::new(TestRepo::bin_path())
        .args(["hook", "list", "--format", "json"])
        .current_dir(repo.path())
        .output()
        .expect("hook list");
    assert!(output.status.success());
    let hooks: Value = serde_json::from_slice(&output.stdout).expect("hook list JSON");
    let command = &hooks
        .as_array()
        .expect("hook array")
        .iter()
        .find(|hook| hook["name"] == "pre-push")
        .expect("pre-push hook")["commands"][0];

    assert_eq!(command["command"], "check-ref-policy");
    assert_eq!(command["delete"], true);
}

/// `hook list` with no hooks configured prints a message to stderr.
#[test]
fn hooks_list_no_hooks() {
    let repo = TestRepo::new();

    Command::new(TestRepo::bin_path())
        .args(["hook", "list"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file!["../snapshots/hooks/list_no_hooks.stderr.expected"]);
}

/// `git std init` creates shim scripts for each `.hooks` file.
#[test]
fn init_creates_shims() {
    let repo = TestRepo::new().with_hooks_file("pre-commit", "dprint check\ncargo test\n");

    Command::new(TestRepo::bin_path())
        .args(["init"])
        .env("GIT_STD_HOOKS_ENABLE", "pre-commit")
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/init_creates_shims.stderr.expected"
        ]);

    // Verify shim exists.
    let shim_path = repo.path().join(".githooks/pre-commit");
    assert!(shim_path.exists(), "shim should exist");
}

/// `hook run` shows pass, fail, and advisory results in collect mode.
#[test]
fn hooks_run_pass_fail_advisory() {
    let repo = TestRepo::new().with_hooks_file("pre-commit", "true\n?false\n!false\n");

    Command::new(TestRepo::bin_path())
        .args(["hook", "run", "pre-commit"])
        .current_dir(repo.path())
        .assert()
        .code(1)
        .stderr_eq(file![
            "../snapshots/hooks/run_pass_fail_advisory.stderr.expected"
        ]);
}

/// `hook run` skips execution when GIT_STD_SKIP_HOOKS=1 is set.
#[test]
fn hooks_run_skip_via_env_var() {
    let repo = TestRepo::new().with_hooks_file("pre-commit", "false\n");

    Command::new(TestRepo::bin_path())
        .args(["hook", "run", "pre-commit"])
        .env("GIT_STD_SKIP_HOOKS", "1")
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/run_skip_via_env_var.stderr.expected"
        ]);
}

/// `hook run` skips execution when GIT_STD_SKIP_HOOKS=true is set.
#[test]
fn hooks_run_skip_via_env_var_true() {
    let repo = TestRepo::new().with_hooks_file("pre-commit", "false\n");

    Command::new(TestRepo::bin_path())
        .args(["hook", "run", "pre-commit"])
        .env("GIT_STD_SKIP_HOOKS", "true")
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/run_skip_via_env_var.stderr.expected"
        ]);
}

/// `hook run` displays glob patterns and skips commands that don't match.
#[test]
fn hooks_run_glob_filtering() {
    let mut repo = TestRepo::new().with_hooks_file("pre-push", "true *.txt\ntrue *.py\n");
    // add_commit creates file-1.txt, so *.txt will match and *.py won't.
    repo.add_commit("chore: init");

    Command::new(TestRepo::bin_path())
        .args(["hook", "run", "pre-push"])
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/run_glob_filtering.stderr.expected"
        ]);
}

/// A deletion-only push skips ordinary checks and runs commands marked `[delete]`.
#[test]
fn hooks_run_deletion_only_commands() {
    let repo = TestRepo::new().with_hooks_file(
        "pre-push",
        "! touch ordinary-ran\n! [delete] tee delete-input >/dev/null\n",
    );
    let input = "(delete) 0000000000000000000000000000000000000000 \
                 refs/heads/topic 1111111111111111111111111111111111111111\n\
                 (delete) 0000000000000000000000000000000000000000 \
                 refs/heads/other 2222222222222222222222222222222222222222\n";

    Command::new(TestRepo::bin_path())
        .args([
            "hook",
            "run",
            "pre-push",
            "--",
            "origin",
            "https://example.com/repo.git",
        ])
        .stdin(input)
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/run_deletion_only_commands.stderr.expected"
        ]);

    assert!(
        !repo.path().join("ordinary-ran").exists(),
        "ordinary commands should be skipped for deletion-only pushes"
    );
    assert_eq!(
        std::fs::read(repo.path().join("delete-input")).expect("delete stdin"),
        input.as_bytes()
    );
}

/// JSON glob skips report the command without its delete marker.
#[test]
fn hooks_run_json_strips_delete_marker_from_glob_skips() {
    let repo =
        TestRepo::new().with_hooks_file("pre-push", "! [delete] check-ref-policy *.missing\n");

    let output = Command::new(TestRepo::bin_path())
        .args(["hook", "run", "pre-push", "--format", "json"])
        .stdin(MIXED_PUSH_INPUT)
        .current_dir(repo.path())
        .output()
        .expect("hook run");
    assert!(output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).expect("hook run JSON");

    assert_eq!(result["commands"][0]["command"], "check-ref-policy");
    assert_eq!(result["commands"][0]["skipped"], true);
}

/// A mixed push runs ordinary and `[delete]` commands with the full ref input.
#[test]
fn hooks_run_all_commands_for_a_mixed_push() {
    let repo = TestRepo::new().with_hooks_file(
        "pre-push",
        "! tee ordinary-input >/dev/null\n! [delete] tee delete-input >/dev/null\n",
    );

    Command::new(TestRepo::bin_path())
        .args([
            "hook",
            "run",
            "pre-push",
            "--",
            "origin",
            "https://example.com/repo.git",
        ])
        .stdin(MIXED_PUSH_INPUT)
        .current_dir(repo.path())
        .assert()
        .success()
        .stderr_eq(file![
            "../snapshots/hooks/run_mixed_push_commands.stderr.expected"
        ]);

    assert_eq!(
        std::fs::read(repo.path().join("ordinary-input")).expect("ordinary stdin"),
        MIXED_PUSH_INPUT.as_bytes()
    );
    assert_eq!(
        std::fs::read(repo.path().join("delete-input")).expect("delete stdin"),
        MIXED_PUSH_INPUT.as_bytes()
    );
}

/// Text hook execution forwards Git's pre-push arguments and replays stdin.
#[test]
fn hooks_run_pre_push_forwards_arguments_in_text_mode() {
    let repo = TestRepo::new().with_hooks_file(
        "pre-push",
        "! printf '%s\\n%s\\n%s\\n' \"$1\" \"$2\" \"$#\" > first-args; cat > first-input\n\
         ! printf '%s\\n%s\\n%s\\n' \"$1\" \"$2\" \"$#\" > second-args; cat > second-input\n",
    );

    Command::new(TestRepo::bin_path())
        .args([
            "hook",
            "run",
            "pre-push",
            "--",
            "origin",
            "https://example.com/repo.git",
        ])
        .stdin(MIXED_PUSH_INPUT)
        .current_dir(repo.path())
        .assert()
        .success();

    let expected_args = "origin\nhttps://example.com/repo.git\n2\n";
    for prefix in ["first", "second"] {
        assert_eq!(
            std::fs::read_to_string(repo.path().join(format!("{prefix}-args")))
                .expect("pre-push arguments"),
            expected_args
        );
        assert_eq!(
            std::fs::read(repo.path().join(format!("{prefix}-input"))).expect("pre-push stdin"),
            MIXED_PUSH_INPUT.as_bytes()
        );
    }
}

/// JSON hook execution forwards Git's pre-push arguments and replays stdin.
#[test]
fn hooks_run_pre_push_forwards_arguments_in_json_mode() {
    let repo = TestRepo::new().with_hooks_file(
        "pre-push",
        "! printf '%s\\n%s\\n%s\\n' \"$1\" \"$2\" \"$#\" > first-args; cat > first-input\n\
         ! printf '%s\\n%s\\n%s\\n' \"$1\" \"$2\" \"$#\" > second-args; cat > second-input\n",
    );

    Command::new(TestRepo::bin_path())
        .args([
            "hook",
            "run",
            "pre-push",
            "--format",
            "json",
            "--",
            "origin",
            "https://example.com/repo.git",
        ])
        .stdin(MIXED_PUSH_INPUT)
        .current_dir(repo.path())
        .assert()
        .success();

    let expected_args = "origin\nhttps://example.com/repo.git\n2\n";
    for prefix in ["first", "second"] {
        assert_eq!(
            std::fs::read_to_string(repo.path().join(format!("{prefix}-args")))
                .expect("pre-push arguments"),
            expected_args
        );
        assert_eq!(
            std::fs::read(repo.path().join(format!("{prefix}-input"))).expect("pre-push stdin"),
            MIXED_PUSH_INPUT.as_bytes()
        );
    }
}

/// Manual pre-push execution replays piped input even without Git's remote arguments.
#[test]
fn hooks_run_replays_stdin_without_git_arguments() {
    let repo = TestRepo::new().with_hooks_file(
        "pre-push",
        "! printf '%s\\n<%s>\\n' \"$#\" \"$*\" > first-args; tee first-input >/dev/null\n\
         ! [delete] printf '%s\\n<%s>\\n' \"$#\" \"$*\" > second-args; tee second-input >/dev/null\n",
    );

    Command::new(TestRepo::bin_path())
        .args(["hook", "run", "pre-push"])
        .stdin(MIXED_PUSH_INPUT)
        .current_dir(repo.path())
        .assert()
        .success();

    assert_eq!(
        std::fs::read(repo.path().join("first-input")).expect("first stdin"),
        MIXED_PUSH_INPUT.as_bytes()
    );
    assert_eq!(
        std::fs::read(repo.path().join("second-input")).expect("second stdin"),
        MIXED_PUSH_INPUT.as_bytes()
    );
    for prefix in ["first", "second"] {
        assert_eq!(
            std::fs::read_to_string(repo.path().join(format!("{prefix}-args")))
                .expect("manual pre-push arguments"),
            "0\n<>\n"
        );
    }
}

/// JSON hook execution replays the complete pre-push input.
#[test]
fn hooks_run_json_replays_pre_push_stdin() {
    let repo = TestRepo::new().with_hooks_file("pre-push", "! tee json-input >/dev/null\n");

    Command::new(TestRepo::bin_path())
        .args([
            "hook",
            "run",
            "pre-push",
            "--format",
            "json",
            "--",
            "origin",
            "https://example.com/repo.git",
        ])
        .stdin(MIXED_PUSH_INPUT)
        .current_dir(repo.path())
        .assert()
        .success();

    assert_eq!(
        std::fs::read(repo.path().join("json-input")).expect("JSON stdin"),
        MIXED_PUSH_INPUT.as_bytes()
    );
}

/// Fail-fast JSON output contains one result per configured command after deletion filtering.
#[test]
fn hooks_run_deletion_fail_fast_reports_each_command_once() {
    let repo = TestRepo::new().with_hooks_file(
        "pre-push",
        "! true\n! [delete] false\n! [delete] touch should-not-run\n",
    );
    let input = "(delete) 0000000000000000000000000000000000000000 \
                 refs/heads/topic 1111111111111111111111111111111111111111\n";

    let output = Command::new(TestRepo::bin_path())
        .args([
            "hook",
            "run",
            "pre-push",
            "--format",
            "json",
            "--",
            "origin",
            "https://example.com/repo.git",
        ])
        .stdin(input)
        .current_dir(repo.path())
        .output()
        .expect("hook run");
    assert_eq!(output.status.code(), Some(1));
    let result: Value = serde_json::from_slice(&output.stdout).expect("hook run JSON");

    assert_eq!(result["commands"].as_array().unwrap().len(), 3);
    assert_eq!(result["commands"][0]["skipped"], true);
    assert_eq!(result["commands"][1]["command"], "false");
    assert_eq!(result["commands"][1]["skipped"], false);
    assert_eq!(result["commands"][2]["command"], "touch should-not-run");
    assert_eq!(result["commands"][2]["skipped"], true);
    assert!(!repo.path().join("should-not-run").exists());
}

/// Human fail-fast output counts configured commands after deletion filtering.
#[test]
fn hooks_run_deletion_fail_fast_reports_exact_remaining_count() {
    let repo = TestRepo::new().with_hooks_file(
        "pre-push",
        "! true\n! [delete] false\n! [delete] touch should-not-run\n",
    );
    let input = "(delete) 0000000000000000000000000000000000000000 \
                 refs/heads/topic 1111111111111111111111111111111111111111\n";

    let output = Command::new(TestRepo::bin_path())
        .args(["hook", "run", "pre-push"])
        .stdin(input)
        .current_dir(repo.path())
        .output()
        .expect("hook run");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).expect("hook stderr");
    assert!(stderr.contains("1 remaining command skipped (fail-fast)"));
    assert!(!stderr.contains("2 remaining commands skipped (fail-fast)"));
    assert!(!repo.path().join("should-not-run").exists());
}

/// Empty or malformed piped input runs all pre-push commands conservatively.
#[test]
fn hooks_run_all_commands_for_empty_and_malformed_input() {
    for (case, input) in [
        ("empty", ""),
        (
            "malformed",
            "(delete) 0000000000000000000000000000000000000000 \
             refs/heads/topic 1111111111111111111111111111111111111111\n\n",
        ),
    ] {
        let repo = TestRepo::new().with_hooks_file(
            "pre-push",
            &format!("! touch ordinary-{case}\n! [delete] touch delete-{case}\n"),
        );

        Command::new(TestRepo::bin_path())
            .args([
                "hook",
                "run",
                "pre-push",
                "--",
                "origin",
                "https://example.com/repo.git",
            ])
            .stdin(input)
            .current_dir(repo.path())
            .assert()
            .success();

        assert!(repo.path().join(format!("ordinary-{case}")).exists());
        assert!(repo.path().join(format!("delete-{case}")).exists());
    }
}

/// `hook run` correctly handles staged renames with fix mode (#387).
/// The stash dance corrupts renames by splitting them, but we repair
/// them by re-staging the old name as a deletion after formatting.
#[test]
fn hooks_run_fix_mode_handles_staged_renames() {
    let mut repo = TestRepo::new().with_hooks_file("pre-commit", "~ echo 'format check'\n");
    repo.add_commit("chore: init");

    // Create and commit a file, then rename it to stage the rename.
    let original_file = "original.txt";
    std::fs::write(repo.path().join(original_file), "content").expect("failed to write file");
    std::process::Command::new("git")
        .args(["add", original_file])
        .current_dir(repo.path())
        .status()
        .expect("failed to add file");
    std::process::Command::new("git")
        .args(["commit", "-m", "chore: add file to rename"])
        .current_dir(repo.path())
        .status()
        .expect("failed to commit file");

    // Now stage a rename
    let renamed_file = "renamed.txt";
    std::process::Command::new("git")
        .args(["mv", original_file, renamed_file])
        .current_dir(repo.path())
        .status()
        .expect("failed to rename file");

    // Run pre-commit hook with a fix command (~).
    // Should succeed and repair the rename corruption from stash apply.
    let output = std::process::Command::new(TestRepo::bin_path())
        .args(["hook", "run", "pre-commit"])
        .current_dir(repo.path())
        .output()
        .expect("failed to run hook run");

    assert!(
        output.status.success(),
        "hook run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    // Verify the fix command ran successfully
    assert!(
        stderr.contains("echo 'format check'"),
        "expected fix command to run, stderr: {stderr}"
    );
    // Verify no warning about the old name being formatted
    assert!(
        !stderr.contains(&format!(
            "{original_file}: unstaged changes were also formatted"
        )),
        "should not warn about old filename: {stderr}"
    );

    // Verify the rename is properly staged for commit
    let git_status = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo.path())
        .output()
        .expect("failed to get git status");
    let status = String::from_utf8_lossy(&git_status.stdout);
    // Should show the rename, not separate delete and add
    assert!(
        status.contains("R ") && status.contains(original_file) && status.contains(renamed_file),
        "should show rename in git status, got: {status}"
    );
}
