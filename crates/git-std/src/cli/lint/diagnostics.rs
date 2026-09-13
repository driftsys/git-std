use standard_commit::LintConfig;

use crate::contract::{Diagnostic, Severity};

pub(super) fn lint_diagnostics(message: &str, lint_config: Option<&LintConfig>) -> Vec<Diagnostic> {
    if let Some(config) = lint_config {
        standard_commit::lint(message, config)
            .iter()
            .map(lint_diagnostic)
            .collect()
    } else {
        standard_commit::parse(message)
            .err()
            .map(|error| vec![parse_diagnostic(&error)])
            .unwrap_or_default()
    }
}

pub(super) fn lint_diagnostic(error: &standard_commit::LintError) -> Diagnostic {
    Diagnostic {
        code: error.code,
        severity: Severity::Error,
        message: error.message.clone(),
    }
}

pub(super) fn parse_diagnostic(error: &standard_commit::ParseError) -> Diagnostic {
    Diagnostic {
        code: standard_commit::rules::PARSE_CODE,
        severity: Severity::Error,
        message: error.to_string(),
    }
}
