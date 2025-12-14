#![doc = include_str!("../README.md")]

pub mod parser;
pub mod error;

pub use parser::parse_woql_dsl;
pub use error::{ParseError, ParseResult};