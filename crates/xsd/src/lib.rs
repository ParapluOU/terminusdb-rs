//! # terminusdb-xsd
//!
//! Runtime XSD to TerminusDB schema converter.
//!
//! This crate enables dynamic TerminusDB schema generation from XSD (XML Schema Definition)
//! files. It supports customer-uploaded schemas at runtime without code generation.
//!
//! ## Architecture
//!
//! 1. **Parse XSD** - Use the pure Rust `xmlschema` library
//! 2. **Extract Types** - Extract complex types, elements, attributes
//! 3. **Generate Schema** - Programmatically create TerminusDB Schema instances
//! 4. **Parse XML** - Convert XML documents to TerminusDB Instances
//!
//! ## Modules
//!
//! - `schema_model`: XSD schema extraction and parsing
//! - `schema_generator`: Runtime TerminusDB Schema generation from XSD
//! - `xml_parser`: XML to TerminusDB instance parsing
//!
//! ## Usage
//!
//! ```no_run
//! use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
//! use terminusdb_xsd::schema_model::XsdSchema;
//!
//! // Parse customer's XSD schema at runtime
//! let xsd_schema = XsdSchema::from_xsd_file("customer-schema.xsd", None::<&str>)?;
//!
//! // Generate TerminusDB schemas programmatically
//! let generator = XsdToSchemaGenerator::new();
//! let schemas = generator.generate(&xsd_schema)?;
//!
//! // Submit schemas to TerminusDB
//! for schema in schemas {
//!     // db.insert_schema(schema)?;
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod schema_generator;
pub mod schema_model;
pub mod xml_parser;
pub mod xsd_model;

use thiserror::Error;

pub use schema_model::*;
pub use xml_parser::{ParseResult, XmlParseError, XmlToInstanceParser};
pub use xsd_model::XsdModel;

#[derive(Debug, Error)]
pub enum XsdError {
    #[error("xmlschema error: {0}")]
    XmlSchema(#[from] xmlschema::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("XSD parsing error: {0}")]
    Parsing(String),

    #[error("Value conversion error: {0}")]
    Conversion(String),
}

pub type Result<T> = std::result::Result<T, XsdError>;

#[cfg(test)]
mod tests {
    #[test]
    fn test_json_roundtrip() {
        let json_str = r#"{"test": "value"}"#;
        let parsed: serde_json::Value = serde_json::from_str(json_str).unwrap();
        let test_val = parsed.get("test").and_then(|v| v.as_str()).unwrap();
        assert_eq!(test_val, "value");
    }
}
