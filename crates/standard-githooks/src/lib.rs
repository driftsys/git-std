//! Git hooks file format parsing, shim generation, and execution model.
//!
//! Owns the `.githooks/<hook>.hooks` file format. Can read/write hook files
//! and generate shim scripts. Does not execute commands, run git operations,
//! or produce terminal output.

mod glob;
mod parse;
mod run;
mod shim;

pub use glob::matches_any;
pub use parse::{HookCommand, Prefix, parse};
pub use run::{HookMode, default_mode, substitute_msg};
pub use shim::{KNOWN_HOOKS, generate_hooks_template, generate_shim};
