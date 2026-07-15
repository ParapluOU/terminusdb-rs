//! Error type for the XPath -> WOQL compiler.

use thiserror::Error;

/// Errors produced while parsing, lowering, or compiling an XPath expression.
#[derive(Debug, Error)]
pub enum XPathError {
    /// The `xee-xpath-ast` parser rejected the expression.
    #[error("failed to parse XPath expression: {0}")]
    Parse(String),

    /// The expression parsed, but uses an XPath construct we do not (yet)
    /// know how to lower into WOQL. See `ROADMAP.md` for coverage.
    #[error("unsupported XPath construct: {0}")]
    Unsupported(String),

    /// The expression was empty / selected nothing to compile.
    #[error("empty XPath expression")]
    Empty,
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, XPathError>;

impl XPathError {
    pub(crate) fn unsupported(msg: impl Into<String>) -> Self {
        XPathError::Unsupported(msg.into())
    }
}
