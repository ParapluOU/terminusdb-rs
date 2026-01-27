//! QuickJS-based WOQL parser runtime
//!
//! This module provides WOQL parsing using the embedded QuickJS JavaScript engine.
//! It requires no external dependencies at runtime.

use anyhow::{Context, Result};
use rquickjs::{Context as JsContext, Runtime};
use std::cell::RefCell;

/// The bundled QuickJS-compatible JavaScript for WOQL parsing (~128KB)
const QUICKJS_BUNDLE: &str = include_str!("../scripts/parse-woql.quickjs.js");

// Thread-local storage for the QuickJS runtime
// Each thread gets its own runtime instance since QuickJS is not thread-safe
thread_local! {
    static RUNTIME: RefCell<Option<Runtime>> = const { RefCell::new(None) };
}

fn with_runtime<F, R>(f: F) -> R
where
    F: FnOnce(&Runtime) -> R,
{
    RUNTIME.with(|rt| {
        let mut rt_ref = rt.borrow_mut();
        if rt_ref.is_none() {
            *rt_ref = Some(Runtime::new().expect("Failed to create QuickJS runtime"));
        }
        f(rt_ref.as_ref().unwrap())
    })
}

/// Parse a JavaScript-syntax WOQL query string into JSON-LD format using QuickJS.
///
/// This function uses the embedded QuickJS JavaScript engine to parse the query.
/// No external dependencies (like Node.js) are required at runtime.
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
/// - The JavaScript runtime encounters an error
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
    with_runtime(|runtime| {
        let ctx = JsContext::full(runtime).context("Failed to create QuickJS context")?;

    ctx.with(|ctx| {
        // Load the bundle (this defines the global parseWoql function)
        let _: () = ctx
            .eval(QUICKJS_BUNDLE)
            .map_err(|e| {
                // Try to get more detailed error info
                if let Some(exc) = ctx.catch().as_exception() {
                    if let Some(msg) = exc.message() {
                        return anyhow::anyhow!("Failed to load WOQL bundle: {}", msg);
                    }
                }
                anyhow::anyhow!("Failed to load WOQL bundle: {:?}", e)
            })?;

        // Escape the query string for JavaScript
        let escaped_query = serde_json::to_string(query)
            .context("Failed to escape query string")?;

        // Call parseWoql with the query
        let js_code = format!("parseWoql({})", escaped_query);
        let result: String = ctx
            .eval(js_code)
            .map_err(|e| {
                // Try to get more detailed error info
                if let Some(exc) = ctx.catch().as_exception() {
                    if let Some(msg) = exc.message() {
                        return anyhow::anyhow!("WOQL parse error: {}", msg);
                    }
                }
                anyhow::anyhow!("WOQL parse error: {:?}", e)
            })?;

        // Parse the JSON result
        serde_json::from_str(&result).context("Failed to parse JSON-LD output from QuickJS")
    })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_triple() {
        let query = r#"triple("v:Subject", "v:Predicate", "v:Object")"#;
        let result = parse_js_woql(query);
        assert!(result.is_ok(), "Failed to parse simple triple: {:?}", result.err());

        let json_ld = result.unwrap();
        assert!(json_ld.is_object());
        assert_eq!(json_ld["@type"], "Triple");
    }

    #[test]
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
    fn test_parse_complex_query() {
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
    fn test_invalid_syntax_returns_error() {
        let query = r#"this is not valid WOQL"#;
        let result = parse_js_woql(query);
        assert!(result.is_err(), "Expected error for invalid syntax");
    }
}
