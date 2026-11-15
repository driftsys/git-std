use super::types::{TagOptions, TagPlan};
use crate::{app::OutputFormat, contract::ContractMetadata, ui};

pub(super) fn print(options: &TagOptions, plan: &TagPlan, exit: i32, error: Option<&str>) {
    if options.format == OutputFormat::Json {
        let metadata = ContractMetadata::current();
        let diagnostics: Vec<_>=plan.diagnostics.iter().map(|message|serde_json::json!({"code":"GITSTD-TAG-NOTE","severity":"warning","message":message})).chain(error.map(|message|serde_json::json!({"code":if exit==1{"GITSTD-TAG-REFUSED"}else{"GITSTD-GIT-OPERATION"},"severity":"error","message":message}))).collect();
        println!(
            "{}",
            serde_json::json!({"schema_version":metadata.schema_version,"tool_version":metadata.tool_version,"status":if exit==0{"success"}else{"error"},"exit_code":exit,"dry_run":options.dry_run,"target_oid":plan.target_oid,"release_branch":plan.release_branch,"remote":options.push,"tags":plan.tags,"diagnostics":diagnostics})
        );
        return;
    }
    if let Some(error) = error {
        ui::error(error);
    }
    for diagnostic in &plan.diagnostics {
        ui::warning(diagnostic);
    }
    for tag in &plan.tags {
        if options.dry_run && exit == 0 {
            ui::info(&format!(
                "Would {} {} at {}{}",
                tag.action,
                tag.name,
                plan.target_oid,
                if options.push.is_some() {
                    " and ensure it is published"
                } else {
                    ""
                }
            ));
        } else if matches!(tag.local, "created" | "adopted") {
            ui::info(&format!("Tagged {} at {}", tag.name, plan.target_oid));
        }
        if tag.remote == "pushed" {
            ui::info(&format!("Published {}", tag.name));
        }
    }
}
