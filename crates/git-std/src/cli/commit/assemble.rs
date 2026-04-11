use std::io::IsTerminal;

use crate::config::ProjectConfig;
use anyhow::{Result, bail};
use standard_commit::ConventionalCommit;

use super::CommitOptions;
use super::prompt;

/// Raw prompt answers before assembly into a `ConventionalCommit`.
pub(super) struct PromptAnswers {
    pub(super) commit_type: String,
    pub(super) scope: Option<String>,
    pub(super) description: String,
    pub(super) body: Option<String>,
    pub(super) breaking: Option<String>,
    pub(super) refs: Vec<String>,
    pub(super) extra_footers: Vec<String>,
}

/// Assemble a `ConventionalCommit` from raw prompt answers.
pub(super) fn build_commit(answers: PromptAnswers) -> ConventionalCommit {
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
    for raw in &answers.extra_footers {
        if let Some((token, value)) = parse_trailer(raw) {
            footers.push(standard_commit::Footer {
                token: token.to_string(),
                value: value.to_string(),
            });
        }
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

/// Gather answers from flags and/or interactive prompts.
///
/// When all required fields (`--type` and `--message`) are provided via flags,
/// prompts are skipped entirely (non-interactive mode). When some flags are
/// given, only the missing fields are prompted.
pub(super) fn gather_answers(
    config: &ProjectConfig,
    opts: &CommitOptions,
) -> Result<PromptAnswers> {
    let fully_non_interactive = opts.commit_type.is_some() && opts.message.is_some();

    if !fully_non_interactive && !std::io::stdin().is_terminal() {
        bail!(
            "interactive prompts require a TTY \u{2014} use --message to provide a commit message non-interactively"
        );
    }

    let commit_type = if let Some(t) = &opts.commit_type {
        t.clone()
    } else {
        prompt::prompt_type(&config.types)?
    };

    let scope = if opts.scope.is_some() {
        opts.scope.clone()
    } else if fully_non_interactive {
        None
    } else {
        prompt::prompt_scope(config)?
    };

    let description = if let Some(m) = &opts.message {
        m.clone()
    } else {
        prompt::prompt_description()?
    };

    let body = if opts.body.is_some() {
        opts.body.clone()
    } else if fully_non_interactive {
        None
    } else {
        prompt::prompt_body()?
    };

    let breaking = if opts.breaking.is_some() {
        opts.breaking.clone()
    } else if fully_non_interactive {
        None
    } else {
        prompt::prompt_breaking()?
    };

    let refs = if fully_non_interactive {
        vec![]
    } else {
        prompt::prompt_refs()?
    };

    // Collect extra footers from --footer flags, --signoff, and interactive prompt.
    let mut extra_footers = opts.footer.clone();

    if opts.signoff {
        let dir = std::path::Path::new(".");
        let signoff = resolve_signoff(dir)?;
        extra_footers.push(signoff);
    }

    if !fully_non_interactive {
        let prompted = prompt::prompt_footers()?;
        extra_footers.extend(prompted);
    }

    Ok(PromptAnswers {
        commit_type,
        scope,
        description,
        body,
        breaking,
        refs,
        extra_footers,
    })
}

/// Parse a raw trailer string like `"Token: value"` into `(token, value)`.
fn parse_trailer(raw: &str) -> Option<(&str, &str)> {
    // Support both "Token: value" and "Token #value" (git trailer separators).
    if let Some(pos) = raw.find(": ") {
        let token = raw[..pos].trim();
        let value = raw[pos + 2..].trim();
        if !token.is_empty() && !value.is_empty() {
            return Some((token, value));
        }
    }
    if let Some(pos) = raw.find(" #") {
        let token = raw[..pos].trim();
        let value = raw[pos + 2..].trim();
        if !token.is_empty() && !value.is_empty() {
            return Some((token, value));
        }
    }
    None
}

/// Build a `Signed-off-by` trailer from git config.
fn resolve_signoff(dir: &std::path::Path) -> Result<String> {
    let name = crate::git::config_value(dir, "user.name")
        .map_err(|e| anyhow::anyhow!("cannot read git user.name: {e}"))?;
    let email = crate::git::config_value(dir, "user.email")
        .map_err(|e| anyhow::anyhow!("cannot read git user.email: {e}"))?;
    Ok(format!("Signed-off-by: {name} <{email}>"))
}
