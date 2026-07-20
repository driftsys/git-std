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
            let discovered = config.resolved_scopes(&cwd, None);
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
                let staged = crate::git::staged_files(&cwd).unwrap_or_default();
                let unmatched_dir = crate::config::unmatched_scope_dir(&staged, &discovered);
                if let Some(dir) = &unmatched_dir {
                    crate::ui::hint(&format!(
                        "staged path '{dir}/' doesn't match any configured scope"
                    ));
                } else {
                    let meta = config.meta_scope(&cwd);
                    if discovered.iter().any(|s| s == &meta)
                        && crate::config::meta_scope_suggested(&staged)
                    {
                        crate::ui::hint(&format!(
                            "staged files touch root-level or multiple scopes — consider `{meta}`"
                        ));
                    }
                }
                let items = scope_select_items(&discovered, unmatched_dir.as_deref());
                let item_refs: Vec<&str> = items.iter().map(|s| s.as_str()).collect();
                let selection = Select::new("scope:", item_refs).prompt()?;
                if selection == OTHER_SCOPE {
                    let scope = Text::new("new scope:").prompt()?;
                    Ok(Some(scope))
                } else {
                    Ok(Some(selection.to_string()))
                }
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

pub(super) fn prompt_footers() -> Result<Vec<String>> {
    let mut footers: Vec<String> = Vec::new();
    loop {
        let input = Text::new("footer:")
            .with_help_message("optional, e.g. Co-authored-by: Name <email>")
            .prompt()?;
        if input.is_empty() {
            break;
        }
        footers.push(input);
    }
    Ok(footers)
}

/// Sentinel item appended to the scope `Select` when a staged path doesn't
/// match any discovered scope, letting the user type a new one instead.
const OTHER_SCOPE: &str = "other (type a new scope)";

/// Build the choice list for the Auto-mode scope `Select` prompt.
///
/// Appends [`OTHER_SCOPE`] when `unmatched_dir` is `Some`, signalling that a
/// staged path didn't match any discovered scope.
fn scope_select_items(discovered: &[String], unmatched_dir: Option<&str>) -> Vec<String> {
    let mut items = discovered.to_vec();
    if unmatched_dir.is_some() {
        items.push(OTHER_SCOPE.to_string());
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_select_items_plain_when_no_unmatched_dir() {
        let items = scope_select_items(&["api".to_string(), "cli".to_string()], None);
        assert_eq!(items, vec!["api", "cli"]);
    }

    #[test]
    fn scope_select_items_appends_other_when_unmatched_dir() {
        let items = scope_select_items(&["api".to_string()], Some("services"));
        assert_eq!(items, vec!["api", OTHER_SCOPE]);
    }
}
