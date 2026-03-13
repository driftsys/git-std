//! Git hooks file format parsing, shim generation, and execution model.
//!
//! Owns the `.githooks/<hook>.hooks` file format. Can read/write hook files
//! and generate shim scripts. Does not execute commands, run git operations,
//! or produce terminal output.
