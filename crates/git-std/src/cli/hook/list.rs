use serde::Serialize;
use yansi::Paint;

use standard_githooks::{HookCommand, HookMode, KNOWN_HOOKS, Prefix, default_mode};

use crate::app::OutputFormat;
use crate::ui;

use super::{is_enabled, read_and_parse_hooks};

/// JSON output schema for a single hook command.
#[derive(Serialize)]
struct HookCommandJson {
    command: String,
    prefix: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    glob: Option<String>,
}

/// JSON output schema for a single hook.
#[derive(Serialize)]
struct HookJson {
    name: String,
    enabled: bool,
    mode: &'static str,
    commands: Vec<HookCommandJson>,
}

fn prefix_label(prefix: Prefix) -> &'static str {
    match prefix {
        Prefix::FailFast => "fail-fast",
        Prefix::Advisory => "advisory",
        Prefix::Fix => "fix",
        Prefix::Default => "default",
    }
}

/// Run the `hook list` subcommand. Returns the process exit code.
///
/// Shows all known hooks with enabled/disabled status and their commands.
pub fn list(format: OutputFormat) -> i32 {
    let hooks_dir = match super::hooks_dir() {
        Ok(d) => d,
        Err(code) => return code,
    };

    if !hooks_dir.exists() {
        if format == OutputFormat::Json {
            println!("[]");
        } else {
            ui::info("no hooks installed — run 'git std init'");
        }
        return 0;
    }

    if format == OutputFormat::Json {
        return list_json(&hooks_dir);
    }

    for (i, hook_name) in KNOWN_HOOKS.iter().enumerate() {
        if i > 0 {
            ui::blank();
        }

        let enabled = is_enabled(&hooks_dir, hook_name);
        let status_label = if enabled {
            "enabled".green().to_string()
        } else {
            "disabled".dim().to_string()
        };

        let template_path = hooks_dir.join(format!("{hook_name}.hooks"));
        let commands: Vec<HookCommand> = if template_path.exists() {
            read_and_parse_hooks(&hooks_dir, hook_name).unwrap_or_default()
        } else {
            vec![]
        };

        let mode = default_mode(hook_name);
        let mode_label = match mode {
            HookMode::Collect => "collect mode",
            HookMode::FailFast => "fail-fast mode",
        };

        ui::info(&format!("{hook_name} ({mode_label}) [{status_label}]:"));

        if commands.is_empty() {
            ui::detail("(no commands)");
        } else {
            // Compute max command width for dynamic glob alignment.
            let max_cmd_width = commands
                .iter()
                .filter(|c| c.glob.is_some())
                .map(|c| c.command.len() + 2) // +2 for prefix char + space
                .max()
                .unwrap_or(0);
            // Minimum column width of 48, with at least 4 chars padding.
            let col_width = max_cmd_width.max(48);

            for cmd in &commands {
                let prefix_char = match cmd.prefix {
                    Prefix::FailFast => "!",
                    Prefix::Advisory => "?",
                    Prefix::Fix => "~",
                    Prefix::Default => " ",
                };

                let display = if let Some(ref glob) = cmd.glob {
                    let cmd_part = format!("{prefix_char} {}", cmd.command);
                    let padding = if cmd_part.len() < col_width {
                        col_width - cmd_part.len()
                    } else {
                        4
                    };
                    format!(
                        "{prefix_char} {}{:width$}{glob}",
                        cmd.command,
                        "",
                        width = padding
                    )
                } else {
                    format!("{prefix_char} {}", cmd.command)
                };

                ui::detail(&display);
            }
        }
    }

    0
}

fn list_json(hooks_dir: &std::path::Path) -> i32 {
    let hooks: Vec<HookJson> = KNOWN_HOOKS
        .iter()
        .map(|hook_name| {
            let enabled = is_enabled(hooks_dir, hook_name);
            let template_path = hooks_dir.join(format!("{hook_name}.hooks"));
            let commands: Vec<HookCommand> = if template_path.exists() {
                read_and_parse_hooks(hooks_dir, hook_name).unwrap_or_default()
            } else {
                vec![]
            };
            let mode = default_mode(hook_name);

            HookJson {
                name: hook_name.to_string(),
                enabled,
                mode: match mode {
                    HookMode::Collect => "collect",
                    HookMode::FailFast => "fail-fast",
                },
                commands: commands
                    .iter()
                    .map(|c| HookCommandJson {
                        command: c.command.clone(),
                        prefix: prefix_label(c.prefix),
                        glob: c.glob.clone(),
                    })
                    .collect(),
            }
        })
        .collect();

    println!("{}", serde_json::to_string(&hooks).unwrap());
    0
}
