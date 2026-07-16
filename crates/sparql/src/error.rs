//! Error type for the SPARQL -> WOQL compiler.

use thiserror::Error;

/// Errors produced while parsing, lowering, or compiling a SPARQL query.
#[derive(Debug, Error)]
pub enum SparqlError {
    /// The `spargebra` parser rejected the query text.
    #[error("failed to parse SPARQL query: {0}")]
    Parse(String),

    /// The query parsed, but uses a SPARQL construct we do not (yet) know how to
    /// lower into WOQL. See `ROADMAP.md` for coverage.
    #[error("unsupported SPARQL construct: {0}")]
    Unsupported(String),

    /// The query form is not a `SELECT` (e.g. ASK/CONSTRUCT/DESCRIBE), which the
    /// v1 compiler does not target.
    #[error("unsupported query form: {0} (only SELECT is compiled)")]
    UnsupportedForm(String),

    /// The query parsed but selected nothing to compile.
    #[error("empty SPARQL query")]
    Empty,
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, SparqlError>;

impl SparqlError {
    pub(crate) fn unsupported(msg: impl Into<String>) -> Self {
        SparqlError::Unsupported(msg.into())
    }
}
