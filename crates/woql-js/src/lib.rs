//! JavaScript WOQL Syntax Parser
//!
//! This crate provides a bridge to the official terminusdb-client-js library
//! for parsing JavaScript-syntax WOQL queries into JSON-LD format.
//!
//! # Runtimes
//!
//! This crate supports two JavaScript runtimes:
//!
//! - **QuickJS** (default): Embedded JavaScript engine with no external dependencies.
//!   Fast startup (~1ms), smaller bundle (~128KB), works everywhere Rust compiles.
//!
//! - **Node.js** (fallback): Uses Node.js as a subprocess. Requires Node.js >= 14.0.0
//!   to be installed. Slower startup (~50-100ms), larger bundle (~965KB).
//!
//! # Feature Flags
//!
//! - `quickjs` (default): Use the embedded QuickJS runtime
//! - `nodejs`: Use Node.js subprocess (fallback for compatibility)
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

use anyhow::{Context, Result};
use terminusdb_woql2::prelude::FromTDBInstance;

// Runtime modules with conditional compilation
#[cfg(feature = "quickjs")]
mod quickjs_runtime;

#[cfg(feature = "nodejs")]
mod nodejs_runtime;

// Test module (only compiled during testing)
#[cfg(test)]
mod quickjs_test;

// Re-export the appropriate parse_js_woql function based on features
// Priority: quickjs > nodejs

/// Parse a JavaScript-syntax WOQL query string into JSON-LD format.
///
/// This function uses the embedded QuickJS JavaScript engine by default,
/// or falls back to Node.js subprocess if compiled with the `nodejs` feature
/// instead of `quickjs`.
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
/// - The query has syntax errors
/// - The query cannot be parsed by the WOQL library
/// - (nodejs only) Node.js is not installed or not in PATH
///
/// # Example
///
/// ```rust,no_run
/// use terminusdb_woql_js::parse_js_woql;
///
/// let query = r#"triple("v:Subject", "v:Predicate", "v:Object")"#;
/// let json_ld = parse_js_woql(query).unwrap();
/// ```
#[cfg(feature = "quickjs")]
pub fn parse_js_woql(query: &str) -> Result<serde_json::Value> {
    quickjs_runtime::parse_js_woql(query)
}

#[cfg(all(feature = "nodejs", not(feature = "quickjs")))]
pub fn parse_js_woql(query: &str) -> Result<serde_json::Value> {
    nodejs_runtime::parse_js_woql(query)
}

// Compile error if neither feature is enabled
#[cfg(not(any(feature = "quickjs", feature = "nodejs")))]
compile_error!("At least one runtime feature must be enabled: 'quickjs' or 'nodejs'");

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

    let woql_query: terminusdb_woql2::query::Query =
        terminusdb_woql2::query::Query::from_json(json_ld)
            .context("Failed to deserialize JSON-LD into terminusdb_woql2::Query")?;

    Ok(woql_query)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_triple() {
        let query = r#"triple("v:Subject", "v:Predicate", "v:Object")"#;
        let result = parse_js_woql(query);
        assert!(
            result.is_ok(),
            "Failed to parse simple triple: {:?}",
            result.err()
        );

        let json_ld = result.unwrap();
        assert!(json_ld.is_object());
        assert_eq!(json_ld["@type"], "Triple");
    }

    #[test]
    fn test_select_query() {
        let query = r#"
            select(
                "Name",
                triple("v:Person", "@schema:name", "v:Name")
            )
        "#;
        let result = parse_js_woql(query);
        assert!(
            result.is_ok(),
            "Failed to parse select query: {:?}",
            result.err()
        );

        let json_ld = result.unwrap();
        assert!(json_ld.is_object());
        assert_eq!(json_ld["@type"], "Select");
    }

    #[test]
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
        assert!(
            result.is_ok(),
            "Failed to parse complex query: {:?}",
            result.err()
        );

        let json_ld = result.unwrap();
        assert!(json_ld.is_object());
        assert_eq!(json_ld["@type"], "Select");
    }

    #[test]
    fn test_parse_to_query() {
        let query = r#"triple("v:S", "v:P", "v:O")"#;
        let result = parse_js_woql_to_query(query);
        assert!(
            result.is_ok(),
            "Failed to parse to Query: {:?}",
            result.err()
        );

        let woql_query = result.unwrap();
        match woql_query {
            terminusdb_woql2::query::Query::Triple(_) => {
                // Success!
            }
            _ => panic!("Expected Triple query, got {:?}", woql_query),
        }
    }

    #[test]
    fn test_invalid_syntax() {
        let query = r#"this is not valid WOQL"#;
        let result = parse_js_woql(query);
        assert!(result.is_err(), "Expected error for invalid syntax");
    }
}
