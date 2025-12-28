//! # terminusdb-xsd
//!
//! Runtime XSD to TerminusDB schema converter using PyO3.
//!
//! This crate enables dynamic TerminusDB schema generation from XSD (XML Schema Definition)
//! files. It supports customer-uploaded schemas at runtime without code generation.
//!
//! ## Architecture
//!
//! 1. **Parse XSD** - Use PyO3 to call Python `xmlschema` library
//! 2. **Extract Types** - Extract complex types, elements, attributes
//! 3. **Generate Schema** - Programmatically create TerminusDB Schema instances
//! 4. **Parse XML** - (Future) Convert XML documents to TerminusDB Instances
//!
//! ## Requirements
//!
//! - Python 3.7+ with xmlschema installed: `pip install xmlschema`
//!
//! ## Modules
//!
//! - `schema_model`: XSD schema extraction and parsing (PyO3-based)
//! - `schema_generator`: Runtime TerminusDB Schema generation from XSD
//!
//! ## Usage
//!
//! ```no_run
//! use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
//! use terminusdb_xsd::schema_model::XsdSchema;
//!
//! // Parse customer's XSD schema at runtime
//! let xsd_schema = XsdSchema::from_xsd_file("customer-schema.xsd", None)?;
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

use pyo3::prelude::*;
use thiserror::Error;

pub use schema_model::*;
pub use xml_parser::{ParseResult, XmlParseError, XmlToInstanceParser};
pub use xsd_model::XsdModel;

#[derive(Debug, Error)]
pub enum XsdError {
    #[error("Python error: {0}")]
    Python(#[from] PyErr),

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
    use super::*;
    use pyo3::types::PyModule;

    #[test]
    fn test_python_basic() {
        Python::with_gil(|py| {
            let result: i32 = py.eval(c"2 + 2", None, None).unwrap().extract().unwrap();
            assert_eq!(result, 4);
        });
    }

    #[test]
    fn test_json_module() {
        Python::with_gil(|py| {
            let json = PyModule::import(py, "json").unwrap();
            let data = r#"{"test": "value"}"#;
            let result = json.call_method1("loads", (data,)).unwrap();
            let test_val: String = result
                .get_item("test")
                .unwrap()
                .extract()
                .unwrap();
            assert_eq!(test_val, "value");
        });
    }
}
