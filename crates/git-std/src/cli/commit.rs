use crate::config::{ProjectConfig, ScopesConfig};
use inquire::{
    Select, Text,
    validator::{ErrorMessage, Validation},
};
use standard_commit::ConventionalCommit;
use std::path::Path;

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
}

/// Raw prompt answers before assembly into a `ConventionalCommit`.
struct PromptAnswers {
    commit_type: String,
    scope: Option<String>,
    description: String,
    body: Option<String>,
    breaking: Option<String>,
    refs: Vec<String>,
}

/// Assemble a `ConventionalCommit` from raw prompt answers.
fn build_commit(answers: PromptAnswers) -> ConventionalCommit {
    let is_breaking = answers.breaking.is_some();
    let mut footers = Vec::new();
    if let Some(desc) = answers.breaking {
        footers.push(standard_commit::Footer {
            token: "BREAKING CHANGE".into(),
            value: desc,
        });
    }
    if !answers.refs.is_empty() {
        footers.push(standard_commit::Footer {
            token: "Refs".into(),
            value: answers.refs.join(", "),
        });
    }

    ConventionalCommit {
        r#type: answers.commit_type,
        scope: answers.scope,
        description: answers.description,
        body: answers.body,
        footers,
        is_breaking,
    }
}

/// Run the commit flow: prompt (or use flags), format, validate, commit.
pub fn run_interactive(config: &ProjectConfig, opts: &CommitOptions) -> i32 {
    let answers = match gather_answers(config, opts) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };

    let commit = build_commit(answers);
    let message = standard_commit::format(&commit);

    if let Err(e) = standard_commit::parse(&message) {
        eprintln!("error: assembled message is invalid: {e}");
        return 1;
    }

    if opts.dry_run {
        println!("{message}");
        return 0;
    }

    // Stage all tracked modified files if --all is set.
    if opts.all
        && let Err(e) = stage_tracked_modified(".")
    {
        eprintln!("error: failed to stage files: {e}");
        eprintln!("  hint: run this command from inside a git repository");
        return 1;
    }

    if opts.sign {
        match create_commit_signed(&message, opts.amend) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("error: failed to create signed commit: {e}");
                eprintln!("  hint: ensure GPG is configured (git config user.signingkey)");
                1
            }
        }
    } else if opts.amend {
        match amend_commit(".", &message) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("error: failed to amend commit: {e}");
                1
            }
        }
    } else {
        match create_commit(".", &message) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("error: failed to create commit: {e}");
                eprintln!("  hint: ensure you have staged changes and user.name/user.email are set");
                1
            }
        }
    }
}

/// Gather answers from flags and/or interactive prompts.
///
/// When all required fields (`--type` and `--message`) are provided via flags,
/// prompts are skipped entirely (non-interactive mode). When some flags are
/// given, only the missing fields are prompted.
fn gather_answers(
    config: &ProjectConfig,
    opts: &CommitOptions,
) -> Result<PromptAnswers, Box<dyn std::error::Error>> {
    let fully_non_interactive = opts.commit_type.is_some() && opts.message.is_some();

    let commit_type = if let Some(t) = &opts.commit_type {
        t.clone()
    } else {
        prompt_type(&config.types)?
    };

    let scope = if opts.scope.is_some() {
        opts.scope.clone()
    } else if fully_non_interactive {
        None
    } else {
        prompt_scope(config)?
    };

    let description = if let Some(m) = &opts.message {
        m.clone()
    } else {
        prompt_description()?
    };

    let body = if fully_non_interactive {
        None
    } else {
        prompt_body()?
    };

    let breaking = if opts.breaking.is_some() {
        opts.breaking.clone()
    } else if fully_non_interactive {
        None
    } else {
        prompt_breaking()?
    };

    let refs = if fully_non_interactive {
        vec![]
    } else {
        prompt_refs()?
    };

    Ok(PromptAnswers {
        commit_type,
        scope,
        description,
        body,
        breaking,
        refs,
    })
}

fn prompt_type(types: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    let items: Vec<&str> = types.iter().map(|s| s.as_str()).collect();
    let selection = Select::new("type:", items).prompt()?;
    Ok(selection.to_string())
}

fn prompt_scope(config: &ProjectConfig) -> Result<Option<String>, Box<dyn std::error::Error>> {
    match &config.scopes {
        ScopesConfig::None => Ok(None),
        ScopesConfig::List(scopes) => {
            let items: Vec<&str> = scopes.iter().map(|s| s.as_str()).collect();
            let selection = Select::new("scope:", items).prompt()?;
            Ok(Some(selection.to_string()))
        }
        ScopesConfig::Auto => {
            // TODO: discover scopes from workspace (package names, folder names)
            let scope = Text::new("scope:").with_help_message("optional").prompt()?;
            if scope.is_empty() {
                Ok(None)
            } else {
                Ok(Some(scope))
            }
        }
    }
}

fn prompt_description() -> Result<String, Box<dyn std::error::Error>> {
    let desc = Text::new("subject:")
        .with_validator(|input: &str| {
            if input.trim().is_empty() {
                Ok(Validation::Invalid(ErrorMessage::Custom(
                    "subject may not be empty".into(),
                )))
            } else {
                Ok(Validation::Valid)
            }
        })
        .prompt()?;
    Ok(desc)
}

fn prompt_body() -> Result<Option<String>, Box<dyn std::error::Error>> {
    let mut paragraphs: Vec<String> = Vec::new();
    loop {
        let line = Text::new("body:").with_help_message("optional").prompt()?;
        if line.is_empty() {
            break;
        }
        paragraphs.push(line);
    }
    if paragraphs.is_empty() {
        Ok(None)
    } else {
        Ok(Some(paragraphs.join("\n\n")))
    }
}

fn prompt_breaking() -> Result<Option<String>, Box<dyn std::error::Error>> {
    let desc = Text::new("breaks:")
        .with_help_message("optional")
        .prompt()?;
    if desc.is_empty() {
        Ok(None)
    } else {
        Ok(Some(desc))
    }
}

fn prompt_refs() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut refs: Vec<String> = Vec::new();
    loop {
        let input = Text::new("issues:")
            .with_help_message("optional")
            .prompt()?;
        if input.is_empty() {
            break;
        }
        refs.push(input);
    }
    Ok(refs)
}

/// Stage all tracked modified files (equivalent to `git add -u`).
fn stage_tracked_modified(path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    let repo = git2::Repository::discover(path)?;
    let mut index = repo.index()?;
    index.update_all(["*"].iter(), None)?;
    index.write()?;
    Ok(())
}

/// Create a new commit using git2.
fn create_commit(path: impl AsRef<Path>, message: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repo = git2::Repository::discover(path)?;
    let sig = repo.signature()?;
    let mut index = repo.index()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let parent = match repo.head() {
        Ok(head) => Some(head.peel_to_commit()?),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
        Err(e) => return Err(e.into()),
    };

    let parents: Vec<&git2::Commit> = parent.iter().collect();
    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?;

    Ok(())
}

/// Amend the previous commit with a new message using git2.
fn amend_commit(path: impl AsRef<Path>, message: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repo = git2::Repository::discover(path)?;
    let sig = repo.signature()?;
    let mut index = repo.index()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let head = repo.head()?.peel_to_commit()?;
    head.amend(
        Some("HEAD"),
        Some(&sig),
        Some(&sig),
        None,
        Some(message),
        Some(&tree),
    )?;

    Ok(())
}

/// Create a signed commit by shelling out to `git commit`.
fn create_commit_signed(message: &str, amend: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = std::process::Command::new("git");
    cmd.args(["commit", "-S", "-m", message]);
    if amend {
        cmd.arg("--amend");
    }
    let status = cmd.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("git commit exited with status {status}").into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_commit() {
        let answers = PromptAnswers {
            commit_type: "feat".into(),
            scope: None,
            description: "add login".into(),
            body: None,
            breaking: None,
            refs: vec![],
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
        };
        let commit = build_commit(answers);
        let message = standard_commit::format(&commit);
        assert!(standard_commit::parse(&message).is_ok());
        assert_eq!(message, "feat(auth): add OAuth2 PKCE flow");
    }

    /// Helper: create a temp repo with one committed file.
    fn init_test_repo() -> (tempfile::TempDir, git2::Repository) {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();

        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();

        let file_path = dir.path().join("hello.txt");
        std::fs::write(&file_path, "hello").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("hello.txt")).unwrap();
        index.write().unwrap();

        create_commit(dir.path(), "feat: initial commit").unwrap();

        (dir, repo)
    }

    #[test]
    fn create_commit_writes_to_repo() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();

        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();

        let file_path = dir.path().join("hello.txt");
        std::fs::write(&file_path, "hello").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("hello.txt")).unwrap();
        index.write().unwrap();

        let result = create_commit(dir.path(), "feat: initial commit");
        assert!(result.is_ok());

        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.message().unwrap(), "feat: initial commit");
    }

    #[test]
    fn amend_commit_updates_message() {
        let (dir, _repo) = init_test_repo();

        amend_commit(dir.path(), "fix: amended commit").unwrap();

        let repo = git2::Repository::open(dir.path()).unwrap();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.message().unwrap(), "fix: amended commit");
    }

    #[test]
    fn stage_tracked_modified_adds_changes() {
        let (dir, _repo) = init_test_repo();

        // Modify the tracked file (without staging).
        std::fs::write(dir.path().join("hello.txt"), "modified").unwrap();

        // stage_tracked_modified should pick it up.
        stage_tracked_modified(dir.path()).unwrap();

        // Re-open repo to get a fresh index reflecting the staged changes.
        let repo = git2::Repository::open(dir.path()).unwrap();
        let index = repo.index().unwrap();
        let entry = index
            .get_path(std::path::Path::new("hello.txt"), 0)
            .unwrap();
        let blob = repo.find_blob(entry.id).unwrap();
        assert_eq!(std::str::from_utf8(blob.content()).unwrap(), "modified");
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
        };
        let answers = gather_answers(&config, &opts).unwrap();
        assert_eq!(answers.commit_type, "feat");
        assert!(answers.scope.is_none());
        assert_eq!(answers.description, "add login");
        assert!(answers.breaking.is_none());
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
        };
        // dry_run returns 0 and doesn't try to open a repo.
        let code = run_interactive(&config, &opts);
        assert_eq!(code, 0);
    }
}
