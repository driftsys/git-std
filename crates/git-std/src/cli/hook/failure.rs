use crate::app::OutputFormat;
use crate::contract::{Diagnostic, print_json_error_with_exit};
use crate::ui;

use super::stash;

pub(super) fn emit(format: OutputFormat, message: impl Into<String>) -> i32 {
    let message = message.into();
    if format == OutputFormat::Json {
        print_json_error_with_exit(Diagnostic::operational("GITSTD-HOOK-RUN", message), 2)
    } else {
        ui::error(&message);
        2
    }
}

pub(super) fn drop_stash(format: OutputFormat, stash_sha: &str) -> Result<(), i32> {
    stash::stash_drop(stash_sha).map_err(|message| emit(format, message))
}
