/// Returns `true` if the message matches a known platform-generated process
/// commit pattern (merge commits, reverts, fixups, squashes, initial commit).
///
/// Detection is based on the first line of the message using simple prefix
/// checks — no regex needed. Covers GitHub, GitLab, Gerrit, and standard git
/// patterns.
pub fn is_process_commit(message: &str) -> bool {
    let first_line = message.lines().next().unwrap_or("");

    // Fast path: most process commits start with "Merge ".
    if let Some(rest) = first_line.strip_prefix("Merge ") {
        return rest.starts_with("pull request #")
            || rest.starts_with("branch '")
            || rest.starts_with("tag '")
            || rest.starts_with("remote-tracking branch")
            || rest.starts_with('"')
            || rest.starts_with("changes");
    }

    first_line.starts_with("Revert \"")
        || first_line.starts_with("fixup! ")
        || first_line.starts_with("squash! ")
        || first_line == "Initial commit"
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Positive cases ---

    #[test]
    fn github_merge_pr() {
        assert!(is_process_commit(
            "Merge pull request #42 from user/feature-branch"
        ));
    }

    #[test]
    fn github_merge_branch() {
        assert!(is_process_commit("Merge branch 'main' into feature"));
    }

    #[test]
    fn git_merge_tag() {
        assert!(is_process_commit("Merge tag 'v1.0.0'"));
    }

    #[test]
    fn git_merge_remote_tracking() {
        assert!(is_process_commit(
            "Merge remote-tracking branch 'origin/main'"
        ));
    }

    #[test]
    fn gerrit_merge_quoted() {
        assert!(is_process_commit("Merge \"Add feature X\""));
    }

    #[test]
    fn gerrit_merge_changes() {
        assert!(is_process_commit("Merge changes I1234abcd,I5678efgh"));
    }

    #[test]
    fn git_revert() {
        assert!(is_process_commit(
            "Revert \"feat(auth): add OAuth2 PKCE flow\""
        ));
    }

    #[test]
    fn git_fixup() {
        assert!(is_process_commit("fixup! feat(auth): add login"));
    }

    #[test]
    fn git_squash() {
        assert!(is_process_commit("squash! fix: handle timeout"));
    }

    #[test]
    fn initial_commit() {
        assert!(is_process_commit("Initial commit"));
    }

    #[test]
    fn multiline_merge_detected_from_first_line() {
        assert!(is_process_commit(
            "Merge pull request #99 from user/branch\n\nSome body text"
        ));
    }

    // --- Negative cases ---

    #[test]
    fn conventional_commit_with_merge_scope() {
        assert!(!is_process_commit("fix(merge): resolve conflict"));
    }

    #[test]
    fn conventional_commit_mentioning_merge() {
        assert!(!is_process_commit("feat: merge auth modules"));
    }

    #[test]
    fn lowercase_merge_not_matched() {
        assert!(!is_process_commit("merge branch 'main'"));
    }

    #[test]
    fn regular_conventional_commit() {
        assert!(!is_process_commit("feat(auth): add OAuth2 PKCE flow"));
    }

    #[test]
    fn empty_message() {
        assert!(!is_process_commit(""));
    }

    #[test]
    fn revert_conventional_commit_not_matched() {
        assert!(!is_process_commit("revert: undo auth changes"));
    }

    #[test]
    fn initial_commit_with_extra_text() {
        assert!(!is_process_commit("Initial commit with files"));
    }
}
