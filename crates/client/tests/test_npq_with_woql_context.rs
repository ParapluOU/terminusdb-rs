use terminusdb_bin::TerminusDBServer;
use terminusdb_client::{TerminusDBHttpClient, BranchSpec};
use serde_json::{json, Value};

#[tokio::test]
async fn test_insert_npq_with_woql_context() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_npq_context", |client, spec| async move {
            // Create NPQ with proper WOQL context
            let npq_with_context = json!({
                "@context": {
                    "@base": "terminusdb://woql/data/",
                    "@schema": "http://terminusdb.com/schema/woql#",
                    "xsd": "http://www.w3.org/2001/XMLSchema#"
                },
                "@type": "NamedParametricQuery",
                "@id": "simple_test",
                "name": "simple_test",
                "parameters": [],
                "query": {
                    "@type": "True"
                }
            });

            println!("NPQ with context: {}", serde_json::to_string_pretty(&npq_with_context)?);

            // Try to insert as WOQL query
            match client.query_string::<Value>(Some(spec.clone()), &serde_json::to_string(&npq_with_context)?, None).await {
                Ok(result) => {
                    println!("Successfully executed NPQ definition!");
                    println!("Result: {:?}", result);
                }
                Err(e) => {
                    println!("Failed to execute NPQ definition: {:?}", e);
                }
            }

            // Try a parametric query
            let param_npq_with_context = json!({
                "@context": {
                    "@base": "terminusdb://woql/data/",
                    "@schema": "http://terminusdb.com/schema/woql#",
                    "xsd": "http://www.w3.org/2001/XMLSchema#"
                },
                "@type": "NamedParametricQuery",
                "@id": "find_by_type",
                "name": "find_by_type",
                "parameters": ["type_var"],
                "query": {
                    "@type": "Triple",
                    "subject": {
                        "@type": "NodeValue",
                        "variable": "x"
                    },
                    "predicate": {
                        "@type": "NodeValue",
                        "node": "rdf:type"
                    },
                    "object": {
                        "@type": "NodeValue",
                        "variable": "type_var"
                    }
                }
            });

            println!("\n\nParametric NPQ with context: {}", serde_json::to_string_pretty(&param_npq_with_context)?);

            match client.query_string::<Value>(Some(spec.clone()), &serde_json::to_string(&param_npq_with_context)?, None).await {
                Ok(result) => {
                    println!("Successfully executed parametric NPQ definition!");
                    println!("Result: {:?}", result);
                }
                Err(e) => {
                    println!("Failed to execute parametric NPQ definition: {:?}", e);
                }
            }

            // Now try to call the stored query
            let call_query = json!({
                "@context": {
                    "@base": "terminusdb://woql/data/",
                    "@schema": "http://terminusdb.com/schema/woql#",
                    "xsd": "http://www.w3.org/2001/XMLSchema#"
                },
                "@type": "Call",
                "name": "simple_test",
                "arguments": []
            });

            println!("\n\nAttempting to call stored query...");
            match client.query_string::<Value>(Some(spec.clone()), &serde_json::to_string(&call_query)?, None).await {
                Ok(result) => {
                    println!("Call succeeded! Result: {:?}", result);
                }
                Err(e) => {
                    println!("Call failed: {:?}", e);
                }
            }

            Ok(())
        })
        .await
}
