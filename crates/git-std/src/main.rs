use clap::{Parser, Subcommand};

mod check;
mod commit;
mod config;

/// Standard git workflow — commits, versioning, hooks.
#[derive(Parser)]
#[command(name = "git-std", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
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
    },
    /// Version bump, changelog, commit, and tag.
    Bump,
    /// Generate a changelog.
    Changelog,
    /// Git hooks management.
    Hooks,
    /// Update git-std to the latest version.
    SelfUpdate,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Check {
            message,
            file,
            range,
            strict,
        } => {
            let project_config = config::load(&std::env::current_dir().unwrap_or_default());
            let effective_strict = strict || project_config.strict;
            let lint_config = project_config.to_lint_config(strict);
            let lint_ref = if effective_strict {
                Some(&lint_config)
            } else {
                None
            };

            let code = if let Some(path) = file {
                check::run_file(&path, lint_ref)
            } else if let Some(range) = range {
                check::run_range(&range, lint_ref)
            } else if let Some(message) = message {
                check::run(&message, lint_ref)
            } else {
                eprintln!("error: provide a message, --file, or --range");
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
            let opts = commit::CommitOptions {
                commit_type,
                scope,
                message,
                breaking,
                dry_run,
                amend,
                sign,
                all,
            };
            let code = commit::run_interactive(&project_config, &opts);
            std::process::exit(code);
        }
        other => {
            let name = match other {
                Command::Commit { .. } => unreachable!(),
                Command::Check { .. } => unreachable!(),
                Command::Bump => "bump",
                Command::Changelog => "changelog",
                Command::Hooks => "hooks",
                Command::SelfUpdate => "self-update",
            };
            eprintln!("git-std {name}: not yet implemented");
            std::process::exit(1);
        }
    }
}
