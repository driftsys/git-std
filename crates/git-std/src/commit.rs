use crate::config::{ProjectConfig, ScopesConfig};
use inquire::{
    Select, Text,
    validator::{ErrorMessage, Validation},
};
use standard_commit::ConventionalCommit;

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

/// Run the interactive commit flow: prompt, format, validate, commit.
pub fn run_interactive(config: &ProjectConfig) -> i32 {
    let answers = match prompt(config) {
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

    match create_commit(&message) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

fn prompt(config: &ProjectConfig) -> Result<PromptAnswers, Box<dyn std::error::Error>> {
    let commit_type = prompt_type(&config.types)?;
    let scope = prompt_scope(config)?;
    let description = prompt_description()?;
    let body = prompt_body()?;
    let breaking = prompt_breaking()?;
    let refs = prompt_refs()?;

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

fn create_commit(message: &str) -> Result<(), Box<dyn std::error::Error>> {
    let repo = git2::Repository::discover(".")?;
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

    #[test]
    fn create_commit_writes_to_repo() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();

        // Configure a signature for the test repo.
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();

        // Create and stage a file.
        let file_path = dir.path().join("hello.txt");
        std::fs::write(&file_path, "hello").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("hello.txt")).unwrap();
        index.write().unwrap();

        // Commit from inside the temp dir.
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let result = create_commit("feat: initial commit");
        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());

        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.message().unwrap(), "feat: initial commit");
    }
}
