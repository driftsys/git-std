//! Git hooks file format parsing, shim generation, and execution model.
//!
//! Owns the `.githooks/<hook>.hooks` file format. Can read/write hook files
//! and generate shim scripts. Does not execute commands, run git operations,
//! or produce terminal output.
//!
//! # Main entry point
//!
//! - [`parse`] — parse the text content of a `.hooks` file into a list of
//!   [`HookCommand`]s
//!
//! # Example
//!
//! ```
//! use standard_githooks::{parse, HookCommand, Prefix};
//!
//! let commands = parse("!cargo test *.rs\n? detekt *.kt\n");
//! assert_eq!(commands.len(), 2);
//! assert_eq!(commands[0].prefix, Prefix::FailFast);
//! assert_eq!(commands[0].command, "cargo test");
//! assert_eq!(commands[0].glob, Some("*.rs".to_string()));
//! ```

mod parse;

pub use parse::{HookCommand, Prefix, parse};
