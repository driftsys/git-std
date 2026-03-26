/// Returns `true` if the commit message looks like an automatically generated
/// process commit — merge, revert, fixup, squash, or initial commit.
///
/// Detection is based on the first line (subject) of the message only, using
/// well-known prefix patterns from GitHub, GitLab, Gerrit, and git itself.
/// No git parent-count check is performed; this works across all input modes.
///
/// # Process commit patterns
///
/// | Pattern                          | Source             |
/// | -------------------------------- | ------------------ |
/// | `Merge pull request #N …`        | GitHub             |
/// | `Merge branch '…'`               | GitHub/GitLab/git  |
/// | `Merge tag '…'`                  | git                |
/// | `Merge remote-tracking branch …` | git                |
/// | `Merge "…"`                      | Gerrit             |
/// | `Merge changes …`                | Gerrit             |
/// | `Revert "…"`                     | `git revert`       |
/// | `fixup! …`                       | `git commit --fixup`   |
/// | `squash! …`                      | `git commit --squash`  |
/// | `Initial commit`                 | GitHub / convention |
///
/// # Examples
///
/// ```
/// use standard_commit::is_process_commit;
///
/// assert!(is_process_commit("Merge pull request #42 from owner/branch"));
/// assert!(is_process_commit("Revert \"feat: add login\""));
/// assert!(is_process_commit("fixup! fix: handle timeout"));
/// assert!(!is_process_commit("feat: add login"));
/// assert!(!is_process_commit("chore: update deps"));
/// ```
pub fn is_process_commit(message: &str) -> bool {
    let subject = message.lines().next().unwrap_or("").trim();
    is_process_subject(subject)
}

fn is_process_subject(subject: &str) -> bool {
    // Merge patterns (case-sensitive — git always capitalises "Merge")
    if subject.starts_with("Merge pull request #") {
        return true;
    }
    if subject.starts_with("Merge branch '") {
        return true;
    }
    if subject.starts_with("Merge tag '") {
        return true;
    }
    if subject.starts_with("Merge remote-tracking branch ") {
        return true;
    }
    // Gerrit: `Merge "..."` and `Merge changes ...`
    if subject.starts_with("Merge \"") {
        return true;
    }
    if subject.starts_with("Merge changes ") {
        return true;
    }
    // git revert: `Revert "..."`
    if subject.starts_with("Revert \"") {
        return true;
    }
    // git commit --fixup / --squash
    if subject.starts_with("fixup! ") {
        return true;
    }
    if subject.starts_with("squash! ") {
        return true;
    }
    // Initial commit (GitHub new-repo default and common convention)
    if subject == "Initial commit" {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Merge patterns ---

    #[test]
    fn github_merge_pull_request() {
        assert!(is_process_commit(
            "Merge pull request #42 from owner/feature-branch"
        ));
    }

    #[test]
    fn merge_branch() {
        assert!(is_process_commit("Merge branch 'main' into feature"));
        assert!(is_process_commit("Merge branch 'develop'"));
    }

    #[test]
    fn merge_tag() {
        assert!(is_process_commit("Merge tag 'v1.2.3'"));
    }

    #[test]
    fn merge_remote_tracking_branch() {
        assert!(is_process_commit(
            "Merge remote-tracking branch 'origin/main'"
        ));
    }

    #[test]
    fn gerrit_merge_quoted() {
        assert!(is_process_commit("Merge \"feat: add login\""));
    }

    #[test]
    fn gerrit_merge_changes() {
        assert!(is_process_commit("Merge changes Iabc1234,Idef5678"));
    }

    // --- Revert ---

    #[test]
    fn git_revert() {
        assert!(is_process_commit("Revert \"feat: add login\""));
        assert!(is_process_commit("Revert \"fix(auth): handle timeout\""));
    }

    // --- Fixup / squash ---

    #[test]
    fn fixup_commit() {
        assert!(is_process_commit("fixup! fix: handle timeout"));
        assert!(is_process_commit("fixup! feat(auth): add OAuth2 PKCE"));
    }

    #[test]
    fn squash_commit() {
        assert!(is_process_commit("squash! feat: add login"));
    }

    // --- Initial commit ---

    #[test]
    fn initial_commit() {
        assert!(is_process_commit("Initial commit"));
    }

    // --- Conventional commits are NOT process commits ---

    #[test]
    fn conventional_feat_is_not_process() {
        assert!(!is_process_commit("feat: add login"));
    }

    #[test]
    fn conventional_fix_is_not_process() {
        assert!(!is_process_commit("fix(auth): handle expired tokens"));
    }

    #[test]
    fn revert_type_conventional_is_not_process() {
        // `revert(scope): ...` follows CC format and is NOT a process commit
        assert!(!is_process_commit("revert(auth): undo broken migration"));
    }

    #[test]
    fn bare_merge_word_is_not_process() {
        // Loose "Merge" without a recognised pattern should not match
        assert!(!is_process_commit("Merge: resolve conflict"));
    }

    #[test]
    fn lowercase_initial_commit_is_not_process() {
        assert!(!is_process_commit("initial commit"));
    }

    #[test]
    fn multiline_message_uses_subject_only() {
        let msg = "feat: add login\n\nMerge pull request #1 from owner/branch";
        assert!(!is_process_commit(msg));
    }
}
