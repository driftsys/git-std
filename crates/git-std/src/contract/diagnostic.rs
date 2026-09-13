use serde::Serialize;

use super::ContractMetadata;

/// Registry entry for a stable machine-readable diagnostic.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct DiagnosticDefinition {
    /// Stable diagnostic identifier.
    pub code: &'static str,
    /// Semantic class used by machine consumers.
    pub class: &'static str,
    /// Human-readable meaning of the diagnostic.
    pub explanation: &'static str,
}

/// Operational and precondition diagnostics emitted by versioned CLI contracts.
pub const DIAGNOSTIC_DEFINITIONS: &[DiagnosticDefinition] = &[
    DiagnosticDefinition {
        code: "GITSTD-INVALID-ARGUMENT",
        class: "operational",
        explanation: "A command argument could not be parsed or is unsupported.",
    },
    DiagnosticDefinition {
        code: "GITSTD-IO-READ",
        class: "operational",
        explanation: "An input file could not be read.",
    },
    DiagnosticDefinition {
        code: "GITSTD-IO-WRITE",
        class: "operational",
        explanation: "An output file could not be written.",
    },
    DiagnosticDefinition {
        code: "GITSTD-GIT-OPERATION",
        class: "operational",
        explanation: "A required Git query or mutation failed.",
    },
    DiagnosticDefinition {
        code: "GITSTD-VERSION-NO-TAG",
        class: "operational",
        explanation: "No version tag exists for the requested version query.",
    },
    DiagnosticDefinition {
        code: "GITSTD-BUMP-PLAN",
        class: "operational",
        explanation: "The bump plan could not be constructed.",
    },
    DiagnosticDefinition {
        code: "GITSTD-BUMP-PLAN-DIVERGED",
        class: "precondition",
        explanation: "Current inputs do not reproduce the expected bump plan.",
    },
    DiagnosticDefinition {
        code: "GITSTD-BUMP-POLICY",
        class: "precondition",
        explanation: "The requested bump violates the configured version policy.",
    },
    DiagnosticDefinition {
        code: "GITSTD-BUMP-BRANCH",
        class: "precondition",
        explanation: "The current branch is not the configured release branch.",
    },
    DiagnosticDefinition {
        code: "GITSTD-DOCTOR-CHECK-FAILED",
        class: "finding",
        explanation: "One or more repository health checks failed.",
    },
    DiagnosticDefinition {
        code: "GITSTD-LINT-EMPTY-RANGE",
        class: "finding",
        explanation: "A lint range was empty while its inverse contained commits.",
    },
    DiagnosticDefinition {
        code: "GITSTD-LIFECYCLE-HOOK",
        class: "operational",
        explanation: "A configured lifecycle hook could not complete.",
    },
    DiagnosticDefinition {
        code: "GITSTD-HOOK-RUN",
        class: "operational",
        explanation: "A Git hook could not complete its setup or cleanup safely.",
    },
];

/// Severity of a stable CLI diagnostic.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// The operation could not complete.
    Error,
    /// The operation completed with a condition requiring attention.
    Warning,
}

/// A machine-readable diagnostic with a stable identifier.
#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    /// Stable identifier registered by `git std registry`.
    pub code: &'static str,
    /// Diagnostic severity.
    pub severity: Severity,
    /// Human-readable explanation.
    pub message: String,
}

impl Diagnostic {
    /// Create an operational-error diagnostic.
    pub fn operational(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
        }
    }
}

/// Top-level JSON document for a failed machine operation.
#[derive(Debug, Serialize)]
pub struct ErrorDocument {
    /// Shared contract metadata.
    #[serde(flatten)]
    pub metadata: ContractMetadata,
    /// Outcome discriminator.
    pub status: &'static str,
    /// Process exit class returned for this failure.
    pub exit_code: i32,
    /// One or more reasons the operation failed.
    pub diagnostics: Vec<Diagnostic>,
}

/// Print one structured operational failure and return exit class 2.
pub fn print_json_error(diagnostic: Diagnostic) -> i32 {
    print_json_error_with_exit(diagnostic, 2)
}

/// Print one structured failure and return the caller-selected exit class.
pub fn print_json_error_with_exit(diagnostic: Diagnostic, exit_code: i32) -> i32 {
    let document = ErrorDocument {
        metadata: ContractMetadata::current(),
        status: "error",
        exit_code,
        diagnostics: vec![diagnostic],
    };
    println!(
        "{}",
        serde_json::to_string(&document).expect("error document is serializable")
    );
    exit_code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_document_contains_contract_metadata_and_diagnostic() {
        let document = ErrorDocument {
            metadata: ContractMetadata::current(),
            status: "error",
            exit_code: 2,
            diagnostics: vec![Diagnostic::operational(
                "GITSTD-IO-READ",
                "cannot read input",
            )],
        };

        let value = serde_json::to_value(document).expect("serializable error document");
        assert_eq!(value["schema_version"], "1.0.0");
        assert_eq!(value["tool_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(value["status"], "error");
        assert_eq!(value["diagnostics"][0]["code"], "GITSTD-IO-READ");
    }
}
