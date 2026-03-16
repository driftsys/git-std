//! Shared UI helpers for consistent CLI output.
//!
//! All human-readable output goes to stderr. Symbols use
//! yansi for colour when enabled.

use yansi::Paint;

/// Two-space indent for top-level output sections.
pub const INDENT: &str = "  ";

/// Four-space indent for detail/nested lines.
pub const DETAIL_INDENT: &str = "    ";

/// Column width for left-aligned labels (e.g. file names).
pub const LABEL_WIDTH: usize = 20;

/// Return the pass symbol: green check mark.
pub fn pass() -> yansi::Painted<&'static str> {
    "\u{2713}".green()
}

/// Return the fail symbol: red cross mark.
pub fn fail() -> yansi::Painted<&'static str> {
    "\u{2717}".red()
}

/// Return the warning symbol: yellow warning sign.
pub fn warn() -> yansi::Painted<&'static str> {
    "\u{26a0}".yellow()
}

/// Print a prefixed error message to stderr.
pub fn error(msg: &str) {
    eprintln!("error: {msg}");
}

/// Print a prefixed warning message to stderr.
pub fn warning(msg: &str) {
    eprintln!("warning: {msg}");
}

/// Print an empty line to stderr.
pub fn blank() {
    eprintln!();
}

/// Print a heading line, e.g. `  Analysing commits since v1.0.0...`
pub fn heading(label: &str, value: &str) {
    eprintln!("{INDENT}{label}{value}");
}

/// Print an indented key-value item line.
///
/// The label is left-aligned to [`LABEL_WIDTH`] columns.
pub fn item(label: &str, value: &str) {
    eprintln!("{DETAIL_INDENT}{:<width$} {value}", label, width = LABEL_WIDTH);
}
