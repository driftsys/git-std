use std::path::Path;

use serde::Serialize;
use yansi::Paint;

use crate::app::LintOutputFormat;
use crate::contract::{ContractMetadata, Diagnostic, print_json_error};
use crate::ui;
use standard_commit::LintConfig;

mod diagnostics;
mod sarif;

use diagnostics::{lint_diagnostic, lint_diagnostics, parse_diagnostic};

/// JSON output schema for a single commit lint result.
#[derive(Serialize)]
struct LintResult {
    #[serde(flatten)]
    metadata: ContractMetadata,
    status: &'static str,
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
    diagnostics: Vec<Diagnostic>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    skipped: bool,
}

/// Run the `lint` subcommand with an inline message. Returns the exit code.
pub fn run(message: &str, lint_config: Option<&LintConfig>, format: LintOutputFormat) -> i32 {
    if format == LintOutputFormat::Sarif {
        return run_sarif(message, lint_config);
    }
    if format == LintOutputFormat::Json {
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

/// Emit an operational machine document for a missing lint input.
pub fn run_no_input(format: LintOutputFormat) -> i32 {
    let message = "no lint input provided";
    match format {
        LintOutputFormat::Json => {
            print_json_error(Diagnostic::operational("GITSTD-INVALID-ARGUMENT", message))
        }
        LintOutputFormat::Sarif => sarif::emit_operational("GITSTD-INVALID-ARGUMENT", message),
        LintOutputFormat::Text => 2,
    }
}

/// Emit SARIF for a command-line parsing failure.
pub fn run_usage_error_sarif(message: &str) -> i32 {
    sarif::emit_operational("GITSTD-INVALID-ARGUMENT", message)
}

/// Run lint with JSON output.
fn run_json(message: &str, lint_config: Option<&LintConfig>) -> i32 {
    let result = if let Some(config) = lint_config {
        let errors = standard_commit::lint(message, config);
        if errors.is_empty() {
            build_valid_result(message)
        } else {
            LintResult {
                metadata: ContractMetadata::current(),
                status: "finding",
                valid: false,
                r#type: None,
                scope: None,
                description: None,
                breaking: None,
                errors: errors.iter().map(|e| e.to_string()).collect(),
                diagnostics: errors.iter().map(lint_diagnostic).collect(),
                skipped: false,
            }
        }
    } else {
        match standard_commit::parse(message) {
            Ok(_) => build_valid_result(message),
            Err(e) => LintResult {
                metadata: ContractMetadata::current(),
                status: "finding",
                valid: false,
                r#type: None,
                scope: None,
                description: None,
                breaking: None,
                errors: vec![e.to_string()],
                diagnostics: vec![parse_diagnostic(&e)],
                skipped: false,
            },
        }
    };

    let code = if result.valid { 0 } else { 1 };
    println!("{}", serde_json::to_string(&result).unwrap());
    code
}

fn run_sarif(message: &str, lint_config: Option<&LintConfig>) -> i32 {
    let diagnostics = lint_diagnostics(message, lint_config);
    sarif::emit_findings(&diagnostics)
}

/// Build a valid LintResult by parsing the commit message.
fn build_valid_result(message: &str) -> LintResult {
    match standard_commit::parse(message) {
        Ok(commit) => LintResult {
            metadata: ContractMetadata::current(),
            status: "success",
            valid: true,
            r#type: Some(commit.r#type),
            scope: commit.scope,
            description: Some(commit.description),
            breaking: Some(commit.is_breaking),
            errors: vec![],
            diagnostics: vec![],
            skipped: false,
        },
        Err(_) => LintResult {
            metadata: ContractMetadata::current(),
            status: "success",
            valid: true,
            r#type: None,
            scope: None,
            description: None,
            breaking: None,
            errors: vec![],
            diagnostics: vec![],
            skipped: false,
        },
    }
}

/// Read a commit message from a file, strip comment lines, and validate.
pub fn run_file(path: &Path, lint_config: Option<&LintConfig>, format: LintOutputFormat) -> i32 {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            let message = format!("cannot read {}: {e}", path.display());
            return match format {
                LintOutputFormat::Json => {
                    print_json_error(Diagnostic::operational("GITSTD-IO-READ", message))
                }
                LintOutputFormat::Sarif => sarif::emit_operational("GITSTD-IO-READ", &message),
                LintOutputFormat::Text => {
                    ui::error(&message);
                    2
                }
            };
        }
    };
    let message = strip_comments(&content);
    run(&message, lint_config, format)
}

/// Validate all commits in a git revision range.
///
/// Returns 0 if every commit is valid or the range is empty, 1 if any commit is
/// invalid or the range is empty while its inverse direction has commits, and 2
/// if the range is malformed or cannot be resolved.
pub fn run_range(range: &str, lint_config: Option<&LintConfig>, format: LintOutputFormat) -> i32 {
    let dir = std::path::Path::new(".");

    let commits = match crate::git::walk_range(dir, range) {
        Ok(c) => c,
        Err(e) => {
            let message = format!("invalid range '{range}': {e}");
            return match format {
                LintOutputFormat::Json => {
                    print_json_error(Diagnostic::operational("GITSTD-GIT-OPERATION", message))
                }
                LintOutputFormat::Sarif => {
                    sarif::emit_operational("GITSTD-GIT-OPERATION", &message)
                }
                LintOutputFormat::Text => {
                    ui::error(&message);
                    2
                }
            };
        }
    };

    if commits.is_empty() {
        return run_empty_range(dir, range, format);
    }

    if format == LintOutputFormat::Json {
        return run_range_json(&commits, lint_config);
    }
    if format == LintOutputFormat::Sarif {
        return run_range_sarif(&commits, lint_config);
    }

    let total = commits.len();
    let mut failures = 0;
    let mut skipped = 0;
    for (oid, message) in &commits {
        let short = &oid[..7];

        if standard_commit::is_process_commit(message) {
            ui::info(&format!("~ {} {}", short, first_line(message).dim(),));
            skipped += 1;
            continue;
        }

        let valid = if let Some(config) = lint_config {
            let errors = standard_commit::lint(message, config);
            if errors.is_empty() {
                true
            } else {
                ui::info(&format!(
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
                    ui::info(&format!(
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
            ui::info(&format!(
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
        ui::print(&format!(
            "{valid_count}/{checked} valid  ({skipped} skipped)"
        ));
    } else {
        ui::summary_counts(valid_count, checked);
    }

    if failures > 0 { 1 } else { 0 }
}

/// Report a revision range that contains no commits. Returns the exit code.
///
/// An ordinary empty range succeeds. When its inverse has commits, it returns
/// finding exit 1 so a commit gate cannot pass after validating nothing.
fn run_empty_range(dir: &Path, range: &str, format: LintOutputFormat) -> i32 {
    match inverse_range_with_commits(dir, range) {
        Some(inverse) => {
            ui::warning(&format!("range '{range}' is empty"));
            ui::hint(&format!("did you mean '{inverse}'?"));
            match format {
                LintOutputFormat::Json => {
                    // Preserve the established range-array contract; the process
                    // exit code carries the rejected-empty-range verdict.
                    let _ = run_range_json(&[], None);
                    1
                }
                LintOutputFormat::Sarif => sarif::emit_findings(&[Diagnostic::operational(
                    "GITSTD-LINT-EMPTY-RANGE",
                    format!("range '{range}' is empty; inverse '{inverse}' contains commits"),
                )]),
                LintOutputFormat::Text => 1,
            }
        }
        None => match format {
            LintOutputFormat::Json => run_range_json(&[], None),
            LintOutputFormat::Sarif => sarif::emit_findings(&[]),
            LintOutputFormat::Text => {
                ui::info(&format!("no commits in range '{range}'"));
                0
            }
        },
    }
}

/// Return `range` with its endpoints swapped, when that inverse direction has
/// commits, which is what a range written backwards looks like.
///
/// Returns `None` when the range ends at the commit the working tree is on, by
/// any name — `HEAD`, `@`, a branch, or a tag. `<base>..HEAD` is the gate form,
/// and an empty result there means the checkout carries nothing of its own, as in
/// a worktree branched from an older `main`. In git that is the same state as
/// reversed endpoints, so only the endpoint order separates them, and a caller
/// who put the checkout on the right asked a question an empty answer answers
/// honestly.
///
/// Returns `None` in three further cases. An inverse that resolves but has no
/// commits is an ordinary empty range. A range with no `..` separator cannot be
/// inverted at all. An inverse git cannot resolve covers the symmetric-difference
/// form `a...b`, which splits into `a` and `.b` and inverts to `.b..a` — never a
/// valid ref, since a ref cannot start with a dot. Reporting that last form as
/// empty rather than reversed is the right answer: a symmetric difference is
/// empty in both directions.
fn inverse_range_with_commits(dir: &Path, range: &str) -> Option<String> {
    let (from, to) = range.split_once("..")?;

    if names_head(dir, to) {
        return None;
    }

    let inverse = format!("{to}..{from}");
    match crate::git::range_has_commits(dir, &inverse) {
        Ok(true) => Some(inverse),
        _ => None,
    }
}

/// Return `true` when `spec` names the commit the working tree is on, whatever
/// kind of ref points at it.
///
/// Both sides are peeled to a commit with `^{commit}`, because an annotated tag
/// resolves to a tag object rather than to the commit it points at, and
/// `git std bump` creates annotated tags. An omitted endpoint counts, because
/// git reads one as `HEAD`. A spec git cannot resolve does not, so an unusual
/// endpoint keeps the reversed check.
fn names_head(dir: &Path, spec: &str) -> bool {
    let spec = if spec.is_empty() { "HEAD" } else { spec };
    match (
        crate::git::resolve_rev(dir, &format!("{spec}^{{commit}}")),
        crate::git::resolve_rev(dir, "HEAD^{commit}"),
    ) {
        (Ok(endpoint), Ok(head)) => endpoint == head,
        _ => false,
    }
}

/// Run range lint with JSON output — outputs a JSON array.
fn run_range_json(commits: &[(String, String)], lint_config: Option<&LintConfig>) -> i32 {
    let mut results = Vec::new();
    let mut any_invalid = false;

    for (_oid, message) in commits {
        if standard_commit::is_process_commit(message) {
            results.push(LintResult {
                metadata: ContractMetadata::current(),
                status: "success",
                valid: true,
                r#type: None,
                scope: None,
                description: None,
                breaking: None,
                errors: vec![],
                diagnostics: vec![],
                skipped: true,
            });
            continue;
        }

        let result = if let Some(config) = lint_config {
            let errors = standard_commit::lint(message, config);
            if errors.is_empty() {
                build_valid_result(message)
            } else {
                LintResult {
                    metadata: ContractMetadata::current(),
                    status: "finding",
                    valid: false,
                    r#type: None,
                    scope: None,
                    description: None,
                    breaking: None,
                    errors: errors.iter().map(|e| e.to_string()).collect(),
                    diagnostics: errors.iter().map(lint_diagnostic).collect(),
                    skipped: false,
                }
            }
        } else {
            match standard_commit::parse(message) {
                Ok(_) => build_valid_result(message),
                Err(e) => LintResult {
                    metadata: ContractMetadata::current(),
                    status: "finding",
                    valid: false,
                    r#type: None,
                    scope: None,
                    description: None,
                    breaking: None,
                    errors: vec![e.to_string()],
                    diagnostics: vec![parse_diagnostic(&e)],
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

fn run_range_sarif(commits: &[(String, String)], lint_config: Option<&LintConfig>) -> i32 {
    let diagnostics: Vec<Diagnostic> = commits
        .iter()
        .filter(|(_, message)| !standard_commit::is_process_commit(message))
        .flat_map(|(_, message)| lint_diagnostics(message, lint_config))
        .collect();
    sarif::emit_findings(&diagnostics)
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
