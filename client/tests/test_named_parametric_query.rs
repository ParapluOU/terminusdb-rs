use terminusdb_bin::TerminusDBServer;
use terminusdb_client::{TerminusDBHttpClient, DocumentInsertArgs, BranchSpec};
use terminusdb_woql2::*;
use terminusdb_woql2::query::{NamedParametricQuery, Query};
use terminusdb_schema::ToTDBSchema;
use serde_json::{json, Value};

#[tokio::test]
async fn test_insert_named_parametric_query() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_npq", |client, spec| async move {
            // First, let's check what schema NamedParametricQuery generates
            let schema = NamedParametricQuery::to_schema();
            println!("NamedParametricQuery schema: {:#?}", schema);

            // Create a simple NamedParametricQuery
            let npq = NamedParametricQuery {
                name: "test_query".to_string(),
                parameters: vec![],
                query: Query::True(query::True {}),
            };

            // Skip instance conversion for now since NPQ doesn't implement ToTDBInstance
            println!("Created NamedParametricQuery: {:#?}", npq);

            // Let's also create the JSON representation
            let json_value = json!({
                "@type": "NamedParametricQuery",
                "@id": "NamedParametricQuery/test_query",
                "name": "test_query",
                "parameters": [],
                "query": {
                    "@type": "True"
                }
            });

            println!("JSON representation: {:#?}", json_value);

            // Try to insert as raw JSON document
            let args = DocumentInsertArgs::from(spec.clone());
            match client.insert_document(&json_value, args).await {
                Ok(result) => {
                    println!("Successfully inserted NPQ!");
                    println!("Result: {:?}", result);

                    // Now try to call it using query_string
                    let call_query = json!({
                        "@type": "Call",
                        "name": "test_query",
                        "arguments": []
                    });

                    match client.query_string::<Value>(Some(spec.clone()), &serde_json::to_string(&call_query)?, None).await {
                        Ok(result) => println!("Call succeeded! Result: {:?}", result),
                        Err(e) => println!("Call failed: {:?}", e),
                    }
                }
                Err(e) => {
                    println!("Failed to insert NPQ: {:?}", e);
                }
            }

            Ok(())
        })
        .await
}

#[tokio::test]
async fn test_parametric_query_with_params() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_npq_params", |client, spec| async move {
            // Create a parametric query with one parameter
            let npq = NamedParametricQuery {
                name: "find_by_type".to_string(),
                parameters: vec!["type".to_string()],
                query: triple!(var!(x), "rdf:type", var!(type)),
            };

            println!("Parametric NPQ: {:?}", npq);

            // Try to insert as JSON
            let json_npq = json!({
                "@type": "NamedParametricQuery",
                "@id": "NamedParametricQuery/find_by_type",
                "name": "find_by_type",
                "parameters": ["type"],
                "query": {
                    "@type": "Triple",
                    "subject": {"variable": "x"},
                    "predicate": {"node": "rdf:type"},
                    "object": {"variable": "type"}
                }
            });

            let args = DocumentInsertArgs::from(spec.clone());
            match client.insert_document(&json_npq, args).await {
                Ok(result) => {
                    println!("Successfully inserted parametric query!");
                    println!("Result: {:?}", result);

                    // Now try to call it with an argument
                    let call_query = json!({
                        "@type": "Call",
                        "name": "find_by_type",
                        "arguments": [{"node": "Person"}]
                    });

                    match client.query_string::<Value>(Some(spec.clone()), &serde_json::to_string(&call_query)?, None).await {
                        Ok(result) => println!("Call with params succeeded: {:?}", result),
                        Err(e) => println!("Call with params failed: {:?}", e),
                    }
                }
                Err(e) => {
                    println!("Failed to insert parametric query: {:?}", e);
                }
            }

            Ok(())
        })
        .await
}
