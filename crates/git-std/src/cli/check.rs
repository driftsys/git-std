use std::path::Path;

use clap::ValueEnum;
use serde::Serialize;
use yansi::Paint;

use crate::ui;
use standard_commit::LintConfig;

/// Output format for the check subcommand.
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable text (default).
    Text,
    /// Machine-readable JSON.
    Json,
}

/// JSON output schema for a single commit check.
#[derive(Serialize)]
struct CheckResult {
    valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    breaking: Option<bool>,
    errors: Vec<String>,
}

/// Run the `check` subcommand with an inline message. Returns the exit code.
pub fn run(message: &str, lint_config: Option<&LintConfig>, format: OutputFormat) -> i32 {
    if format == OutputFormat::Json {
        return run_json(message, lint_config);
    }

    if let Some(config) = lint_config {
        let errors = standard_commit::lint(message, config);
        if errors.is_empty() {
            eprintln!("{} {}", ui::pass(), "valid".green());
            return 0;
        }
        for error in &errors {
            eprintln!("{} {}", ui::fail(), error.to_string().red());
        }
        eprintln!(
            "{INDENT}Expected: <type>(<scope>): <description>",
            INDENT = ui::INDENT
        );
        eprintln!(
            "{INDENT}Got:      {}",
            first_line(message),
            INDENT = ui::INDENT
        );
        return 1;
    }

    match standard_commit::parse(message) {
        Ok(_) => {
            eprintln!("{} {}", ui::pass(), "valid".green());
            0
        }
        Err(e) => {
            print_diagnostic(message, &e);
            1
        }
    }
}

/// Run check with JSON output.
fn run_json(message: &str, lint_config: Option<&LintConfig>) -> i32 {
    let result = if let Some(config) = lint_config {
        let errors = standard_commit::lint(message, config);
        if errors.is_empty() {
            build_valid_result(message)
        } else {
            CheckResult {
                valid: false,
                r#type: None,
                scope: None,
                description: None,
                breaking: None,
                errors: errors.iter().map(|e| e.to_string()).collect(),
            }
        }
    } else {
        match standard_commit::parse(message) {
            Ok(_) => build_valid_result(message),
            Err(e) => CheckResult {
                valid: false,
                r#type: None,
                scope: None,
                description: None,
                breaking: None,
                errors: vec![e.to_string()],
            },
        }
    };

    let code = if result.valid { 0 } else { 1 };
    println!("{}", serde_json::to_string(&result).unwrap());
    code
}

/// Build a valid CheckResult by parsing the commit message.
fn build_valid_result(message: &str) -> CheckResult {
    match standard_commit::parse(message) {
        Ok(commit) => CheckResult {
            valid: true,
            r#type: Some(commit.r#type),
            scope: commit.scope,
            description: Some(commit.description),
            breaking: Some(commit.is_breaking),
            errors: vec![],
        },
        Err(_) => CheckResult {
            valid: true,
            r#type: None,
            scope: None,
            description: None,
            breaking: None,
            errors: vec![],
        },
    }
}

/// Read a commit message from a file, strip comment lines, and validate.
pub fn run_file(path: &Path, lint_config: Option<&LintConfig>, format: OutputFormat) -> i32 {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            ui::error(&format!("cannot read {}: {e}", path.display()));
            return 2;
        }
    };
    let message = strip_comments(&content);
    run(&message, lint_config, format)
}

/// Validate all commits in a git revision range. Returns 0 if all valid, 1 if any invalid.
pub fn run_range(range: &str, lint_config: Option<&LintConfig>, format: OutputFormat) -> i32 {
    let dir = std::path::Path::new(".");

    let commits = match crate::git::walk_range(dir, range) {
        Ok(c) => c,
        Err(e) => {
            ui::error(&format!("invalid range '{range}': {e}"));
            return 2;
        }
    };

    if commits.is_empty() {
        ui::error(&format!("no commits in range '{range}'"));
        return 2;
    }

    if format == OutputFormat::Json {
        return run_range_json(&commits, lint_config);
    }

    let total = commits.len();
    let mut failures = 0;
    for (oid, message) in &commits {
        let short = &oid[..7];
        let valid = if let Some(config) = lint_config {
            let errors = standard_commit::lint(message, config);
            if errors.is_empty() {
                true
            } else {
                eprintln!(
                    "{INDENT}{} {} {}",
                    ui::fail(),
                    short,
                    first_line(message).red(),
                    INDENT = ui::INDENT,
                );
                for error in &errors {
                    eprintln!("{DETAIL}\u{2192} {}", error, DETAIL = ui::DETAIL_INDENT,);
                }
                false
            }
        } else {
            match standard_commit::parse(message) {
                Ok(_) => true,
                Err(e) => {
                    eprintln!(
                        "{INDENT}{} {} {}",
                        ui::fail(),
                        short,
                        first_line(message).red(),
                        INDENT = ui::INDENT,
                    );
                    eprintln!("{DETAIL}\u{2192} {}", e, DETAIL = ui::DETAIL_INDENT,);
                    false
                }
            }
        };

        if valid {
            eprintln!(
                "{INDENT}{} {} {}",
                ui::pass(),
                short,
                first_line(message).green(),
                INDENT = ui::INDENT,
            );
        } else {
            failures += 1;
        }
    }

    let valid_count = total - failures;
    ui::blank();
    eprintln!("{}/{} valid", valid_count, total);

    if failures > 0 { 1 } else { 0 }
}

/// Run range check with JSON output — outputs a JSON array.
fn run_range_json(commits: &[(String, String)], lint_config: Option<&LintConfig>) -> i32 {
    let mut results = Vec::new();
    let mut any_invalid = false;

    for (_oid, message) in commits {
        let result = if let Some(config) = lint_config {
            let errors = standard_commit::lint(message, config);
            if errors.is_empty() {
                build_valid_result(message)
            } else {
                CheckResult {
                    valid: false,
                    r#type: None,
                    scope: None,
                    description: None,
                    breaking: None,
                    errors: errors.iter().map(|e| e.to_string()).collect(),
                }
            }
        } else {
            match standard_commit::parse(message) {
                Ok(_) => build_valid_result(message),
                Err(e) => CheckResult {
                    valid: false,
                    r#type: None,
                    scope: None,
                    description: None,
                    breaking: None,
                    errors: vec![e.to_string()],
                },
            }
        };

        if !result.valid {
            any_invalid = true;
        }
        results.push(result);
    }

    println!("{}", serde_json::to_string(&results).unwrap());
    if any_invalid { 1 } else { 0 }
}

/// Strip lines starting with `#` (git comment convention).
fn strip_comments(content: &str) -> String {
    content
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

fn print_diagnostic(message: &str, error: &standard_commit::ParseError) {
    eprintln!("{} {}", ui::fail(), format!("invalid: {error}").red());
    eprintln!(
        "{INDENT}Expected: <type>(<scope>): <description>",
        INDENT = ui::INDENT
    );
    eprintln!(
        "{INDENT}Got:      {}",
        first_line(message),
        INDENT = ui::INDENT
    );
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_comments_removes_hash_lines() {
        let input = "feat: add login\n# This is a comment\n\nBody text\n# Another comment";
        let result = strip_comments(input);
        assert_eq!(result, "feat: add login\n\nBody text");
    }

    #[test]
    fn strip_comments_preserves_non_comment_lines() {
        let input = "fix: handle error\n\nSome body";
        let result = strip_comments(input);
        assert_eq!(result, input);
    }
}
