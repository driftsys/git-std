use std::io::IsTerminal;

use clap::{CommandFactory, Parser};

pub mod app;
mod cli;
mod config;
mod git;
pub mod ui;

use app::*;

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
            format,
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
                format,
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
                HooksCommand::Run { hook, args, format } => cli::hooks::run(&hook, &args, format),
                HooksCommand::List { format } => cli::hooks::list(format),
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
        Command::Doctor { format } => {
            let cwd = std::env::current_dir().unwrap_or_default();
            std::process::exit(cli::doctor::run(&cwd, format));
        }
        Command::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "git-std", &mut std::io::stdout());
        }
    }
}
