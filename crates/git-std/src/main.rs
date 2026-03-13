use clap::{Parser, Subcommand};

mod check;

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
    Commit,
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
        } => {
            let code = if let Some(path) = file {
                check::run_file(&path)
            } else if let Some(range) = range {
                check::run_range(&range)
            } else if let Some(message) = message {
                check::run(&message)
            } else {
                eprintln!("error: provide a message, --file, or --range");
                2
            };
            std::process::exit(code);
        }
        other => {
            let name = match other {
                Command::Commit => "commit",
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
