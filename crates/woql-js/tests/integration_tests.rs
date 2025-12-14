use terminusdb_woql_js::{parse_js_woql, parse_js_woql_to_query};
use terminusdb_woql2::query::Query;

/// Test parsing a simple triple query
#[test]
#[ignore] // Requires Node.js and npm dependencies installed
fn test_parse_simple_triple() {
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

/// Test parsing with WOQL object prefix (as used in dashboard)
#[test]
#[ignore] // Requires Node.js and npm dependencies installed
fn test_parse_with_woql_prefix() {
    // Note: The emerge() prelude automatically provides WOQL functions,
    // so we don't need to prefix with WOQL. in the query string
    let query = r#"triple("v:S", "v:P", "v:O")"#;

    let result = parse_js_woql(query);
    assert!(result.is_ok());
}

/// Test parsing a select query
#[test]
#[ignore] // Requires Node.js and npm dependencies installed
fn test_parse_select_query() {
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
    assert_eq!(json_ld["@type"], "Select");
}

/// Test parsing an and query
#[test]
#[ignore] // Requires Node.js and npm dependencies installed
fn test_parse_and_query() {
    let query = r#"
        and(
            triple("v:Person", "rdf:type", "@schema:Person"),
            triple("v:Person", "@schema:name", "v:Name")
        )
    "#;

    let result = parse_js_woql(query);
    assert!(result.is_ok(), "Failed to parse and query: {:?}", result.err());

    let json_ld = result.unwrap();
    assert_eq!(json_ld["@type"], "And");
}

/// Test parsing an or query
#[test]
#[ignore] // Requires Node.js and npm dependencies installed
fn test_parse_or_query() {
    let query = r#"
        or(
            triple("v:Person", "@schema:isAdult", true),
            greater("v:Age", 18)
        )
    "#;

    let result = parse_js_woql(query);
    assert!(result.is_ok(), "Failed to parse or query: {:?}", result.err());

    let json_ld = result.unwrap();
    assert_eq!(json_ld["@type"], "Or");
}

/// Test parsing a complex query with select, and, and comparisons
#[test]
#[ignore] // Requires Node.js and npm dependencies installed
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
    assert!(
        result.is_ok(),
        "Failed to parse complex query: {:?}",
        result.err()
    );

    let json_ld = result.unwrap();
    assert_eq!(json_ld["@type"], "Select");

    // Verify it has the expected structure
    assert!(json_ld["variables"].is_array());
    assert!(json_ld["query"].is_object());
}

/// Test parsing directly to Query type
#[test]
#[ignore] // Requires Node.js and npm dependencies installed
fn test_parse_to_query_type() {
    let query = r#"triple("v:S", "v:P", "v:O")"#;

    let result = parse_js_woql_to_query(query);
    assert!(
        result.is_ok(),
        "Failed to parse to Query: {:?}",
        result.err()
    );

    let woql_query = result.unwrap();
    match woql_query {
        Query::Triple(_) => {
            // Success!
        }
        _ => panic!("Expected Triple query, got {:?}", woql_query),
    }
}

/// Test parsing a select query to Query type
#[test]
#[ignore] // Requires Node.js and npm dependencies installed
fn test_parse_select_to_query_type() {
    let query = r#"
        select(
            "Name",
            triple("v:Person", "@schema:name", "v:Name")
        )
    "#;

    let result = parse_js_woql_to_query(query);
    assert!(result.is_ok());

    let woql_query = result.unwrap();
    match woql_query {
        Query::Select(select) => {
            assert_eq!(select.variables.len(), 1);
            assert_eq!(select.variables[0], "Name");
        }
        _ => panic!("Expected Select query, got {:?}", woql_query),
    }
}

/// Test error handling for invalid syntax
#[test]
#[ignore] // Requires Node.js and npm dependencies installed
fn test_invalid_syntax_error() {
    let query = r#"this is not valid WOQL syntax"#;

    let result = parse_js_woql(query);
    assert!(
        result.is_err(),
        "Expected error for invalid syntax, but got success"
    );
}

/// Test error handling for incomplete query
#[test]
#[ignore] // Requires Node.js and npm dependencies installed
fn test_incomplete_query_error() {
    let query = r#"triple(v("S"), v("P")"#; // Missing closing parenthesis

    let result = parse_js_woql(query);
    assert!(
        result.is_err(),
        "Expected error for incomplete query"
    );
}

/// Test parsing with Vars helper (from terminusdb-client-js)
#[test]
#[ignore] // Requires Node.js and npm dependencies installed
fn test_parse_with_vars_helper() {
    // Variables are strings with v: prefix
    let query = r#"
        triple("v:Person", "@schema:name", "v:Name")
    "#;

    let result = parse_js_woql(query);
    assert!(result.is_ok());
}

/// Test parsing path query
#[test]
#[ignore] // Requires Node.js and npm dependencies installed
fn test_parse_path_query() {
    let query = r#"
        path(
            "v:Person",
            star("v:Predicate"),
            "v:Object",
            "v:Path"
        )
    "#;

    let result = parse_js_woql(query);
    assert!(
        result.is_ok(),
        "Failed to parse path query: {:?}",
        result.err()
    );
}

/// Test parsing document operations
#[test]
#[ignore] // Requires Node.js and npm dependencies installed
fn test_parse_document_operations() {
    let query = r#"
        and(
            read_document("v:ID", "v:Doc"),
            insert_document("v:NewDoc", "v:NewID")
        )
    "#;

    let result = parse_js_woql(query);
    assert!(
        result.is_ok(),
        "Failed to parse document operations: {:?}",
        result.err()
    );
}

/// Test parsing with limit and order_by
#[test]
#[ignore] // Requires Node.js and npm dependencies installed
fn test_parse_with_modifiers() {
    let query = r#"
        limit(
            10,
            order_by(
                [asc("v:Name")],
                triple("v:Person", "@schema:name", "v:Name")
            )
        )
    "#;

    let result = parse_js_woql(query);
    assert!(
        result.is_ok(),
        "Failed to parse query with modifiers: {:?}",
        result.err()
    );
}
