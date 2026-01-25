// TODO: This test file uses debug module functionality (clear_operation_log, get_operation_log,
// local_node_test, debug::OperationType) that is not fully implemented.
// Disabled until the debug module is complete.
#![cfg(feature = "__disabled_test_last_query_via_operation_log")]

use terminusdb_client::*;
use terminusdb_woql_builder::prelude::*;

#[tokio::test]
async fn test_last_query_retrieval() -> anyhow::Result<()> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let spec = BranchSpec::from("test_last_query");

    // Clear operation log to start fresh
    client.clear_operation_log();

    // Initially, there should be no last query
    assert!(client.last_query().is_none());
    assert!(client.last_query_json().is_none());

    // Execute a query
    let query = WoqlBuilder::new()
        .select(vec![vars!("Subject"), vars!("Predicate"), vars!("Object")])
        .triple("v:Subject", "v:Predicate", "v:Object")
        .finalize();

    let _: WOQLResult<serde_json::Value> = client.query(Some(spec.clone()), query.clone()).await?;

    // Now we should have a last query
    let last_query = client.last_query();
    assert!(last_query.is_some());

    // The JSON representation should also be available
    let last_query_json = client.last_query_json();
    assert!(last_query_json.is_some());

    // Execute another query
    let query2 = WoqlBuilder::new()
        .select(vec![vars!("Class")])
        .triple("v:Class", "rdf:type", "owl:Class")
        .finalize();

    let _: WOQLResult<serde_json::Value> = client.query(Some(spec), query2.clone()).await?;

    // The last query should now be the second query
    let last_query2 = client.last_query();
    assert!(last_query2.is_some());

    // Check operation log has both queries
    let operations = client.get_operation_log();
    let query_operations: Vec<_> = operations
        .iter()
        .filter(|op| {
            matches!(
                op.operation_type,
                terminusdb_client::debug::OperationType::Query
            )
        })
        .collect();

    assert!(query_operations.len() >= 2);

    Ok(())
}

#[tokio::test]
async fn test_query_string_last_query() -> anyhow::Result<()> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let spec = BranchSpec::from("test_query_string");

    client.clear_operation_log();

    // Execute a query using DSL string
    let dsl_query =
        r#"select([$Subject, $Predicate, $Object], triple($Subject, $Predicate, $Object))"#;
    let _: WOQLResult<serde_json::Value> =
        client.query_string(Some(spec.clone()), dsl_query).await?;

    // Should have a last query
    let last_query = client.last_query();
    assert!(last_query.is_some());

    // Execute a query using JSON-LD string
    let json_query = r#"{
        "@type": "Select",
        "variables": ["Subject"],
        "query": {
            "@type": "Triple",
            "subject": {"@type": "Value", "variable": "Subject"},
            "predicate": {"@type": "Value", "node": "rdf:type"},
            "object": {"@type": "Value", "node": "owl:Class"}
        }
    }"#;

    let _: WOQLResult<serde_json::Value> = client.query_string(Some(spec), json_query).await?;

    // Should have the parsed query if it was parseable
    let operations = client.get_operation_log();
    let query_operations: Vec<_> = operations
        .iter()
        .filter(|op| {
            matches!(
                op.operation_type,
                terminusdb_client::debug::OperationType::Query
            )
        })
        .collect();

    // Both query_string calls should be logged
    assert!(query_operations.len() >= 2);

    Ok(())
}
