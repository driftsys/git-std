mod apply;
mod detect;
mod plan;
mod stable;

/// Options for the bump subcommand.
pub struct BumpOptions {
    /// Print the plan without writing anything.
    pub dry_run: bool,
    /// Bump as pre-release (e.g. `2.0.0-rc.1`).
    pub prerelease: Option<String>,
    /// Force a specific version, skip calculation.
    pub release_as: Option<String>,
    /// Use current version for initial changelog (no bump).
    pub first_release: bool,
    /// Skip tag creation.
    pub no_tag: bool,
    /// Skip commit and tag (update files only).
    pub no_commit: bool,
    /// Skip changelog generation.
    pub skip_changelog: bool,
    /// GPG-sign the commit and tag.
    pub sign: bool,
    /// Allow breaking changes in patch-only scheme.
    pub force: bool,
    /// Create a stable branch for patch-only releases.
    ///
    /// `None` = flag not used, `Some(None)` = `--stable` without value
    /// (auto-generate branch name), `Some(Some(name))` = custom branch name.
    pub stable: Option<Option<String>>,
    /// Use minor bump instead of major when advancing main after `--stable`.
    pub minor: bool,
}

/// Context passed from the version-computation phase to the shared finalize logic.
pub(super) struct FinalizeContext<'a> {
    /// The new version string (semver or calver).
    pub(super) new_version: String,
    /// The previous version string, if any (used for changelog compare links).
    pub(super) prev_version: Option<&'a str>,
    /// Raw commits since the last tag, used for changelog generation.
    pub(super) raw_commits: &'a [(String, String)],
}

/// Run the bump subcommand. Returns the exit code.
pub fn run(config: &crate::config::ProjectConfig, opts: &BumpOptions) -> i32 {
    if opts.stable.is_some() {
        return stable::run_stable(config, opts);
    }
    plan::dispatch(config, opts)
}
