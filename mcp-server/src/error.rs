use thiserror::Error;

pub type McpResult<T> = Result<T, McpError>;

#[derive(Error, Debug)]
pub enum McpError {
    #[error("Parse error: {0}")]
    ParseError(#[from] terminusdb_woql_dsl::error::ParseError),

    #[error("Client error: {0}")]
    ClientError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("MCP transport error: {0}")]
    TransportError(String),
}
