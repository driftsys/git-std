use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

/// Output format for subcommands that support structured output.
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable text (default).
    Text,
    /// Machine-readable JSON.
    Json,
}

/// When to enable coloured output.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ColorWhen {
    /// Colour if stdout is a TTY.
    Auto,
    /// Always use colour.
    Always,
    /// Never use colour.
    Never,
}

/// Standard git workflow — commits, versioning, hooks.
#[derive(Parser)]
#[command(name = "git-std", about)]
pub struct Cli {
    /// When to use coloured output.
    #[arg(long, global = true, default_value = "auto")]
    pub color: ColorWhen,

    /// Generate shell completion scripts and print to stdout.
    #[arg(long, value_name = "SHELL")]
    pub completions: Option<Shell>,

    /// Dump project context as Markdown for agent consumption.
    #[arg(long)]
    pub context: bool,

    /// Update git-std to the latest release.
    #[arg(long)]
    pub update: bool,

    /// Output format for --context.
    #[arg(long, default_value = "text")]
    pub format: OutputFormat,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Available subcommands.
#[derive(Subcommand)]
pub enum Command {
    /// Interactive conventional commit builder.
    Commit {
        /// Commit type (e.g. feat, fix, chore).
        #[arg(long = "type")]
        commit_type: Option<String>,
        /// Commit scope.
        #[arg(long)]
        scope: Option<String>,
        /// Commit description (subject line). Skips all prompts when combined with --type.
        #[arg(short = 'm', long)]
        message: Option<String>,
        /// Commit body paragraph (extended description).
        #[arg(short = 'b', long)]
        body: Option<String>,
        /// Breaking change description.
        #[arg(long)]
        breaking: Option<String>,
        /// Print the formatted message without committing.
        #[arg(long)]
        dry_run: bool,
        /// Amend the previous commit instead of creating a new one.
        #[arg(long)]
        amend: bool,
        /// GPG-sign the commit.
        #[arg(short = 'S', long)]
        sign: bool,
        /// Stage all tracked modified files before committing.
        #[arg(short = 'a', long)]
        all: bool,
        /// Add a trailer footer to the commit message (repeatable).
        #[arg(long)]
        footer: Vec<String>,
        /// Add a `Signed-off-by` trailer using git user.name and user.email.
        #[arg(short = 's', long)]
        signoff: bool,
    },
    /// Validate commit messages.
    Lint {
        /// Commit message to validate (inline).
        message: Option<String>,
        /// Read commit message from a file (strips `#` comment lines).
        #[arg(long, short, conflicts_with = "message", conflicts_with = "range")]
        file: Option<std::path::PathBuf>,
        /// Validate all commits in a git revision range.
        #[arg(long, short, conflicts_with = "message", conflicts_with = "file")]
        range: Option<String>,
        /// Reject types/scopes not in `.git-std.toml` and require scope if scopes are defined.
        #[arg(long)]
        strict: bool,
        /// Output format.
        #[arg(long, default_value = "text")]
        format: OutputFormat,
    },
    /// Version bump, changelog, commit, and tag.
    Bump {
        /// Print the full plan without writing anything.
        #[arg(long)]
        dry_run: bool,
        /// Bump as pre-release (e.g. `2.0.0-rc.1`). Uses default tag from config if no value given.
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
        prerelease: Option<String>,
        /// Force a specific version, skip calculation.
        #[arg(long)]
        release_as: Option<String>,
        /// Use current version for initial changelog (no bump).
        #[arg(long)]
        first_release: bool,
        /// Skip tag creation.
        #[arg(long)]
        no_tag: bool,
        /// Update files only, no commit or tag.
        #[arg(long)]
        no_commit: bool,
        /// Skip changelog generation.
        #[arg(long)]
        skip_changelog: bool,
        /// GPG-sign the release commit and annotated tag.
        #[arg(short = 'S', long)]
        sign: bool,
        /// Allow breaking changes in patch-only scheme.
        #[arg(long)]
        force: bool,
        /// Create a stable branch for patch-only releases. Optionally specify a custom branch name.
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
        stable: Option<String>,
        /// Use minor bump (instead of major) when advancing main after --stable.
        #[arg(long)]
        minor: bool,
        /// Output format.
        #[arg(long, default_value = "text")]
        format: OutputFormat,
        /// Filter bump to specific package(s) (monorepo only).
        #[arg(short = 'p', long = "package")]
        packages: Vec<String>,
        /// Push commit and tags to remote after release. Optionally specify a remote name (default: origin).
        /// Skipped (with a warning) when --no-commit or --no-tag is set.
        #[arg(long, num_args = 0..=1, default_missing_value = "origin", value_name = "REMOTE")]
        push: Option<String>,
        /// Skip branch confirmation prompt (also: GIT_STD_YES=1).
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Generate a changelog (incremental by default, --full to regenerate).
    Changelog {
        /// Regenerate the entire changelog from the first commit.
        #[arg(long)]
        full: bool,
        /// Write to file instead of stdout. Optionally specify a path (default: CHANGELOG.md).
        #[arg(short = 'w', long, num_args = 0..=1, default_missing_value = "CHANGELOG.md")]
        write: Option<String>,
        /// Git revision range (e.g. `v1.0.0..v2.0.0`).
        #[arg(long)]
        range: Option<String>,
        /// Generate changelog for a specific package (monorepo only).
        #[arg(short = 'p', long = "package")]
        package: Option<String>,
    },
    /// Post-clone environment setup.
    Bootstrap {
        /// Print what would be done without executing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Scaffold hooks, bootstrap script, and README section in one step.
    Init {
        /// Overwrite existing files.
        #[arg(long)]
        force: bool,
        /// Update skill files and merge config defaults without overwriting hooks.
        #[arg(long)]
        refresh: bool,
    },
    /// Git hooks management.
    ///
    /// Git hooks (triggered by git):
    ///
    ///   pre-commit          runs before a commit is created
    ///   commit-msg          validates the commit message ($1 = message file)
    ///   pre-push            runs before pushing to a remote
    ///   post-commit         runs after a commit is created (informational)
    ///   prepare-commit-msg  runs before the commit message editor opens
    ///   post-merge          runs after a successful merge or pull
    ///
    /// Bootstrap hook (triggered by `git std bootstrap`):
    ///
    ///   bootstrap           runs after built-in post-clone checks
    ///
    /// Bump lifecycle hooks (triggered by `git std bump`):
    ///
    ///   pre-bump            gate before version detection — abort to cancel
    ///   post-version        runs after version files are updated ($1 = new version)
    ///   post-changelog      runs after CHANGELOG.md is written
    ///   post-bump           runs after commit + tag (use for publish, notify)
    ///
    /// Hook commands are defined in `.githooks/<hook>.hooks` — one command per
    /// line. Each line may start with an optional sigil:
    ///
    ///   !  required   — run the command; abort the hook on non-zero exit
    ///   ~  fix        — stash unstaged changes, run, re-stage result, restore
    ///   ?  advisory   — run the command; warn on failure, never abort
    ///
    /// Lines without a sigil use the hook's default mode (fail-fast for most
    /// git hooks, advisory for bootstrap).
    ///
    /// A glob pattern at the end of a line restricts the command to matching
    /// files only (populated as $@ when the hook is invoked by git):
    ///
    ///   ~ cargo fmt -- $@   *.rs
    ///   ! cargo clippy
    ///
    /// Examples (.githooks/pre-commit.hooks):
    ///
    ///   ~ cargo fmt -- $@
    ///   ~ npx prettier --write $@   *.{js,ts,json}
    ///   ! cargo clippy --workspace -- -D warnings
    ///
    /// Examples (.githooks/pre-push.hooks):
    ///
    ///   ! cargo test --workspace
    ///   ! npx markdownlint "**/*.md"
    ///
    /// Examples (.githooks/commit-msg.hooks):
    ///
    ///   ! git std lint -f $1
    Hook {
        #[command(subcommand)]
        subcommand: HookCommand,
    },
    /// Run health checks on the local git-std setup.
    Doctor {
        /// Output format.
        #[arg(long, default_value = "text")]
        format: OutputFormat,
    },
    /// Query the current project version.
    Version {
        /// Print cargo-style describe: version with distance + hash + dirty flag.
        #[arg(long)]
        describe: bool,
        /// Compute and print the next version from conventional commits.
        #[arg(long)]
        next: bool,
        /// Print the bump label (major/minor/patch).
        #[arg(long)]
        label: bool,
        /// Print the version code integer.
        #[arg(long)]
        code: bool,
        /// Output format.
        #[arg(long, default_value = "text")]
        format: OutputFormat,
    },
}

/// Hook subcommands.
#[derive(Subcommand)]
pub enum HookCommand {
    /// Execute all commands in a hook file.
    Run {
        /// Hook name (e.g. pre-commit, commit-msg, pre-push).
        hook: String,
        /// Arguments passed through to hook commands (after `--`).
        #[arg(last = true)]
        args: Vec<String>,
        /// Output format.
        #[arg(long, default_value = "text")]
        format: OutputFormat,
    },
    /// Display all configured hooks and their commands.
    List {
        /// Output format.
        #[arg(long, default_value = "text")]
        format: OutputFormat,
    },
    /// Enable a hook (activate its shim).
    Enable {
        /// Hook name (e.g. pre-commit, commit-msg).
        hook: String,
    },
    /// Disable a hook (deactivate its shim).
    Disable {
        /// Hook name (e.g. pre-commit, commit-msg).
        hook: String,
    },
}
