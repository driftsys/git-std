/// Stable code for an invalid conventional-commit header.
pub const PARSE_CODE: &str = "GITSTD-COMMIT-PARSE";
/// Stable code for a header exceeding the configured length.
pub const HEADER_LENGTH_CODE: &str = "GITSTD-COMMIT-HEADER-LENGTH";
/// Stable code for a disallowed commit type.
pub const TYPE_CODE: &str = "GITSTD-COMMIT-TYPE";
/// Stable code for a missing required scope.
pub const SCOPE_REQUIRED_CODE: &str = "GITSTD-COMMIT-SCOPE-REQUIRED";
/// Stable code for a disallowed scope.
pub const SCOPE_CODE: &str = "GITSTD-COMMIT-SCOPE";

/// Static description of one lint rule.
#[derive(Clone, Copy, Debug)]
pub struct RuleDefinition {
    /// Stable rule identifier.
    pub id: &'static str,
    /// Diagnostic emitted when the rule fails.
    pub code: &'static str,
    /// Human-readable rule explanation.
    pub explanation: &'static str,
    /// Whether project configuration changes this rule.
    pub configurable: bool,
}

/// Canonical conventional-commit rule registry.
pub const RULES: &[RuleDefinition] = &[
    RuleDefinition {
        id: "conventional-header",
        code: PARSE_CODE,
        explanation: "The header must follow <type>[(<scope>)][!]: <description>.",
        configurable: false,
    },
    RuleDefinition {
        id: "header-length",
        code: HEADER_LENGTH_CODE,
        explanation: "The header must not exceed the configured maximum length.",
        configurable: true,
    },
    RuleDefinition {
        id: "allowed-type",
        code: TYPE_CODE,
        explanation: "Strict mode restricts the commit type to the effective allowlist.",
        configurable: true,
    },
    RuleDefinition {
        id: "required-scope",
        code: SCOPE_REQUIRED_CODE,
        explanation: "Strict mode can require a scope when scopes are configured.",
        configurable: true,
    },
    RuleDefinition {
        id: "allowed-scope",
        code: SCOPE_CODE,
        explanation: "Strict mode restricts scopes to the effective allowlist.",
        configurable: true,
    },
];
