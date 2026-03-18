//! Calendar versioning (calver) support.
//!
//! Computes the next calver version from a format string, the current date,
//! and the previous version string. The format string uses tokens like
//! `YYYY`, `MM`, `PATCH`, etc.
//!
//! This module is pure — it takes the date as a parameter and performs no I/O.

pub mod bump;
pub mod parse;

pub use bump::{next_version, validate_format};

/// Date information needed for calver computation.
///
/// All fields are simple integers — the caller is responsible for computing
/// them from the current date. This keeps the library pure (no clock access).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalverDate {
    /// Full year (e.g. 2026).
    pub year: u32,
    /// Month (1–12).
    pub month: u32,
    /// Day of month (1–31).
    pub day: u32,
    /// ISO week number (1–53).
    pub iso_week: u32,
    /// ISO day of week (1=Monday, 7=Sunday).
    pub day_of_week: u32,
}

/// The default calver format when none is specified.
pub const DEFAULT_FORMAT: &str = "YYYY.MM.PATCH";

/// Errors that can occur during calver computation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CalverError {
    /// The format string contains no `PATCH` token.
    #[error("calver format must contain the PATCH token")]
    NoPatchToken,
    /// The format string contains an unrecognised token.
    #[error("unknown calver format token: {0}")]
    UnknownToken(String),
    /// The format string is empty.
    #[error("calver format string is empty")]
    EmptyFormat,
}
