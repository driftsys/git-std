mod apply;
pub(crate) mod detect;
mod lifecycle;
pub(crate) mod monorepo;
mod plan;
mod stable;

use std::io::IsTerminal;

use crate::app::OutputFormat;
use crate::{git, ui};

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
    /// Output format (text or json).
    pub format: OutputFormat,
    /// Filter bump to specific package(s) (monorepo only).
    pub packages: Vec<String>,
    /// Push commit and tags to the given remote after tagging.
    ///
    /// `None` = flag not used, `Some(name)` = push to the named remote.
    pub push: Option<String>,
    /// Skip the branch confirmation prompt (`--yes` / `-y` / `GIT_STD_YES=1`).
    pub yes: bool,
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
    // Branch guard: warn and prompt when bumping on a non-release branch.
    // Skipped for --dry-run (read-only), --yes, and GIT_STD_YES=1.
    if !opts.dry_run && !opts.yes {
        let env_yes = std::env::var("GIT_STD_YES")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        if !env_yes {
            let cwd = std::env::current_dir().unwrap_or_default();
            if let Ok(branch) = git::current_branch(&cwd) {
                // Detached HEAD → skip the check.
                if branch != "HEAD" {
                    let release = config.release_branch.as_deref().unwrap_or("main");
                    let on_release = branch == release
                        || (config.release_branch.is_none() && branch == "master");

                    if !on_release {
                        ui::warning(&format!("you are on branch '{branch}', not '{release}'"));
                        ui::warning(
                            "bumping here will create a version commit and tag on this branch",
                        );

                        if !std::io::stdin().is_terminal() {
                            ui::hint("use --yes / -y to bypass, or set GIT_STD_YES=1");
                            return 1;
                        }

                        match inquire::Confirm::new("Continue?")
                            .with_default(false)
                            .prompt()
                        {
                            Ok(true) => {}
                            _ => {
                                ui::error("bump cancelled");
                                return 1;
                            }
                        }
                    }
                }
            }
        }
    }

    // pre-bump gate: runs before version detection, non-zero exit aborts bump.
    // Skipped for --dry-run.
    if !opts.dry_run
        && let Err(code) = lifecycle::run_lifecycle_hook("pre-bump", &[])
    {
        return code;
    }

    if opts.stable.is_some() {
        return stable::run_stable(config, opts);
    }
    if config.monorepo {
        return monorepo::plan_monorepo_bump(config, opts, &opts.packages);
    }
    plan::dispatch(config, opts)
}
