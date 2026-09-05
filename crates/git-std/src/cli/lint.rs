use std::path::Path;

use serde::Serialize;
use yansi::Paint;

use crate::app::OutputFormat;
use crate::ui;
use standard_commit::LintConfig;

/// JSON output schema for a single commit lint result.
#[derive(Serialize)]
struct LintResult {
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

/// Run the `lint` subcommand with an inline message. Returns the exit code.
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

/// Run lint with JSON output.
fn run_json(message: &str, lint_config: Option<&LintConfig>) -> i32 {
    let result = if let Some(config) = lint_config {
        let errors = standard_commit::lint(message, config);
        if errors.is_empty() {
            build_valid_result(message)
        } else {
            LintResult {
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
            Err(e) => LintResult {
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

/// Build a valid LintResult by parsing the commit message.
fn build_valid_result(message: &str) -> LintResult {
    match standard_commit::parse(message) {
        Ok(commit) => LintResult {
            valid: true,
            r#type: Some(commit.r#type),
            scope: commit.scope,
            description: Some(commit.description),
            breaking: Some(commit.is_breaking),
            errors: vec![],
            skipped: false,
        },
        Err(_) => LintResult {
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

/// Validate all commits in a git revision range.
///
/// Returns 0 if every commit is valid or the range is empty, 1 if any commit is
/// invalid or the range is empty while its inverse direction has commits, and 2
/// if the range is malformed or cannot be resolved.
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
        return run_empty_range(dir, range, format);
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
        eprintln!("{valid_count}/{checked} valid  ({skipped} skipped)");
    } else {
        ui::summary_counts(valid_count, checked);
    }

    if failures > 0 { 1 } else { 0 }
}

/// Report a revision range that contains no commits. Returns the exit code.
///
/// Nothing to lint is not a failure, so an empty range returns 0 — `main..HEAD`
/// is empty on `main` itself, which is an ordinary state.
///
/// When the inverse direction has commits the range returns 1 instead, because
/// returning 0 would let a commit gate pass having validated nothing. That
/// condition covers endpoints written in the wrong order and a left endpoint
/// that is simply ahead of the right one; the two are the same state in git, so
/// the hint names the inverse range as a suggestion rather than a diagnosis.
fn run_empty_range(dir: &Path, range: &str, format: OutputFormat) -> i32 {
    if format == OutputFormat::Json {
        // Machine output stays a valid array even when an empty range is rejected.
        // No commits means no verdict to report, so the exit code below carries it.
        let _ = run_range_json(&[], None);
    }

    match inverse_range_with_commits(dir, range) {
        Some(inverse) => {
            ui::warning(&format!("range '{range}' is empty"));
            ui::hint(&format!("did you mean '{inverse}'?"));
            1
        }
        None => {
            if format != OutputFormat::Json {
                ui::info(&format!("no commits in range '{range}'"));
            }
            0
        }
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
                LintResult {
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
                Err(e) => LintResult {
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
