use std::io::IsTerminal;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

mod cli;
mod config;
mod git;
pub mod ui;

/// Standard git workflow — commits, versioning, hooks.
#[derive(Parser)]
#[command(name = "git-std", version, about)]
struct Cli {
    /// When to use coloured output.
    #[arg(long, global = true, default_value = "auto")]
    color: ColorWhen,

    #[command(subcommand)]
    command: Command,
}

/// When to enable coloured output.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum ColorWhen {
    /// Colour if stdout is a TTY.
    Auto,
    /// Always use colour.
    Always,
    /// Never use colour.
    Never,
}

/// Available subcommands.
#[derive(Subcommand)]
enum Command {
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
    },
    /// Validate commit messages.
    Check {
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
        format: cli::check::OutputFormat,
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
    },
    /// Generate a changelog (incremental by default, --full to regenerate).
    Changelog {
        /// Regenerate the entire changelog from the first commit.
        #[arg(long)]
        full: bool,
        /// Print to stdout instead of writing to a file.
        #[arg(long)]
        stdout: bool,
        /// Output file path.
        #[arg(long, default_value = "CHANGELOG.md")]
        output: String,
        /// Git revision range (e.g. `v1.0.0..v2.0.0`).
        #[arg(long)]
        range: Option<String>,
    },
    /// Post-clone environment setup.
    Bootstrap {
        #[command(subcommand)]
        subcommand: Option<BootstrapCommand>,
        /// Print what would be done without executing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Git hooks management.
    Hooks {
        #[command(subcommand)]
        subcommand: HooksCommand,
    },
    /// Inspect effective git-std configuration.
    Config {
        #[command(subcommand)]
        subcommand: ConfigCommand,
    },
    /// Generate shell completion scripts.
    Completions {
        /// Target shell.
        shell: Shell,
    },
}

/// Bootstrap subcommands.
#[derive(Subcommand)]
enum BootstrapCommand {
    /// Generate bootstrap script and hooks template.
    Install {
        /// Overwrite existing files.
        #[arg(long)]
        force: bool,
    },
}

/// Config subcommands.
#[derive(Subcommand)]
enum ConfigCommand {
    /// List all effective configuration values with their sources.
    List {
        /// Output format.
        #[arg(long, default_value = "text")]
        format: cli::config::OutputFormat,
    },
    /// Get a single configuration value.
    Get {
        /// Dot-separated key (e.g. versioning.tag_prefix).
        key: String,
        /// Output format.
        #[arg(long, default_value = "text")]
        format: cli::config::OutputFormat,
    },
}

/// Hooks subcommands.
#[derive(Subcommand)]
enum HooksCommand {
    /// Set up hooks directory, shims, and core.hooksPath.
    Install,
    /// Execute all commands in a hook file.
    Run {
        /// Hook name (e.g. pre-commit, commit-msg, pre-push).
        hook: String,
        /// Arguments passed through to hook commands (after `--`).
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Display all configured hooks and their commands.
    List,
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

fn main() {
    let cli = Cli::parse();

    // Configure yansi colour output based on --color flag.
    match cli.color {
        ColorWhen::Always => yansi::enable(),
        ColorWhen::Never => yansi::disable(),
        ColorWhen::Auto => {
            if !std::io::stdout().is_terminal() {
                yansi::disable();
            }
        }
    }

    match cli.command {
        Command::Check {
            message,
            file,
            range,
            strict,
            format,
        } => {
            let cwd = std::env::current_dir().unwrap_or_default();
            let project_config = config::load(&cwd);
            let effective_strict = strict || project_config.strict;
            let lint_config = project_config.to_lint_config(strict, &cwd);
            let lint_ref = if effective_strict {
                Some(&lint_config)
            } else {
                None
            };

            let code = if let Some(path) = file {
                cli::check::run_file(&path, lint_ref, format)
            } else if let Some(range) = range {
                cli::check::run_range(&range, lint_ref, format)
            } else if let Some(message) = message {
                cli::check::run(&message, lint_ref, format)
            } else {
                ui::error("no input provided");
                ui::info("usage: git std check <message>");
                ui::info("       git std check --file <path>");
                ui::info("       git std check --range <from..to>");
                2
            };
            std::process::exit(code);
        }
        Command::Commit {
            commit_type,
            scope,
            message,
            breaking,
            dry_run,
            amend,
            sign,
            all,
        } => {
            let project_config = config::load(&std::env::current_dir().unwrap_or_default());
            let opts = cli::commit::CommitOptions {
                commit_type,
                scope,
                message,
                breaking,
                dry_run,
                amend,
                sign,
                all,
            };
            let code = cli::commit::run_interactive(&project_config, &opts);
            std::process::exit(code);
        }
        Command::Changelog {
            full,
            stdout,
            output,
            range,
        } => {
            let project_config = config::load(&std::env::current_dir().unwrap_or_default());
            let changelog_config = project_config.to_changelog_config();
            let opts = cli::changelog::ChangelogOptions {
                full,
                stdout,
                output,
                range,
            };
            let code = cli::changelog::run(&changelog_config, &opts);
            std::process::exit(code);
        }
        Command::Bump {
            dry_run,
            prerelease,
            release_as,
            first_release,
            no_tag,
            no_commit,
            skip_changelog,
            sign,
            force,
            stable,
            minor,
        } => {
            let project_config = config::load(&std::env::current_dir().unwrap_or_default());
            let stable = stable.map(|s| if s.is_empty() { None } else { Some(s) });
            let opts = cli::bump::BumpOptions {
                dry_run,
                prerelease,
                release_as,
                first_release,
                no_tag,
                no_commit,
                skip_changelog,
                sign,
                force,
                stable,
                minor,
            };
            let code = cli::bump::run(&project_config, &opts);
            std::process::exit(code);
        }
        Command::Bootstrap {
            subcommand,
            dry_run,
        } => {
            let code = match subcommand {
                Some(BootstrapCommand::Install { force }) => cli::bootstrap::install(force),
                None => cli::bootstrap::run(dry_run),
            };
            std::process::exit(code);
        }
        Command::Hooks { subcommand } => {
            let code = match subcommand {
                HooksCommand::Install => cli::hooks::install(),
                HooksCommand::Run { hook, args } => cli::hooks::run(&hook, &args),
                HooksCommand::List => cli::hooks::list(),
                HooksCommand::Enable { hook } => cli::hooks::enable(&hook),
                HooksCommand::Disable { hook } => cli::hooks::disable(&hook),
            };
            std::process::exit(code);
        }
        Command::Config { subcommand } => {
            let cwd = std::env::current_dir().unwrap_or_default();
            let code = match subcommand {
                ConfigCommand::List { format } => cli::config::list(&cwd, format),
                ConfigCommand::Get { key, format } => cli::config::get(&cwd, &key, format),
            };
            std::process::exit(code);
        }
        Command::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "git-std", &mut std::io::stdout());
        }
    }
}
