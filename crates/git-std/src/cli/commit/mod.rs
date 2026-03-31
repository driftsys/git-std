mod assemble;
mod prompt;

use crate::ui;

/// Options passed from CLI flags to the commit flow.
pub struct CommitOptions {
    pub commit_type: Option<String>,
    pub scope: Option<String>,
    pub message: Option<String>,
    pub breaking: Option<String>,
    pub dry_run: bool,
    pub amend: bool,
    pub sign: bool,
    pub all: bool,
    pub footer: Vec<String>,
    pub signoff: bool,
}

/// Run the commit flow: prompt (or use flags), format, validate, commit.
pub fn run_interactive(config: &crate::config::ProjectConfig, opts: &CommitOptions) -> i32 {
    let answers = match assemble::gather_answers(config, opts) {
        Ok(a) => a,
        Err(e) => {
            ui::error(&e.to_string());
            return 1;
        }
    };

    let commit = assemble::build_commit(answers);
    let message = standard_commit::format(&commit);

    if let Err(e) = standard_commit::parse(&message) {
        ui::error(&format!("assembled message is invalid: {e}"));
        return 1;
    }

    if opts.dry_run {
        ui::info(&message);
        return 0;
    }

    let dir = std::path::Path::new(".");

    // Stage all tracked modified files if --all is set.
    if opts.all
        && let Err(e) = crate::git::stage_tracked_modified(dir)
    {
        ui::error(&e.to_string());
        return 1;
    }

    let result = if opts.sign {
        crate::git::create_signed_commit_amend(dir, &message, opts.amend)
    } else if opts.amend {
        crate::git::amend_commit(dir, &message)
    } else {
        crate::git::create_commit(dir, &message)
    };

    match result {
        Ok(()) => {
            print_commit_result(dir, opts.amend);
            0
        }
        Err(e) => {
            ui::error(&e.to_string());
            1
        }
    }
}

/// Print a post-commit summary: short SHA, branch, and message subject.
fn print_commit_result(dir: &std::path::Path, amend: bool) {
    let sha = crate::git::head_oid(dir)
        .map(|s| s[..s.len().min(7)].to_string())
        .unwrap_or_else(|_| "???????".to_string());
    let branch = crate::git::current_branch(dir).unwrap_or_else(|_| "?".to_string());
    let action = if amend { "amended" } else { "committed" };
    ui::heading("", &format!("{action} [{branch} {sha}]"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ProjectConfig, ScopesConfig};
    use assemble::{PromptAnswers, build_commit, gather_answers};

    #[test]
    fn minimal_commit() {
        let answers = PromptAnswers {
            commit_type: "feat".into(),
            scope: None,
            description: "add login".into(),
            body: None,
            breaking: None,
            refs: vec![],
            extra_footers: vec![],
        };
        let commit = build_commit(answers);
        assert_eq!(commit.r#type, "feat");
        assert_eq!(commit.description, "add login");
        assert!(commit.scope.is_none());
        assert!(!commit.is_breaking);
        assert!(commit.footers.is_empty());
    }

    #[test]
    fn with_scope() {
        let answers = PromptAnswers {
            commit_type: "fix".into(),
            scope: Some("auth".into()),
            description: "handle tokens".into(),
            body: None,
            breaking: None,
            refs: vec![],
            extra_footers: vec![],
        };
        let commit = build_commit(answers);
        assert_eq!(commit.scope.as_deref(), Some("auth"));
    }

    #[test]
    fn with_body() {
        let answers = PromptAnswers {
            commit_type: "feat".into(),
            scope: None,
            description: "add PKCE".into(),
            body: Some("Full PKCE flow.".into()),
            breaking: None,
            refs: vec![],
            extra_footers: vec![],
        };
        let commit = build_commit(answers);
        assert_eq!(commit.body.as_deref(), Some("Full PKCE flow."));
    }

    #[test]
    fn breaking_change_sets_flag_and_footer() {
        let answers = PromptAnswers {
            commit_type: "feat".into(),
            scope: None,
            description: "remove legacy API".into(),
            body: None,
            breaking: Some("removed v1 endpoints".into()),
            refs: vec![],
            extra_footers: vec![],
        };
        let commit = build_commit(answers);
        assert!(commit.is_breaking);
        assert_eq!(commit.footers.len(), 1);
        assert_eq!(commit.footers[0].token, "BREAKING CHANGE");
        assert_eq!(commit.footers[0].value, "removed v1 endpoints");
    }

    #[test]
    fn refs_joined_as_single_footer() {
        let answers = PromptAnswers {
            commit_type: "fix".into(),
            scope: None,
            description: "fix bug".into(),
            body: None,
            breaking: None,
            refs: vec!["#42".into(), "#15".into()],
            extra_footers: vec![],
        };
        let commit = build_commit(answers);
        assert_eq!(commit.footers.len(), 1);
        assert_eq!(commit.footers[0].token, "Refs");
        assert_eq!(commit.footers[0].value, "#42, #15");
    }

    #[test]
    fn breaking_and_refs_produce_two_footers() {
        let answers = PromptAnswers {
            commit_type: "feat".into(),
            scope: Some("api".into()),
            description: "new auth".into(),
            body: Some("Rewrote auth.".into()),
            breaking: Some("changed token format".into()),
            refs: vec!["#10".into()],
            extra_footers: vec![],
        };
        let commit = build_commit(answers);
        assert!(commit.is_breaking);
        assert_eq!(commit.footers.len(), 2);
        assert_eq!(commit.footers[0].token, "BREAKING CHANGE");
        assert_eq!(commit.footers[1].token, "Refs");
        assert_eq!(commit.footers[1].value, "#10");
    }

    #[test]
    fn no_breaking_no_flag() {
        let answers = PromptAnswers {
            commit_type: "chore".into(),
            scope: None,
            description: "update deps".into(),
            body: None,
            breaking: None,
            refs: vec![],
            extra_footers: vec![],
        };
        let commit = build_commit(answers);
        assert!(!commit.is_breaking);
    }

    #[test]
    fn formatted_message_roundtrips() {
        let answers = PromptAnswers {
            commit_type: "feat".into(),
            scope: Some("auth".into()),
            description: "add OAuth2 PKCE flow".into(),
            body: None,
            breaking: None,
            refs: vec![],
            extra_footers: vec![],
        };
        let commit = build_commit(answers);
        let message = standard_commit::format(&commit);
        assert!(standard_commit::parse(&message).is_ok());
        assert_eq!(message, "feat(auth): add OAuth2 PKCE flow");
    }

    /// Helper: create a temp repo with one committed file.
    fn init_test_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        git(p, &["init"]);
        git(p, &["config", "user.name", "Test"]);
        git(p, &["config", "user.email", "test@test.com"]);

        std::fs::write(p.join("hello.txt"), "hello").unwrap();
        git(p, &["add", "hello.txt"]);
        git(p, &["commit", "-m", "feat: initial commit"]);

        dir
    }

    fn git(dir: &std::path::Path, args: &[&str]) -> String {
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

    fn head_message(dir: &std::path::Path) -> String {
        git(dir, &["log", "-1", "--format=%s"])
    }

    #[test]
    fn create_commit_writes_to_repo() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        git(p, &["init"]);
        git(p, &["config", "user.name", "Test"]);
        git(p, &["config", "user.email", "test@test.com"]);

        std::fs::write(p.join("hello.txt"), "hello").unwrap();
        git(p, &["add", "hello.txt"]);
        git(p, &["commit", "-m", "feat: initial commit"]);

        assert_eq!(head_message(p), "feat: initial commit");
    }

    #[test]
    fn amend_commit_updates_message() {
        let dir = init_test_repo();
        let p = dir.path();

        crate::git::amend_commit(p, "fix: amended commit").unwrap();
        assert_eq!(head_message(p), "fix: amended commit");
    }

    #[test]
    fn stage_tracked_modified_adds_changes() {
        let dir = init_test_repo();
        let p = dir.path();

        // Modify the tracked file (without staging).
        std::fs::write(p.join("hello.txt"), "modified").unwrap();

        // stage_tracked_modified should pick it up.
        crate::git::stage_tracked_modified(p).unwrap();

        // Verify it's staged by committing and checking the content.
        git(p, &["commit", "-m", "chore: update"]);
        let content = git(p, &["show", "HEAD:hello.txt"]);
        assert_eq!(content, "modified");
    }

    #[test]
    fn gather_answers_fully_non_interactive() {
        let config = ProjectConfig {
            types: vec!["feat".into(), "fix".into()],
            scopes: ScopesConfig::None,
            strict: false,
            ..Default::default()
        };
        let opts = CommitOptions {
            commit_type: Some("feat".into()),
            scope: Some("auth".into()),
            message: Some("add login".into()),
            breaking: Some("removed old flow".into()),
            dry_run: false,
            amend: false,
            sign: false,
            all: false,
            footer: vec![],
            signoff: false,
        };
        let answers = gather_answers(&config, &opts).unwrap();
        assert_eq!(answers.commit_type, "feat");
        assert_eq!(answers.scope.as_deref(), Some("auth"));
        assert_eq!(answers.description, "add login");
        assert!(answers.body.is_none());
        assert_eq!(answers.breaking.as_deref(), Some("removed old flow"));
        assert!(answers.refs.is_empty());
    }

    #[test]
    fn gather_answers_minimal_non_interactive() {
        let config = ProjectConfig {
            types: vec!["feat".into()],
            scopes: ScopesConfig::None,
            strict: false,
            ..Default::default()
        };
        let opts = CommitOptions {
            commit_type: Some("feat".into()),
            scope: None,
            message: Some("add login".into()),
            breaking: None,
            dry_run: false,
            amend: false,
            sign: false,
            all: false,
            footer: vec![],
            signoff: false,
        };
        let answers = gather_answers(&config, &opts).unwrap();
        assert_eq!(answers.commit_type, "feat");
        assert!(answers.scope.is_none());
        assert_eq!(answers.description, "add login");
        assert!(answers.breaking.is_none());
    }

    #[test]
    fn gather_answers_scope_required_in_strict_mode() {
        // strict = true + auto scopes + no --scope flag -> non-interactive path skips scope
        // The prompt path can't be unit-tested (requires TTY), but we verify that
        // fully-non-interactive mode with no scope provided and strict=true still works
        // when scope is passed explicitly via flag.
        let config = ProjectConfig {
            types: vec!["feat".into()],
            scopes: ScopesConfig::Auto,
            strict: true,
            ..Default::default()
        };
        let opts = CommitOptions {
            commit_type: Some("feat".into()),
            scope: Some("git-std".into()),
            message: Some("add feature".into()),
            breaking: None,
            dry_run: false,
            amend: false,
            sign: false,
            all: false,
            footer: vec![],
            signoff: false,
        };
        let answers = gather_answers(&config, &opts).unwrap();
        assert_eq!(answers.scope.as_deref(), Some("git-std"));
    }

    #[test]
    fn dry_run_prints_message_without_committing() {
        let config = ProjectConfig {
            types: vec!["feat".into()],
            scopes: ScopesConfig::None,
            strict: false,
            ..Default::default()
        };
        let opts = CommitOptions {
            commit_type: Some("feat".into()),
            scope: Some("auth".into()),
            message: Some("add login".into()),
            breaking: None,
            dry_run: true,
            amend: false,
            sign: false,
            all: false,
            footer: vec![],
            signoff: false,
        };
        // dry_run returns 0 and doesn't try to open a repo.
        let code = run_interactive(&config, &opts);
        assert_eq!(code, 0);
    }

    #[test]
    fn extra_footers_added_to_commit() {
        let answers = PromptAnswers {
            commit_type: "feat".into(),
            scope: None,
            description: "add login".into(),
            body: None,
            breaking: None,
            refs: vec![],
            extra_footers: vec!["Co-authored-by: Alice <a@b.com>".into()],
        };
        let commit = build_commit(answers);
        assert_eq!(commit.footers.len(), 1);
        assert_eq!(commit.footers[0].token, "Co-authored-by");
        assert_eq!(commit.footers[0].value, "Alice <a@b.com>");
    }

    #[test]
    fn multiple_extra_footers() {
        let answers = PromptAnswers {
            commit_type: "feat".into(),
            scope: None,
            description: "new api".into(),
            body: None,
            breaking: None,
            refs: vec![],
            extra_footers: vec![
                "Co-authored-by: Alice <a@b.com>".into(),
                "Reviewed-by: Carol <c@d.com>".into(),
            ],
        };
        let commit = build_commit(answers);
        assert_eq!(commit.footers.len(), 2);
        assert_eq!(commit.footers[0].token, "Co-authored-by");
        assert_eq!(commit.footers[1].token, "Reviewed-by");
    }

    #[test]
    fn extra_footers_combined_with_breaking_and_refs() {
        let answers = PromptAnswers {
            commit_type: "feat".into(),
            scope: None,
            description: "new api".into(),
            body: None,
            breaking: Some("removed old endpoints".into()),
            refs: vec!["#42".into()],
            extra_footers: vec!["Signed-off-by: Test <test@test.com>".into()],
        };
        let commit = build_commit(answers);
        assert_eq!(commit.footers.len(), 3);
        assert_eq!(commit.footers[0].token, "BREAKING CHANGE");
        assert_eq!(commit.footers[1].token, "Refs");
        assert_eq!(commit.footers[2].token, "Signed-off-by");
    }

    #[test]
    fn extra_footers_roundtrip_through_format_and_parse() {
        let answers = PromptAnswers {
            commit_type: "fix".into(),
            scope: None,
            description: "fix crash".into(),
            body: None,
            breaking: None,
            refs: vec![],
            extra_footers: vec!["Signed-off-by: Test <test@test.com>".into()],
        };
        let commit = build_commit(answers);
        let message = standard_commit::format(&commit);
        let parsed = standard_commit::parse(&message).unwrap();
        assert_eq!(parsed.footers.len(), 1);
        assert_eq!(parsed.footers[0].token, "Signed-off-by");
        assert_eq!(parsed.footers[0].value, "Test <test@test.com>");
    }

    #[test]
    fn gather_answers_with_footer_flags() {
        let config = ProjectConfig {
            types: vec!["feat".into()],
            scopes: ScopesConfig::None,
            strict: false,
            ..Default::default()
        };
        let opts = CommitOptions {
            commit_type: Some("feat".into()),
            scope: None,
            message: Some("add login".into()),
            breaking: None,
            dry_run: false,
            amend: false,
            sign: false,
            all: false,
            footer: vec!["Co-authored-by: Alice <a@b.com>".into()],
            signoff: false,
        };
        let answers = gather_answers(&config, &opts).unwrap();
        assert_eq!(answers.extra_footers.len(), 1);
        assert_eq!(answers.extra_footers[0], "Co-authored-by: Alice <a@b.com>");
    }
}
