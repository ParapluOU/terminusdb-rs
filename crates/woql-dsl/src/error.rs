use thiserror::Error;

pub type ParseResult<T> = Result<T, ParseError>;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Parse error at position {position}: {message}")]
    ParseError {
        position: usize,
        message: String,
    },

    #[error("Unexpected end of input")]
    UnexpectedEof,

    #[error("Invalid variable name: {0}")]
    InvalidVariable(String),

    #[error("Invalid function: {0}")]
    InvalidFunction(String),

    #[error("Invalid argument count for {function}: expected {expected}, got {got}")]
    InvalidArgumentCount {
        function: String,
        expected: String,
        got: usize,
    },

    #[error("Invalid literal: {0}")]
    InvalidLiteral(String),

    #[error("Nom parsing error: {0}")]
    NomError(String),
}