use std::path::Path;

use serde::Serialize;
use yansi::Paint;

use crate::app::OutputFormat;
use crate::ui;
use standard_commit::LintConfig;

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
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    skipped: bool,
}

/// Run the `check` subcommand with an inline message. Returns the exit code.
pub fn run(message: &str, lint_config: Option<&LintConfig>, format: OutputFormat) -> i32 {
    if format == OutputFormat::Json {
        return run_json(message, lint_config);
    }

    if let Some(config) = lint_config {
        let errors = standard_commit::lint(message, config);
        if errors.is_empty() {
            ui::print(&format!("{} {}", ui::pass(), "valid".green()));
            return 0;
        }
        for error in &errors {
            ui::print(&format!("{} {}", ui::fail(), error.to_string().red()));
        }
        ui::info("Expected: <type>(<scope>): <description>");
        ui::info(&format!("Got:      {}", first_line(message)));
        return 1;
    }

    match standard_commit::parse(message) {
        Ok(_) => {
            ui::print(&format!("{} {}", ui::pass(), "valid".green()));
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
                skipped: false,
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
                skipped: false,
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
            skipped: false,
        },
        Err(_) => CheckResult {
            valid: true,
            r#type: None,
            scope: None,
            description: None,
            breaking: None,
            errors: vec![],
            skipped: false,
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
    let mut skipped = 0;
    for (oid, message) in &commits {
        let short = &oid[..7];

        if standard_commit::is_process_commit(message) {
            ui::result_line(&format!("~ {} {}", short, first_line(message).dim(),));
            skipped += 1;
            continue;
        }

        let valid = if let Some(config) = lint_config {
            let errors = standard_commit::lint(message, config);
            if errors.is_empty() {
                true
            } else {
                ui::result_line(&format!(
                    "{} {} {}",
                    ui::fail(),
                    short,
                    first_line(message).red(),
                ));
                for error in &errors {
                    ui::detail(&format!("\u{2192} {}", error));
                }
                false
            }
        } else {
            match standard_commit::parse(message) {
                Ok(_) => true,
                Err(e) => {
                    ui::result_line(&format!(
                        "{} {} {}",
                        ui::fail(),
                        short,
                        first_line(message).red(),
                    ));
                    ui::detail(&format!("\u{2192} {}", e));
                    false
                }
            }
        };

        if valid {
            ui::result_line(&format!(
                "{} {} {}",
                ui::pass(),
                short,
                first_line(message).green(),
            ));
        } else {
            failures += 1;
        }
    }

    let checked = total - skipped;
    let valid_count = checked - failures;
    ui::blank();
    if skipped > 0 {
        eprintln!("{valid_count}/{checked} valid  ({skipped} skipped)");
    } else {
        ui::summary_counts(valid_count, checked);
    }

    if failures > 0 { 1 } else { 0 }
}

/// Run range check with JSON output — outputs a JSON array.
fn run_range_json(commits: &[(String, String)], lint_config: Option<&LintConfig>) -> i32 {
    let mut results = Vec::new();
    let mut any_invalid = false;

    for (_oid, message) in commits {
        if standard_commit::is_process_commit(message) {
            results.push(CheckResult {
                valid: true,
                r#type: None,
                scope: None,
                description: None,
                breaking: None,
                errors: vec![],
                skipped: true,
            });
            continue;
        }

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
                    skipped: false,
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
                    skipped: false,
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
    ui::print(&format!(
        "{} {}",
        ui::fail(),
        format!("invalid: {error}").red()
    ));
    ui::info("Expected: <type>(<scope>): <description>");
    ui::info(&format!("Got:      {}", first_line(message)));
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
