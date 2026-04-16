use std::io::IsTerminal;

use clap::{CommandFactory, Parser};

pub mod app;
mod cli;
mod config;
pub mod ecosystem;
mod git;
pub mod ui;

use app::*;

fn main() {
    // Handle hidden background update-check mode before clap parsing.
    if std::env::args().any(|a| a == "--update-check-bg") {
        cli::update_check::run_background_check();
        return;
    }

    // Handle --update before clap so it works as a global flag.
    if std::env::args().any(|a| a == "--update") {
        std::process::exit(cli::update_check::run_self_update());
    }

    // Handle --version / -V before clap so we can append the update hint to stderr.
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("git-std {}", env!("CARGO_PKG_VERSION"));
        cli::update_check::print_update_hint();
        return;
    }

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

    // Handle --completions before subcommand dispatch so it works without a subcommand.
    if let Some(shell) = cli.completions {
        let mut cmd = Cli::command();
        clap_complete::generate(shell, &mut cmd, "git-std", &mut std::io::stdout());
        print!("{}", cli::completions::git_subcommand_wrapper(shell));
        return;
    }

    // Handle --context before subcommand dispatch so it works without a subcommand.
    if cli.context {
        let cwd = std::env::current_dir().unwrap_or_default();
        let code = cli::context::run(&cwd, cli.format);
        std::process::exit(code);
    }

    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            // No subcommand and no --completions: print help.
            let mut cmd = Cli::command();
            cmd.print_help().ok();
            println!();
            std::process::exit(2);
        }
    };

    cli::update_check::maybe_spawn_background_check();

    let code = match command {
        Command::Lint {
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

            if let Some(path) = file {
                cli::lint::run_file(&path, lint_ref, format)
            } else if let Some(range) = range {
                cli::lint::run_range(&range, lint_ref, format)
            } else if let Some(message) = message {
                cli::lint::run(&message, lint_ref, format)
            } else {
                ui::error("no input provided");
                ui::info("usage: git std lint <message>");
                ui::info("       git std lint --file <path>");
                ui::info("       git std lint --range <from..to>");
                2
            }
        }
        Command::Commit {
            commit_type,
            scope,
            message,
            body,
            breaking,
            dry_run,
            amend,
            sign,
            all,
            footer,
            signoff,
        } => {
            let project_config = config::load(&std::env::current_dir().unwrap_or_default());
            let opts = cli::commit::CommitOptions {
                commit_type,
                scope,
                message,
                body,
                breaking,
                dry_run,
                amend,
                sign,
                all,
                footer,
                signoff,
            };
            cli::commit::run_interactive(&project_config, &opts)
        }
        Command::Changelog {
            full,
            write,
            range,
            package,
        } => {
            let project_config = config::load(&std::env::current_dir().unwrap_or_default());
            let changelog_config = project_config.to_changelog_config();
            let opts = cli::changelog::ChangelogOptions {
                full,
                write,
                range,
                package,
                monorepo: project_config.monorepo,
                tag_template: project_config.versioning.tag_template.clone(),
                tag_prefix: project_config.versioning.tag_prefix.clone(),
            };
            cli::changelog::run(&project_config, &changelog_config, &opts)
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
            packages,
            push,
            yes,
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
                packages,
                push,
                yes,
            };
            cli::bump::run(&project_config, &opts)
        }
        Command::Init { force, refresh } => cli::init::run(force, refresh),
        Command::Bootstrap { dry_run } => cli::bootstrap::run(dry_run),
        Command::Hook { subcommand } => match subcommand {
            HookCommand::Run { hook, args, format } => cli::hook::run(&hook, &args, format),
            HookCommand::List { format } => cli::hook::list(format),
            HookCommand::Enable { hook } => cli::hook::enable(&hook),
            HookCommand::Disable { hook } => cli::hook::disable(&hook),
        },
        Command::Doctor { format } => {
            let cwd = std::env::current_dir().unwrap_or_default();
            cli::doctor::run(&cwd, format)
        }
        Command::Version {
            describe,
            next,
            label,
            code,
            format,
        } => {
            let project_config = config::load(&std::env::current_dir().unwrap_or_default());
            let opts = cli::version::VersionOptions {
                describe,
                next,
                label,
                code,
                format,
            };
            cli::version::run(&project_config, &opts)
        }
    };

    cli::update_check::print_update_hint();
    std::process::exit(code);
}
