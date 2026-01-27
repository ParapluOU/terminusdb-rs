//! Node.js-based WOQL parser runtime
//!
//! This module provides WOQL parsing using Node.js as a subprocess.
//! It requires Node.js >= 14.0.0 to be installed and available in PATH.

use anyhow::{Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

/// The bundled Node.js JavaScript for WOQL parsing (~965KB)
const NODEJS_BUNDLE: &str = include_str!("../scripts/parse-woql.bundle.js");

/// Parse a JavaScript-syntax WOQL query string into JSON-LD format using Node.js.
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
        .write_all(NODEJS_BUNDLE.as_bytes())
        .context("Failed to write bundled script to temporary file")?;

    // Flush to ensure all data is written
    temp_file
        .flush()
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
    let stdout = String::from_utf8(output.stdout).context("Node.js output is not valid UTF-8")?;

    let json_ld: serde_json::Value =
        serde_json::from_str(&stdout).context("Failed to parse JSON-LD output from Node.js")?;

    Ok(json_ld)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires Node.js installed
    fn test_parse_simple_triple() {
        let query = r#"triple("v:Subject", "v:Predicate", "v:Object")"#;
        let result = parse_js_woql(query);
        assert!(result.is_ok(), "Failed to parse simple triple: {:?}", result.err());

        let json_ld = result.unwrap();
        assert!(json_ld.is_object());
        assert_eq!(json_ld["@type"], "Triple");
    }

    #[test]
    #[ignore] // Requires Node.js installed
    fn test_parse_select_query() {
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
    #[ignore] // Requires Node.js installed
    fn test_invalid_syntax_returns_error() {
        let query = r#"this is not valid WOQL"#;
        let result = parse_js_woql(query);
        assert!(result.is_err(), "Expected error for invalid syntax");
    }
}
