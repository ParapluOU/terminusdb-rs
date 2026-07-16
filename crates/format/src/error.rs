//! Error type for fallible format operations (currently only schema-document
//! parsing).

/// Errors produced while parsing the TerminusDB wire format.
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    /// A schema-graph document was malformed (missing key, unrecognized type
    /// family, unexpected property value shape, …).
    #[error("schema parse error: {0}")]
    SchemaParse(String),
}

/// Convenience alias for results in this crate.
pub type Result<T> = std::result::Result<T, FormatError>;
