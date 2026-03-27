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
            ui::blank();
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
