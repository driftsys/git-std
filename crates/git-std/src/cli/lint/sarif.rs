use crate::contract::{ContractMetadata, DIAGNOSTIC_DEFINITIONS, Diagnostic};

/// Emit SARIF for a completed lint invocation and return finding exit class.
pub(super) fn emit_findings(diagnostics: &[Diagnostic]) -> i32 {
    emit(diagnostics, true);
    if diagnostics.is_empty() { 0 } else { 1 }
}

/// Emit SARIF for an operational failure and return exit class 2.
pub(super) fn emit_operational(code: &'static str, message: &str) -> i32 {
    emit(&[Diagnostic::operational(code, message.to_string())], false);
    2
}

fn emit(diagnostics: &[Diagnostic], execution_successful: bool) {
    let mut rules: Vec<serde_json::Value> = standard_commit::rules::RULES
        .iter()
        .map(|rule| {
            serde_json::json!({
                "id": rule.code,
                "name": rule.id,
                "shortDescription": { "text": rule.explanation },
            })
        })
        .collect();
    rules.extend(DIAGNOSTIC_DEFINITIONS.iter().map(|definition| {
        serde_json::json!({
            "id": definition.code,
            "name": definition.code,
            "shortDescription": { "text": definition.explanation },
        })
    }));
    let results: Vec<serde_json::Value> = diagnostics
        .iter()
        .map(|diagnostic| {
            serde_json::json!({
                "ruleId": diagnostic.code,
                "level": "error",
                "message": { "text": diagnostic.message },
            })
        })
        .collect();
    let metadata = ContractMetadata::current();
    let document = serde_json::json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "git-std",
                    "semanticVersion": metadata.tool_version,
                    "rules": rules,
                    "properties": { "schemaVersion": metadata.schema_version },
                }
            },
            "invocations": [{ "executionSuccessful": execution_successful }],
            "results": results,
        }],
    });
    println!("{document}");
}
