/// The execution mode for a hook command.
///
/// Controls what happens when the command exits with a non-zero status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prefix {
    /// No explicit prefix — uses the hook's default execution mode.
    Default,
    /// `!` prefix — abort the hook immediately on failure.
    FailFast,
    /// `?` prefix — report as a warning, never cause the hook to fail.
    Advisory,
}

/// A single command parsed from a `.githooks/<hook>.hooks` file.
///
/// Each non-blank, non-comment line in a hooks file produces one
/// `HookCommand`. The line format is:
///
/// ```text
/// [prefix]command [arguments] [glob]
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookCommand {
    /// The execution mode prefix (`!`, `?`, or none).
    pub prefix: Prefix,
    /// The command text (executable and arguments, without prefix or glob).
    pub command: String,
    /// Optional trailing glob pattern that restricts the command to matching
    /// staged or tracked files.
    pub glob: Option<String>,
}

/// Parse the text content of a `.githooks/<hook>.hooks` file.
///
/// Blank lines and comment lines (starting with `#`) are skipped.
/// Each remaining line is parsed into a [`HookCommand`] with its
/// prefix, command text, and optional trailing glob pattern.
///
/// # Example
///
/// ```
/// use standard_githooks::{parse, Prefix};
///
/// let input = "# Formatting\ndprint check\n!cargo clippy --workspace -- -D warnings *.rs\n? detekt --input modules/ *.kt\n";
///
/// let commands = parse(input);
/// assert_eq!(commands.len(), 3);
/// assert_eq!(commands[0].prefix, Prefix::Default);
/// assert_eq!(commands[0].command, "dprint check");
/// assert_eq!(commands[0].glob, None);
///
/// assert_eq!(commands[1].prefix, Prefix::FailFast);
/// assert_eq!(commands[1].command, "cargo clippy --workspace -- -D warnings");
/// assert_eq!(commands[1].glob, Some("*.rs".to_string()));
///
/// assert_eq!(commands[2].prefix, Prefix::Advisory);
/// assert_eq!(commands[2].command, "detekt --input modules/");
/// assert_eq!(commands[2].glob, Some("*.kt".to_string()));
/// ```
pub fn parse(content: &str) -> Vec<HookCommand> {
    content
        .lines()
        .filter_map(|line| parse_line(line.trim()))
        .collect()
}

/// Parse a single trimmed line into a `HookCommand`, or `None` if the
/// line is blank or a comment.
fn parse_line(line: &str) -> Option<HookCommand> {
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let (prefix, rest) = extract_prefix(line);
    let rest = rest.trim();

    let (command, glob) = extract_glob(rest);

    Some(HookCommand {
        prefix,
        command: command.to_string(),
        glob,
    })
}

/// Extract the prefix character and return the remaining text.
fn extract_prefix(line: &str) -> (Prefix, &str) {
    if let Some(rest) = line.strip_prefix('!') {
        (Prefix::FailFast, rest)
    } else if let Some(rest) = line.strip_prefix('?') {
        (Prefix::Advisory, rest)
    } else {
        (Prefix::Default, line)
    }
}

/// Extract an optional trailing glob pattern from the command text.
///
/// A glob is the last whitespace-separated token on the line, but only
/// if it looks like a file-matching pattern (contains `*`, `[`, or
/// brace expansion like `*.{js,ts}`). Quoted tokens and substitution
/// tokens like `{msg}` are not treated as globs.
fn extract_glob(text: &str) -> (&str, Option<String>) {
    // Split at the last whitespace boundary
    if let Some(pos) = text.rfind(|c: char| c.is_ascii_whitespace()) {
        let last_token = &text[pos + 1..];
        if !last_token.starts_with('"') && is_glob(last_token) {
            let command = text[..pos].trim_end();
            return (command, Some(last_token.to_string()));
        }
    }
    (text, None)
}

/// Check whether a token looks like a glob pattern.
///
/// Recognises `*` and `[` as glob metacharacters. Brace expansion
/// (`{a,b}`) counts only when the braces contain a comma, so that
/// substitution tokens like `{msg}` are not mistaken for globs.
fn is_glob(token: &str) -> bool {
    if token.contains('*') || token.contains('[') {
        return true;
    }
    // Brace expansion requires a comma inside braces
    if let Some(open) = token.find('{')
        && let Some(close) = token[open..].find('}')
    {
        let inner = &token[open + 1..open + close];
        return inner.contains(',');
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_lines_are_skipped() {
        let commands = parse("\n\n  \n");
        assert!(commands.is_empty());
    }

    #[test]
    fn comment_lines_are_skipped() {
        let commands = parse("# This is a comment\n  # indented comment\n");
        assert!(commands.is_empty());
    }

    #[test]
    fn simple_command_no_prefix_no_glob() {
        let commands = parse("dprint check\n");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].prefix, Prefix::Default);
        assert_eq!(commands[0].command, "dprint check");
        assert_eq!(commands[0].glob, None);
    }

    #[test]
    fn fail_fast_prefix() {
        let commands = parse("!cargo build --workspace\n");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].prefix, Prefix::FailFast);
        assert_eq!(commands[0].command, "cargo build --workspace");
        assert_eq!(commands[0].glob, None);
    }

    #[test]
    fn advisory_prefix() {
        let commands = parse("? detekt --input modules/ *.kt\n");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].prefix, Prefix::Advisory);
        assert_eq!(commands[0].command, "detekt --input modules/");
        assert_eq!(commands[0].glob, Some("*.kt".to_string()));
    }

    #[test]
    fn advisory_prefix_no_space() {
        let commands = parse("?detekt --input modules/ *.kt\n");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].prefix, Prefix::Advisory);
        assert_eq!(commands[0].command, "detekt --input modules/");
        assert_eq!(commands[0].glob, Some("*.kt".to_string()));
    }

    #[test]
    fn trailing_glob_pattern() {
        let commands = parse("cargo clippy --workspace -- -D warnings *.rs\n");
        assert_eq!(commands.len(), 1);
        assert_eq!(
            commands[0].command,
            "cargo clippy --workspace -- -D warnings"
        );
        assert_eq!(commands[0].glob, Some("*.rs".to_string()));
    }

    #[test]
    fn command_with_arguments_no_glob() {
        let commands = parse("prettier --check \"**/*.md\"\n");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command, "prettier --check \"**/*.md\"");
    }

    #[test]
    fn command_with_msg_substitution() {
        let commands = parse("! git std check --file {msg}\n");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].prefix, Prefix::FailFast);
        assert_eq!(commands[0].command, "git std check --file {msg}");
        assert_eq!(commands[0].glob, None);
    }

    #[test]
    fn mixed_content() {
        let input = "\
# ── Formatting ────────────────────────────
dprint check
prettier --check \"**/*.md\"

# ── Rust ──────────────────────────────────
cargo clippy --workspace -- -D warnings *.rs
cargo test --workspace --lib *.rs

# ── Android ───────────────────────────────
? detekt --input modules/ *.kt
";
        let commands = parse(input);
        assert_eq!(commands.len(), 5);

        assert_eq!(commands[0].prefix, Prefix::Default);
        assert_eq!(commands[0].command, "dprint check");
        assert_eq!(commands[0].glob, None);

        assert_eq!(commands[1].prefix, Prefix::Default);
        assert_eq!(commands[1].command, "prettier --check \"**/*.md\"");
        assert_eq!(commands[1].glob, None);

        assert_eq!(commands[2].prefix, Prefix::Default);
        assert_eq!(
            commands[2].command,
            "cargo clippy --workspace -- -D warnings"
        );
        assert_eq!(commands[2].glob, Some("*.rs".to_string()));

        assert_eq!(commands[3].prefix, Prefix::Default);
        assert_eq!(commands[3].command, "cargo test --workspace --lib");
        assert_eq!(commands[3].glob, Some("*.rs".to_string()));

        assert_eq!(commands[4].prefix, Prefix::Advisory);
        assert_eq!(commands[4].command, "detekt --input modules/");
        assert_eq!(commands[4].glob, Some("*.kt".to_string()));
    }

    #[test]
    fn commit_msg_hooks_file() {
        let input = "! git std check --file {msg}\n";
        let commands = parse(input);
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].prefix, Prefix::FailFast);
        assert_eq!(commands[0].command, "git std check --file {msg}");
        assert_eq!(commands[0].glob, None);
    }

    #[test]
    fn glob_with_brackets() {
        let commands = parse("lint src/[a-z]*.rs\n");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command, "lint");
        assert_eq!(commands[0].glob, Some("src/[a-z]*.rs".to_string()));
    }

    #[test]
    fn glob_with_braces() {
        let commands = parse("check *.{js,ts}\n");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command, "check");
        assert_eq!(commands[0].glob, Some("*.{js,ts}".to_string()));
    }

    #[test]
    fn single_word_command() {
        let commands = parse("lint\n");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command, "lint");
        assert_eq!(commands[0].glob, None);
    }

    #[test]
    fn whitespace_handling() {
        let commands = parse("  cargo test  \n");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command, "cargo test");
        assert_eq!(commands[0].glob, None);
    }

    #[test]
    fn empty_input() {
        let commands = parse("");
        assert!(commands.is_empty());
    }

    #[test]
    fn prefix_display_coverage() {
        assert_ne!(Prefix::Default, Prefix::FailFast);
        assert_ne!(Prefix::Default, Prefix::Advisory);
        assert_ne!(Prefix::FailFast, Prefix::Advisory);
    }

    // --- Edge-case tests (#115) ---

    #[test]
    fn prefix_only_bang_produces_empty_command() {
        let commands = parse("!\n");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].prefix, Prefix::FailFast);
        assert_eq!(commands[0].command, "");
        assert_eq!(commands[0].glob, None);
    }

    #[test]
    fn prefix_only_question_mark_produces_empty_command() {
        let commands = parse("?\n");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].prefix, Prefix::Advisory);
        assert_eq!(commands[0].command, "");
        assert_eq!(commands[0].glob, None);
    }

    #[test]
    fn whitespace_only_lines_are_skipped() {
        let commands = parse("   \n\t\n  \t  \n");
        assert!(commands.is_empty());
    }

    #[test]
    fn lines_with_only_tabs() {
        let commands = parse("\t\t\t\n");
        assert!(commands.is_empty());
    }

    #[test]
    fn malformed_glob_no_star_or_bracket() {
        // A trailing token without glob metacharacters is treated as part
        // of the command, not a glob.
        let commands = parse("cargo test src/main.rs\n");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command, "cargo test src/main.rs");
        assert_eq!(commands[0].glob, None);
    }

    #[test]
    fn braces_without_comma_are_not_glob() {
        // `{msg}` has no comma, so it's not treated as a brace-expansion glob.
        let commands = parse("echo {msg}\n");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command, "echo {msg}");
        assert_eq!(commands[0].glob, None);
    }

    #[test]
    fn braces_with_comma_are_glob() {
        let commands = parse("lint src/*.{js,ts}\n");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command, "lint");
        assert_eq!(commands[0].glob, Some("src/*.{js,ts}".to_string()));
    }

    #[test]
    fn empty_braces_are_not_glob() {
        let commands = parse("echo {}\n");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command, "echo {}");
        assert_eq!(commands[0].glob, None);
    }

    #[test]
    fn prefix_with_space_before_command() {
        // `! ` (prefix + space) should work the same as `!`
        let commands = parse("! cargo test\n");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].prefix, Prefix::FailFast);
        assert_eq!(commands[0].command, "cargo test");
    }

    #[test]
    fn comment_after_whitespace() {
        // Indented comment should still be skipped.
        let commands = parse("    # indented comment\n");
        assert!(commands.is_empty());
    }

    #[test]
    fn mixed_edge_cases() {
        let input = "\n\
            !\n\
            ?\n\
            #comment\n\
            \n\
            cargo test\n\
            \t\n";
        let commands = parse(input);
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0].prefix, Prefix::FailFast);
        assert_eq!(commands[0].command, "");
        assert_eq!(commands[1].prefix, Prefix::Advisory);
        assert_eq!(commands[1].command, "");
        assert_eq!(commands[2].prefix, Prefix::Default);
        assert_eq!(commands[2].command, "cargo test");
    }
}
