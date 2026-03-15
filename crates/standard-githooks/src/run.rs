/// The execution mode for a hook.
///
/// Determines how commands without an explicit prefix behave when they
/// exit with a non-zero status code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookMode {
    /// Run all commands and collect results. Report a summary at the end.
    Collect,
    /// Abort on the first command that fails (non-advisory).
    FailFast,
}

/// Return the default execution mode for a given hook name.
///
/// Per the spec:
/// - `pre-commit` defaults to `Collect` (show all issues at once).
/// - `pre-push` defaults to `FailFast` (don't push broken code).
/// - `commit-msg` defaults to `FailFast` (reject bad messages immediately).
/// - All other hooks default to `Collect` (safe default).
///
/// # Example
///
/// ```
/// use standard_githooks::{HookMode, default_mode};
///
/// assert_eq!(default_mode("pre-commit"), HookMode::Collect);
/// assert_eq!(default_mode("pre-push"), HookMode::FailFast);
/// assert_eq!(default_mode("commit-msg"), HookMode::FailFast);
/// assert_eq!(default_mode("post-merge"), HookMode::Collect);
/// ```
pub fn default_mode(hook_name: &str) -> HookMode {
    match hook_name {
        "pre-push" | "commit-msg" => HookMode::FailFast,
        _ => HookMode::Collect,
    }
}

/// Replace `{msg}` tokens in a command string with the given file path.
///
/// This enables hooks like `commit-msg` to pass the commit message file
/// path into commands. If the command does not contain `{msg}`, the
/// original string is returned unchanged.
///
/// # Example
///
/// ```
/// use standard_githooks::substitute_msg;
///
/// let result = substitute_msg("git std check --file {msg}", ".git/COMMIT_EDITMSG");
/// assert_eq!(result, "git std check --file .git/COMMIT_EDITMSG");
///
/// let unchanged = substitute_msg("cargo test", ".git/COMMIT_EDITMSG");
/// assert_eq!(unchanged, "cargo test");
/// ```
pub fn substitute_msg(command: &str, msg_path: &str) -> String {
    command.replace("{msg}", msg_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pre_commit_defaults_to_collect() {
        assert_eq!(default_mode("pre-commit"), HookMode::Collect);
    }

    #[test]
    fn pre_push_defaults_to_fail_fast() {
        assert_eq!(default_mode("pre-push"), HookMode::FailFast);
    }

    #[test]
    fn commit_msg_defaults_to_fail_fast() {
        assert_eq!(default_mode("commit-msg"), HookMode::FailFast);
    }

    #[test]
    fn unknown_hook_defaults_to_collect() {
        assert_eq!(default_mode("post-merge"), HookMode::Collect);
        assert_eq!(default_mode("pre-rebase"), HookMode::Collect);
        assert_eq!(default_mode("post-checkout"), HookMode::Collect);
    }

    #[test]
    fn substitute_msg_replaces_token() {
        let result = substitute_msg("git std check --file {msg}", ".git/COMMIT_EDITMSG");
        assert_eq!(result, "git std check --file .git/COMMIT_EDITMSG");
    }

    #[test]
    fn substitute_msg_no_token() {
        let result = substitute_msg("cargo test --workspace", ".git/COMMIT_EDITMSG");
        assert_eq!(result, "cargo test --workspace");
    }

    #[test]
    fn substitute_msg_multiple_tokens() {
        let result = substitute_msg("echo {msg} && cat {msg}", "/tmp/msg");
        assert_eq!(result, "echo /tmp/msg && cat /tmp/msg");
    }

    #[test]
    fn substitute_msg_empty_path() {
        let result = substitute_msg("check --file {msg}", "");
        assert_eq!(result, "check --file ");
    }
}
