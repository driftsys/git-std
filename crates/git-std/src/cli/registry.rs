use serde::Serialize;

use crate::app::OutputFormat;
use crate::config::ProjectConfig;
use crate::contract::{ContractMetadata, DIAGNOSTIC_DEFINITIONS, DiagnosticDefinition};
use crate::{git, ui};

#[derive(Serialize)]
struct RuleJson {
    id: &'static str,
    code: &'static str,
    explanation: &'static str,
    configurable: bool,
}

#[derive(Serialize)]
struct EffectiveConvention {
    strict: bool,
    types: Option<Vec<String>>,
    scopes: Option<Vec<String>>,
    max_header_length: usize,
    require_scope: bool,
}

#[derive(Serialize)]
struct RegistryDocument {
    #[serde(flatten)]
    metadata: ContractMetadata,
    status: &'static str,
    facts: RegistryFacts,
    rules: Vec<RuleJson>,
    diagnostics: Vec<DiagnosticDefinition>,
    effective_convention: EffectiveConvention,
}

#[derive(Serialize)]
struct RegistryFacts {
    rules: &'static str,
    diagnostics: &'static str,
    effective_convention: &'static str,
}

/// Emit the rule and diagnostic registry using effective project convention.
pub fn run(config: &ProjectConfig, format: OutputFormat) -> i32 {
    let cwd = std::env::current_dir().unwrap_or_default();
    let root = git::workdir(&cwd).unwrap_or(cwd);
    let lint = config.to_lint_config(false, &root);

    if format == OutputFormat::Text {
        for rule in standard_commit::rules::RULES {
            ui::info(&format!("{}  {}", rule.code, rule.explanation));
        }
        return 0;
    }

    let rules: Vec<RuleJson> = standard_commit::rules::RULES
        .iter()
        .map(|rule| RuleJson {
            id: rule.id,
            code: rule.code,
            explanation: rule.explanation,
            configurable: rule.configurable,
        })
        .collect();
    let mut diagnostics: Vec<DiagnosticDefinition> = rules
        .iter()
        .map(|rule| DiagnosticDefinition {
            code: rule.code,
            class: "finding",
            explanation: rule.explanation,
        })
        .collect();
    diagnostics.extend(
        DIAGNOSTIC_DEFINITIONS
            .iter()
            .map(|definition| DiagnosticDefinition {
                code: definition.code,
                class: definition.class,
                explanation: definition.explanation,
            }),
    );

    let document = RegistryDocument {
        metadata: ContractMetadata::current(),
        status: "success",
        facts: RegistryFacts {
            rules: "static",
            diagnostics: "static",
            effective_convention: "resolved for current project",
        },
        rules,
        diagnostics,
        effective_convention: EffectiveConvention {
            strict: config.strict,
            types: lint.types,
            scopes: lint.scopes,
            max_header_length: lint.max_header_length,
            require_scope: lint.require_scope,
        },
    };
    println!(
        "{}",
        serde_json::to_string(&document).expect("registry document is serializable")
    );
    0
}
