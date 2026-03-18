use crate::config::{ProjectConfig, ScopesConfig};
use anyhow::Result;
use inquire::{
    Select, Text,
    validator::{ErrorMessage, Validation},
};

/// Standard commit type descriptions, keyed by type name.
const TYPE_DESCRIPTIONS: &[(&str, &str)] = &[
    ("feat", "A new feature"),
    ("fix", "A bug fix"),
    ("docs", "Documentation only"),
    ("style", "Formatting, no code change"),
    ("refactor", "Code change, no feature or fix"),
    ("perf", "Performance improvement"),
    ("test", "Adding or fixing tests"),
    ("build", "Build system or dependencies"),
    ("ci", "CI configuration"),
    ("chore", "Other changes"),
    ("revert", "Reverts a previous commit"),
];

pub(super) fn prompt_type(types: &[String]) -> Result<String> {
    let display: Vec<String> = types
        .iter()
        .map(|t| {
            TYPE_DESCRIPTIONS
                .iter()
                .find(|(name, _)| *name == t.as_str())
                .map(|(_, desc)| format!("{t} \u{2014} {desc}"))
                .unwrap_or_else(|| t.clone())
        })
        .collect();
    let display_refs: Vec<&str> = display.iter().map(|s| s.as_str()).collect();
    let choice = Select::new("type:", display_refs).raw_prompt()?;
    Ok(types[choice.index].clone())
}

pub(super) fn prompt_scope(config: &ProjectConfig) -> Result<Option<String>> {
    match &config.scopes {
        ScopesConfig::None => Ok(None),
        ScopesConfig::List(scopes) => {
            let items: Vec<&str> = scopes.iter().map(|s| s.as_str()).collect();
            let selection = Select::new("scope:", items).prompt()?;
            Ok(Some(selection.to_string()))
        }
        ScopesConfig::Auto => {
            let cwd = std::env::current_dir().unwrap_or_default();
            let discovered = config.resolved_scopes(&cwd);
            if discovered.is_empty() {
                let mut prompt = Text::new("scope:");
                if config.strict {
                    prompt = prompt.with_validator(|input: &str| {
                        if input.trim().is_empty() {
                            Ok(Validation::Invalid(ErrorMessage::Custom(
                                "scope is required (strict mode)".into(),
                            )))
                        } else {
                            Ok(Validation::Valid)
                        }
                    });
                } else {
                    prompt = prompt.with_help_message("optional");
                }
                let scope = prompt.prompt()?;
                if scope.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(scope))
                }
            } else {
                let items: Vec<&str> = discovered.iter().map(|s| s.as_str()).collect();
                let selection = Select::new("scope:", items).prompt()?;
                Ok(Some(selection.to_string()))
            }
        }
    }
}

pub(super) fn prompt_description() -> Result<String> {
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

pub(super) fn prompt_body() -> Result<Option<String>> {
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

pub(super) fn prompt_breaking() -> Result<Option<String>> {
    let desc = Text::new("breaks:")
        .with_help_message("optional")
        .prompt()?;
    if desc.is_empty() {
        Ok(None)
    } else {
        Ok(Some(desc))
    }
}

pub(super) fn prompt_refs() -> Result<Vec<String>> {
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
