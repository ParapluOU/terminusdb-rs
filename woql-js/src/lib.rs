//! JavaScript WOQL Syntax Parser
//!
//! This crate provides a bridge to the official terminusdb-client-js library
//! for parsing JavaScript-syntax WOQL queries into JSON-LD format.
//!
//! # Requirements
//!
//! - Node.js >= 14.0.0 must be installed and available in PATH
//! - The `scripts/package.json` dependencies must be installed (run `npm install` in the scripts directory)
//!
//! # Usage
//!
//! ```rust,no_run
//! use terminusdb_woql_js::parse_js_woql;
//!
//! let js_query = r#"
//!     select(
//!         "Name", "Age",
//!         and(
//!             triple("v:Person", "rdf:type", "@schema:Person"),
//!             triple("v:Person", "@schema:name", "v:Name"),
//!             triple("v:Person", "@schema:age", "v:Age")
//!         )
//!     )
//! "#;
//!
//! let json_ld = parse_js_woql(js_query).unwrap();
//! println!("{}", serde_json::to_string_pretty(&json_ld).unwrap());
//! ```
//!
//! # JS Syntax vs Rust DSL
//!
//! The JS syntax uses the terminusdb-client-js library syntax:
//! - Variables in queries: `"v:Name"` (strings with `v:` prefix)
//! - Variables in select/distinct: `"Name", "Age"` (variadic arguments without prefix)
//! - Functions: `triple(...)`, `select(...)`, `and(...)`, etc.
//! - Node references: `"@schema:Person"`, `"rdf:type"`
//!
//! This is different from the Rust DSL syntax which uses `$` for variables.

use std::io::Write;
use std::process::{Command, Stdio};
use anyhow::{Context, Result};

// Embed the bundled JavaScript at compile time
const BUNDLED_SCRIPT: &str = include_str!("../scripts/parse-woql.bundle.js");

/// Parse a JavaScript-syntax WOQL query string into JSON-LD format.
///
/// This function spawns a Node.js process that uses the official terminusdb-client-js
/// library to parse the query. The query string should use the JS syntax as defined
/// by terminusdb-client-js.
///
/// # Arguments
///
/// * `query` - A string containing the JS-syntax WOQL query
///
/// # Returns
///
/// Returns a `serde_json::Value` containing the JSON-LD representation of the query.
///
/// # Errors
///
/// This function will return an error if:
/// - Node.js is not installed or not in PATH
/// - The npm dependencies are not installed
/// - The query has syntax errors
/// - The query cannot be parsed by terminusdb-client-js
///
/// # Example
///
/// ```rust,no_run
/// use terminusdb_woql_js::parse_js_woql;
///
/// let query = r#"triple("v:Subject", "v:Predicate", "v:Object")"#;
/// let json_ld = parse_js_woql(query).unwrap();
/// ```
pub fn parse_js_woql(query: &str) -> Result<serde_json::Value> {
    // Create a temporary file to store the bundled script
    let mut temp_file = tempfile::NamedTempFile::new()
        .context("Failed to create temporary file for bundled script")?;

    // Write the bundled script to the temp file
    temp_file
        .write_all(BUNDLED_SCRIPT.as_bytes())
        .context("Failed to write bundled script to temporary file")?;

    // Flush to ensure all data is written
    temp_file.flush()
        .context("Failed to flush temporary file")?;

    // Spawn the Node.js process with the bundled script
    let mut child = Command::new("node")
        .arg(temp_file.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn Node.js process. Is Node.js installed and in PATH?")?;

    // Write the query to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(query.as_bytes())
            .context("Failed to write query to Node.js process stdin")?;
    }

    // Wait for the process to complete and collect output
    let output = child
        .wait_with_output()
        .context("Failed to read Node.js process output")?;

    // Check exit code
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Node.js parser failed with exit code {:?}:\n{}",
            output.status.code(),
            stderr
        ));
    }

    // Parse the JSON-LD output
    let stdout = String::from_utf8(output.stdout)
        .context("Node.js output is not valid UTF-8")?;

    let json_ld: serde_json::Value = serde_json::from_str(&stdout)
        .context("Failed to parse JSON-LD output from Node.js")?;

    Ok(json_ld)
}

/// Parse a JavaScript-syntax WOQL query string directly into a `terminusdb_woql2::Query`.
///
/// This is a convenience function that combines `parse_js_woql` with deserialization
/// into the Rust WOQL query type.
///
/// # Arguments
///
/// * `query` - A string containing the JS-syntax WOQL query
///
/// # Returns
///
/// Returns a `terminusdb_woql2::query::Query` object.
///
/// # Errors
///
/// This function will return an error if:
/// - Any error from `parse_js_woql` occurs
/// - The JSON-LD cannot be deserialized into a Query object
///
/// # Example
///
/// ```rust,no_run
/// use terminusdb_woql_js::parse_js_woql_to_query;
///
/// let query_str = r#"triple("v:S", "v:P", "v:O")"#;
/// let query = parse_js_woql_to_query(query_str).unwrap();
/// ```
pub fn parse_js_woql_to_query(query: &str) -> Result<terminusdb_woql2::query::Query> {
    let json_ld = parse_js_woql(query)?;

    let woql_query: terminusdb_woql2::query::Query = serde_json::from_value(json_ld)
        .context("Failed to deserialize JSON-LD into terminusdb_woql2::Query")?;

    Ok(woql_query)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires Node.js and npm dependencies installed
    fn test_simple_triple() {
        let query = r#"triple("v:Subject", "v:Predicate", "v:Object")"#;
        let result = parse_js_woql(query);
        assert!(result.is_ok(), "Failed to parse simple triple: {:?}", result.err());

        let json_ld = result.unwrap();
        assert!(json_ld.is_object());
        assert_eq!(json_ld["@type"], "Triple");
    }

    #[test]
    #[ignore] // Requires Node.js and npm dependencies installed
    fn test_select_query() {
        let query = r#"
            select(
                "Name",
                triple("v:Person", "@schema:name", "v:Name")
            )
        "#;
        let result = parse_js_woql(query);
        assert!(result.is_ok(), "Failed to parse select query: {:?}", result.err());

        let json_ld = result.unwrap();
        assert!(json_ld.is_object());
        assert_eq!(json_ld["@type"], "Select");
    }

    #[test]
    #[ignore] // Requires Node.js and npm dependencies installed
    fn test_complex_query() {
        let query = r#"
            select(
                "Name", "Age",
                and(
                    triple("v:Person", "rdf:type", "@schema:Person"),
                    triple("v:Person", "@schema:name", "v:Name"),
                    triple("v:Person", "@schema:age", "v:Age"),
                    greater("v:Age", 18)
                )
            )
        "#;
        let result = parse_js_woql(query);
        assert!(result.is_ok(), "Failed to parse complex query: {:?}", result.err());

        let json_ld = result.unwrap();
        assert!(json_ld.is_object());
        assert_eq!(json_ld["@type"], "Select");
    }

    #[test]
    #[ignore] // Requires Node.js and npm dependencies installed
    fn test_parse_to_query() {
        let query = r#"triple("v:S", "v:P", "v:O")"#;
        let result = parse_js_woql_to_query(query);
        assert!(result.is_ok(), "Failed to parse to Query: {:?}", result.err());

        let woql_query = result.unwrap();
        match woql_query {
            terminusdb_woql2::query::Query::Triple(_) => {
                // Success!
            }
            _ => panic!("Expected Triple query, got {:?}", woql_query),
        }
    }

    #[test]
    #[ignore] // Requires Node.js and npm dependencies installed
    fn test_invalid_syntax() {
        let query = r#"this is not valid WOQL"#;
        let result = parse_js_woql(query);
        assert!(result.is_err(), "Expected error for invalid syntax");
    }
}
