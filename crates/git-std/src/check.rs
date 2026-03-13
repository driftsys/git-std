use std::path::Path;

use standard_commit::LintConfig;

/// Run the `check` subcommand with an inline message. Returns the exit code.
pub fn run(message: &str, lint_config: Option<&LintConfig>) -> i32 {
    if let Some(config) = lint_config {
        let errors = standard_commit::lint(message, config);
        if errors.is_empty() {
            return 0;
        }
        for error in &errors {
            eprintln!("\u{2717} {error}");
        }
        eprintln!("  Expected: <type>(<scope>): <description>");
        eprintln!("  Got:      {}", first_line(message));
        return 1;
    }

    match standard_commit::parse(message) {
        Ok(_) => 0,
        Err(e) => {
            print_diagnostic(message, &e);
            1
        }
    }
}

/// Read a commit message from a file, strip comment lines, and validate.
pub fn run_file(path: &Path, lint_config: Option<&LintConfig>) -> i32 {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: cannot read {}: {e}", path.display());
            return 2;
        }
    };
    let message = strip_comments(&content);
    run(&message, lint_config)
}

/// Validate all commits in a git revision range. Returns 0 if all valid, 1 if any invalid.
pub fn run_range(range: &str, lint_config: Option<&LintConfig>) -> i32 {
    let repo = match git2::Repository::discover(".") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: cannot open repository: {e}");
            return 2;
        }
    };

    let commits = match walk_range(&repo, range) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: invalid range '{range}': {e}");
            return 2;
        }
    };

    if commits.is_empty() {
        eprintln!("error: no commits in range '{range}'");
        return 2;
    }

    let mut failures = 0;
    for (oid, message) in &commits {
        let short = &oid.to_string()[..7];
        let valid = if let Some(config) = lint_config {
            let errors = standard_commit::lint(message, config);
            if errors.is_empty() {
                true
            } else {
                eprintln!("\u{2717} {short} {}", first_line(message));
                for error in &errors {
                    eprintln!("  {error}");
                }
                false
            }
        } else {
            match standard_commit::parse(message) {
                Ok(_) => true,
                Err(e) => {
                    eprintln!("\u{2717} {short} {}", first_line(message));
                    eprintln!("  {e}");
                    false
                }
            }
        };

        if valid {
            eprintln!("\u{2713} {short} {}", first_line(message));
        } else {
            failures += 1;
        }
    }

    if failures > 0 { 1 } else { 0 }
}

/// Strip lines starting with `#` (git comment convention).
fn strip_comments(content: &str) -> String {
    content
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Walk a revision range and collect (oid, message) pairs.
fn walk_range(
    repo: &git2::Repository,
    range: &str,
) -> Result<Vec<(git2::Oid, String)>, git2::Error> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_range(range)?;

    let mut commits = Vec::new();
    for oid in revwalk {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let message = commit.message().unwrap_or("").to_string();
        commits.push((oid, message));
    }
    Ok(commits)
}

fn print_diagnostic(message: &str, error: &standard_commit::ParseError) {
    eprintln!("\u{2717} invalid: {error}");
    eprintln!("  Expected: <type>(<scope>): <description>");
    eprintln!("  Got:      {}", first_line(message));
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
