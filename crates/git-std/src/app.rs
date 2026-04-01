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
#[command(name = "git-std", version, about)]
pub struct Cli {
    /// When to use coloured output.
    #[arg(long, global = true, default_value = "auto")]
    pub color: ColorWhen,

    /// Generate shell completion scripts and print to stdout.
    #[arg(long, value_name = "SHELL")]
    pub completions: Option<Shell>,

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
    },
    /// Git hooks management.
    Hook {
        #[command(subcommand)]
        subcommand: HookCommand,
    },
    /// Inspect effective git-std configuration.
    Config {
        #[command(subcommand)]
        subcommand: ConfigCommand,
    },
    /// Run health checks on the local git-std setup.
    Doctor {
        /// Output format.
        #[arg(long, default_value = "text")]
        format: OutputFormat,
    },
}

/// Config subcommands.
#[derive(Subcommand)]
pub enum ConfigCommand {
    /// List all effective configuration values with their sources.
    List {
        /// Output format.
        #[arg(long, default_value = "text")]
        format: OutputFormat,
    },
    /// Get a single configuration value.
    Get {
        /// Dot-separated key (e.g. versioning.tag_prefix).
        key: String,
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
