use crate::app::OutputFormat;
use crate::contract::{Diagnostic, print_json_error, print_json_error_with_exit};
use crate::ui;

use super::BumpOptions;

pub(super) fn machine_or_human_error(
    opts: &BumpOptions,
    code: &'static str,
    message: impl Into<String>,
    exit_code: i32,
) -> i32 {
    let message = message.into();
    if opts.format == OutputFormat::Json {
        if exit_code == 2 {
            print_json_error(Diagnostic::operational(code, message))
        } else {
            print_json_error_with_exit(Diagnostic::operational(code, message), exit_code)
        }
    } else {
        ui::error(&message);
        exit_code
    }
}

pub(super) fn plan_diverged(opts: &BumpOptions, expected: &str, actual: &str) -> i32 {
    machine_or_human_error(
        opts,
        "GITSTD-BUMP-PLAN-DIVERGED",
        format!("expected plan {expected}, but current inputs produce {actual}"),
        1,
    )
}

pub(super) fn lifecycle_failure(opts: &BumpOptions, hook: &str, exit_code: i32) -> i32 {
    if opts.format == OutputFormat::Json {
        machine_or_human_error(
            opts,
            "GITSTD-LIFECYCLE-HOOK",
            format!("{hook} lifecycle hook failed"),
            2,
        )
    } else {
        exit_code
    }
}
