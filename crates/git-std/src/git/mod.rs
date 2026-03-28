//! Git operations implemented via `git` CLI subprocess calls.
//!
//! This module replaces the previous `git2`-based implementation with direct
//! calls to the `git` binary, removing the C dependency on libgit2.

pub(crate) mod cmd;
mod mutate;
mod query;

pub use mutate::{
    amend_commit, branch_exists, checkout_branch, create_annotated_tag, create_branch,
    create_commit, create_signed_commit, create_signed_commit_amend, create_signed_tag,
    is_working_tree_dirty, stage_files, stage_tracked_modified, workdir,
};
pub use query::{
    collect_tags, commit_date, current_branch, detect_host, find_latest_calver_tag,
    find_latest_version_tag, head_oid, resolve_rev, walk_commits, walk_commits_for_path,
    walk_range,
};
