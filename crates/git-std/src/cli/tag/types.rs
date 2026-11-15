use crate::app::OutputFormat;

/// Options for tagging an existing release commit.
pub struct TagOptions {
    /// Explicit target commit-ish.
    pub commit: String,
    /// Validate without creating refs.
    pub dry_run: bool,
    /// Selected released packages.
    pub packages: Vec<String>,
    /// Explicit release branch.
    pub branch: Option<String>,
    /// Request signed annotated tags.
    pub sign: bool,
    /// Configured publication remote.
    pub push: Option<String>,
    /// Human or machine output.
    pub format: OutputFormat,
}

use serde::Serialize;

#[derive(Debug)]
pub(super) struct Refusal(pub String);
impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Refusal {}
pub(super) fn refuse(message: impl Into<String>) -> anyhow::Error {
    Refusal(message.into()).into()
}
pub(super) fn require(condition: bool, message: impl Into<String>) -> anyhow::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(refuse(message))
    }
}

#[derive(Clone, Serialize)]
pub(super) struct PlannedTag {
    pub name: String,
    pub version: String,
    pub version_basis: String,
    pub oid: Option<String>,
    pub local: &'static str,
    pub remote: &'static str,
    pub action: &'static str,
    pub signature: &'static str,
}

#[derive(Default, Serialize)]
pub(super) struct TagPlan {
    pub target_oid: String,
    pub release_branch: String,
    pub tags: Vec<PlannedTag>,
    #[serde(skip)]
    pub endpoint: Option<String>,
    pub diagnostics: Vec<String>,
}
