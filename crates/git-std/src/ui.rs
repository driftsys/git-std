//! Shared UI helpers for consistent CLI output.
//!
//! All human-readable output goes to stderr. Symbols use
//! yansi for colour when enabled.

use std::io::{IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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
    eprintln!(
        "{DETAIL_INDENT}{:<width$} {value}",
        label,
        width = LABEL_WIDTH
    );
}

/// Print an indented informational line (two-space indent).
pub fn info(msg: &str) {
    eprintln!("{INDENT}{msg}");
}

/// Print a detail-indented line (four-space indent).
pub fn detail(msg: &str) {
    eprintln!("{DETAIL_INDENT}{msg}");
}

/// Print a hint line (two-space indent, no prefix).
pub fn hint(msg: &str) {
    eprintln!("{INDENT}hint: {msg}");
}

/// Print a check-line (four-space indent, symbol, two-space gap, label).
pub fn check_line(symbol: &str, label: &str) {
    eprintln!("{DETAIL_INDENT}{symbol}  {label}");
}

/// Print a summary count line (`valid_count/total valid`).
pub fn summary_counts(valid: usize, total: usize) {
    eprintln!("{valid}/{total} valid");
}

/// Print a plain message to stderr with no indentation.
pub fn print(msg: &str) {
    eprintln!("{msg}");
}

/// Return `true` when stderr is connected to a terminal.
pub fn is_tty() -> bool {
    std::io::stderr().is_terminal()
}

/// Print a pending line for a hook command before it starts executing.
///
/// On TTY: prints `  [index+1/total] > display` with no trailing newline,
/// so the caller can overwrite it with `\r\x1b[K` when the command completes.
///
/// Non-TTY: prints `  > display` followed by a newline (no position tracking).
pub fn pending(index: usize, total: usize, display: &str) {
    if is_tty() {
        eprint!("{INDENT}[{}/{}] > {display}", index + 1, total);
    } else {
        eprintln!("{INDENT}> {display}");
    }
}

/// Print a simple pending line for non-TTY without a counter.
pub fn pending_non_tty(display: &str) {
    eprintln!("{INDENT}> {display}");
}

/// Run `f` while animating a spinner on TTY for `display`.
///
/// On TTY: shows `  ⠋ display` with an animated braille spinner, clears
/// the line with `\r\x1b[K` when `f` returns, ready for the caller to
/// print the result line.
///
/// On non-TTY or when colour is disabled: prints `  > display\n` once
/// (no animation) and runs `f` normally.
pub fn spin_while<F, T>(display: &str, f: F) -> T
where
    F: FnOnce() -> T,
{
    if !is_tty() || !yansi::is_enabled() {
        eprintln!("{INDENT}> {display}");
        return f();
    }

    const FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    let running = Arc::new(AtomicBool::new(true));
    let stop = Arc::clone(&running);
    let label = display.to_string();

    let handle = std::thread::spawn(move || {
        let mut i = 0usize;
        while stop.load(Ordering::Relaxed) {
            eprint!("\r{INDENT}{} {label}", FRAMES[i % FRAMES.len()]);
            let _ = std::io::stderr().flush();
            std::thread::sleep(std::time::Duration::from_millis(80));
            i += 1;
        }
    });

    let result = f();

    running.store(false, Ordering::Relaxed);
    let _ = handle.join();
    eprint!("\r\x1b[K"); // clear spinner line before caller prints result

    result
}
