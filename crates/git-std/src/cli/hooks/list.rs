use yansi::Paint;

use standard_githooks::{HookCommand, HookMode, KNOWN_HOOKS, Prefix, default_mode};

use crate::ui;

use super::{is_enabled, read_and_parse_hooks};

/// Run the `hooks list` subcommand. Returns the process exit code.
///
/// Shows all known hooks with enabled/disabled status and their commands.
pub fn list() -> i32 {
    let hooks_dir = std::path::Path::new(".githooks");

    if !hooks_dir.exists() {
        ui::info("no hooks installed — run 'git std hooks install'");
        return 0;
    }

    for (i, hook_name) in KNOWN_HOOKS.iter().enumerate() {
        if i > 0 {
            println!();
        }

        let enabled = is_enabled(hooks_dir, hook_name);
        let status_label = if enabled {
            "enabled".green().to_string()
        } else {
            "disabled".dim().to_string()
        };

        let template_path = hooks_dir.join(format!("{hook_name}.hooks"));
        let commands: Vec<HookCommand> = if template_path.exists() {
            read_and_parse_hooks(hook_name).unwrap_or_default()
        } else {
            vec![]
        };

        let mode = default_mode(hook_name);
        let mode_label = match mode {
            HookMode::Collect => "collect",
            HookMode::FailFast => "fail-fast",
        };

        println!(
            "{INDENT}{hook_name} ({mode_label}) [{status_label}]:",
            INDENT = ui::INDENT
        );

        if commands.is_empty() {
            println!("{INDENT}  (no commands)", INDENT = ui::INDENT);
        } else {
            for cmd in &commands {
                let prefix_char = match cmd.prefix {
                    Prefix::FailFast => "!",
                    Prefix::Advisory => "?",
                    Prefix::Fix => "~",
                    Prefix::Default => " ",
                };

                let display = if let Some(ref glob) = cmd.glob {
                    let cmd_part = format!("  {prefix_char} {}", cmd.command);
                    let total_width = 50;
                    let padding = if cmd_part.len() < total_width {
                        total_width - cmd_part.len()
                    } else {
                        4
                    };
                    format!(
                        "  {prefix_char} {}{:width$}{glob}",
                        cmd.command,
                        "",
                        width = padding
                    )
                } else {
                    format!("  {prefix_char} {}", cmd.command)
                };

                println!("{INDENT}{display}", INDENT = ui::INDENT);
            }
        }
    }

    0
}
