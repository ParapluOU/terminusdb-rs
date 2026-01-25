use serde_json::{json, Value};
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::{BranchSpec, DocumentInsertArgs, TerminusDBHttpClient};
use terminusdb_woql2::query::{NamedParametricQuery, Query};
use terminusdb_woql2::*;

#[tokio::test]
async fn test_insert_npq_as_document() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_npq_doc", |client, spec| async move {
            // Create a simple NPQ
            let npq_json = json!({
                "@type": "NamedParametricQuery",
                "@id": "NamedParametricQuery/simple_test",
                "name": "simple_test",
                "parameters": [],
                "query": {
                    "@type": "True"
                }
            });

            println!("Attempting to insert NPQ as document: {:#?}", npq_json);

            // Try to insert as a document
            let args = DocumentInsertArgs::from(spec.clone());
            match client.insert_document(&npq_json, args).await {
                Ok(result) => {
                    println!("Successfully inserted NPQ as document!");
                    println!("Result: {:?}", result);

                    // Now try to call it
                    let call_json = json!({
                        "@type": "Call",
                        "name": "simple_test",
                        "arguments": []
                    });

                    // Try executing through query_string
                    match client
                        .query_string::<Value>(
                            Some(spec.clone()),
                            &serde_json::to_string(&call_json)?,
                            None,
                        )
                        .await
                    {
                        Ok(response) => {
                            println!("Call succeeded! Response: {:?}", response);
                        }
                        Err(e) => {
                            println!("Call failed: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("Failed to insert NPQ as document: {:?}", e);
                }
            }

            Ok(())
        })
        .await
}

#[tokio::test]
async fn test_insert_parametric_npq() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_npq_params", |client, spec| async move {
            // Create a parametric query
            let npq_json = json!({
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

            println!("\nInserting parametric NPQ: {:#?}", npq_json);

            let args = DocumentInsertArgs::from(spec.clone());
            match client.insert_document(&npq_json, args).await {
                Ok(result) => {
                    println!("Successfully inserted parametric NPQ!");

                    // Try to call with parameters
                    let call_json = json!({
                        "@type": "Call",
                        "name": "find_by_type",
                        "arguments": [{"node": "Person"}]
                    });

                    println!("\nCalling parametric query with args: {:#?}", call_json);

                    match client
                        .query_string::<Value>(
                            Some(spec.clone()),
                            &serde_json::to_string(&call_json)?,
                            None,
                        )
                        .await
                    {
                        Ok(response) => {
                            println!("Parametric call succeeded! Response: {:?}", response);
                        }
                        Err(e) => {
                            println!("Parametric call failed: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("Failed to insert parametric NPQ: {:?}", e);
                }
            }

            Ok(())
        })
        .await
}
