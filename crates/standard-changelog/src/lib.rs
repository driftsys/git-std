//! Changelog generation from conventional commits.
//!
//! Groups parsed commits by type, renders markdown sections, and manages
//! `CHANGELOG.md` files. Pure library — no I/O, no git operations, no
//! terminal output.
//!
//! # Main entry points
//!
//! - [`build_release`] — parse raw commits into a [`VersionRelease`]
//! - [`render`] — render multiple releases into a full `CHANGELOG.md`
//! - [`render_version`] — render a single version section
//! - [`prepend_release`] — splice a new release into an existing changelog
//!
//! # Example
//!
//! ```
//! use standard_changelog::{build_release, render, ChangelogConfig, RepoHost};
//!
//! let commits = vec![
//!     ("abc1234", "feat(auth): add OAuth2 PKCE flow"),
//!     ("def5678", "fix: handle expired tokens"),
//! ];
//!
//! let config = ChangelogConfig::default();
//! let mut release = build_release(&commits, "1.0.0", None, &config).unwrap();
//! release.date = "2026-03-14".to_string();
//!
//! let host = RepoHost::Unknown;
//! let changelog = render(&[release], &config, &host);
//! assert!(changelog.contains("## 1.0.0 (2026-03-14)"));
//! assert!(changelog.contains("### Features"));
//! assert!(changelog.contains("### Bug Fixes"));
//! ```

mod build;
mod date;
mod host;
mod link;
mod model;
mod render;
pub use build::build_release;
pub use date::{days_to_date, format_date};
pub use host::detect_host;
pub use model::*;
pub use render::{prepend_release, render, render_version};
