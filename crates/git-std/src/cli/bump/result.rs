use serde::Serialize;

use crate::contract::ContractMetadata;

use super::contract::BumpContract;
use super::version_facts::{VersionMismatch, VersionObservation};

/// JSON description of one version-file update.
#[derive(Serialize)]
pub(super) struct UpdatedFileJson {
    pub(super) path: String,
    pub(super) old_version: String,
    pub(super) new_version: String,
}

/// Versioned JSON output shared by bump planning and apply.
#[derive(Serialize)]
pub(super) struct BumpResultJson {
    #[serde(flatten)]
    pub(super) metadata: ContractMetadata,
    pub(super) status: &'static str,
    #[serde(flatten)]
    pub(super) contract: BumpContract,
    pub(super) version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) previous_version: Option<String>,
    pub(super) tag: Option<String>,
    pub(super) updated_files: Vec<UpdatedFileJson>,
    pub(super) synced_locks: Vec<String>,
    pub(super) changelog: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) pushed_to: Option<String>,
    pub(super) version_observations: Vec<VersionObservation>,
    pub(super) version_mismatches: Vec<VersionMismatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) commit_oid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tag_oid: Option<String>,
    pub(super) dry_run: bool,
}
