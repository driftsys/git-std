use serde::Serialize;

/// Current version of the public CLI JSON contracts.
pub const CLI_SCHEMA_VERSION: &str = "1.0.0";

/// Metadata embedded in every object-shaped machine response.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct ContractMetadata {
    /// Semantic version of the JSON contract.
    pub schema_version: &'static str,
    /// Version of the executable producing the document.
    pub tool_version: &'static str,
}

impl ContractMetadata {
    /// Build metadata for this executable and schema generation.
    pub const fn current() -> Self {
        Self {
            schema_version: CLI_SCHEMA_VERSION,
            tool_version: env!("CARGO_PKG_VERSION"),
        }
    }
}
