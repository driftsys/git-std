use clap::{Parser, Subcommand};

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
    Check,
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

    let name = match cli.command {
        Command::Commit => "commit",
        Command::Check => "check",
        Command::Bump => "bump",
        Command::Changelog => "changelog",
        Command::Hooks => "hooks",
        Command::SelfUpdate => "self-update",
    };

    eprintln!("git-std {name}: not yet implemented");
    std::process::exit(1);
}
