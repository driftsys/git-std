use serde::Serialize;

use crate::contract::ContractMetadata;
use crate::ui;

pub(super) fn print_failure_hints(hook: &str) {
    let skip_flag = match hook {
        "pre-commit" | "commit-msg" => "git commit --no-verify",
        "pre-push" => "git push --no-verify",
        _ => &format!(
            "GIT_STD_SKIP_HOOKS=1 git {}",
            hook.trim_start_matches("pre-").trim_start_matches("post-")
        ),
    };
    ui::hint(&format!("to skip this hook:    {skip_flag}"));
    ui::hint("to skip all hooks:    GIT_STD_SKIP_HOOKS=1 git ...");
    ui::hint(&format!(
        "to disable a command: comment it out in .githooks/{hook}.hooks"
    ));
}

pub(super) fn format_display(command_text: &str, glob: Option<&str>) -> String {
    match glob {
        Some(glob) => format!("{command_text} ({glob})"),
        None => command_text.to_string(),
    }
}

#[derive(Clone, Serialize)]
pub(super) struct CommandExecutionJson {
    pub(super) command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) glob: Option<String>,
    pub(super) exit_code: Option<i32>,
    pub(super) success: bool,
    pub(super) advisory: bool,
    pub(super) skipped: bool,
}

#[derive(Serialize)]
struct HooksRunResultJson {
    #[serde(flatten)]
    metadata: ContractMetadata,
    status: &'static str,
    hook: String,
    commands: Vec<CommandExecutionJson>,
    passed: usize,
    failed: usize,
    advisory_warnings: usize,
    skipped: usize,
}

pub(super) fn emit_json_result(
    hook: &str,
    commands: &[CommandExecutionJson],
    has_failure: bool,
) -> i32 {
    let passed = commands
        .iter()
        .filter(|result| result.success && !result.skipped)
        .count();
    let failed = commands
        .iter()
        .filter(|result| !result.success && !result.advisory && !result.skipped)
        .count();
    let advisory_warnings = commands
        .iter()
        .filter(|result| !result.success && result.advisory && !result.skipped)
        .count();
    let skipped = commands.iter().filter(|result| result.skipped).count();

    let result = HooksRunResultJson {
        metadata: ContractMetadata::current(),
        status: if has_failure { "finding" } else { "success" },
        hook: hook.to_string(),
        commands: commands.to_vec(),
        passed,
        failed,
        advisory_warnings,
        skipped,
    };
    println!("{}", serde_json::to_string(&result).unwrap());
    if has_failure { 1 } else { 0 }
}
