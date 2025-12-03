use terminusdb_bin::TerminusDBServer;
use terminusdb_client::{TerminusDBHttpClient, DocumentInsertArgs, BranchSpec};
use terminusdb_woql2::*;
use terminusdb_woql2::query::{NamedParametricQuery, Query};
use serde_json::{json, Value};

#[tokio::test]
async fn test_insert_npq_raw() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_npq_raw", |client, spec| async move {
            // Create a simple NPQ using Rust struct
            let npq = NamedParametricQuery {
                name: "simple_test".to_string(),
                parameters: vec![],
                query: Query::True(query::True {}),
            };

            println!("NPQ struct: {:?}", npq);

            // Insert as document
            println!("\nInserting NPQ as document...");

            let args = DocumentInsertArgs::from(spec.clone());

            match client.insert_instance(&npq, args).await {
                Ok(result) => {
                    println!("Successfully inserted NPQ!");
                    println!("Inserted NPQ with ID: {}", result.root_id);

                    // Now try to call it
                    let call_query = call!("simple_test");
                    println!("\nCreated call query: {:?}", call_query);

                    // Convert call to JSON and try to execute it
                    let call_json: Value = serde_json::to_value(&call_query)?;
                    println!("Call as JSON: {}", serde_json::to_string_pretty(&call_json)?);

                    // Note: We'd need to execute this through the query endpoint
                    // For now, just demonstrate the structure is correct
                }
                Err(e) => {
                    println!("Failed to insert NPQ: {:?}", e);
                }
            }

            // Try with a parametric query
            println!("\n\n--- Testing parametric query ---");

            let param_npq = NamedParametricQuery {
                name: "find_by_type".to_string(),
                parameters: vec!["type_var".to_string()],
                query: triple!(var!(x), "rdf:type", var!(type_var)),
            };

            println!("Parametric NPQ struct: {:?}", param_npq);

            let args2 = DocumentInsertArgs::from(spec.clone());

            match client.insert_instance(&param_npq, args2).await {
                Ok(result) => {
                    println!("\nSuccessfully inserted parametric NPQ!");

                    // Create a call with arguments
                    let call_with_args = call!("find_by_type", [node!("Person")]);
                    let call_json: Value = serde_json::to_value(&call_with_args)?;
                    println!("\nCall with args as JSON: {}", serde_json::to_string_pretty(&call_json)?);
                }
                Err(e) => {
                    println!("Failed to insert parametric NPQ: {:?}", e);
                }
            }

            Ok(())
        })
        .await
}
