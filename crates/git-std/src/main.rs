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

    match command {
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

            let code = if let Some(path) = file {
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
            footer,
            signoff,
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
                footer,
                signoff,
            };
            let code = cli::commit::run_interactive(&project_config, &opts);
            std::process::exit(code);
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
            let code = cli::changelog::run(&project_config, &changelog_config, &opts);
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
            packages,
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
        Command::Hook { subcommand } => {
            let code = match subcommand {
                HookCommand::Install => cli::hook::install(),
                HookCommand::Run { hook, args, format } => cli::hook::run(&hook, &args, format),
                HookCommand::List { format } => cli::hook::list(format),
                HookCommand::Enable { hook } => cli::hook::enable(&hook),
                HookCommand::Disable { hook } => cli::hook::disable(&hook),
            };
            std::process::exit(code);
        }
        Command::Config { subcommand } => {
            let cwd = std::env::current_dir().unwrap_or_default();
            // Try CWD first; fall back to repo root so config is found
            // from subdirectories. If neither has the file, CWD is used
            // (defaults apply).
            let dir = if cwd.join(".git-std.toml").exists() {
                cwd.clone()
            } else {
                git::workdir(&cwd).unwrap_or(cwd)
            };
            let code = match subcommand {
                ConfigCommand::List { format } => cli::config::list(&dir, format),
                ConfigCommand::Get { key, format } => cli::config::get(&dir, &key, format),
            };
            std::process::exit(code);
        }
        Command::Doctor { format } => {
            let cwd = std::env::current_dir().unwrap_or_default();
            std::process::exit(cli::doctor::run(&cwd, format));
        }
    }
}
