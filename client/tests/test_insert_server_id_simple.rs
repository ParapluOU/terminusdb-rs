#![cfg(not(target_arch = "wasm32"))]
#![cfg(test)]

use anyhow::Result;
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::TerminusDBModel;

// Simple test model with ServerIDFor
#[derive(Clone, Debug, Default, TerminusDBModel, serde::Serialize, serde::Deserialize)]
#[tdb(key = "lexical", key_fields = "name", id_field = "id")]
pub struct SimpleModel {
    pub id: ServerIDFor<Self>,
    pub name: String,
}

#[tokio::test]
async fn test_simple_insert_with_server_id() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_simple_server_id", |client, spec| async move {
            // Insert schema
            client
                .insert_schemas::<(SimpleModel,)>(spec.clone().into())
                .await?;

            // Create a model with no ID
            let model = SimpleModel {
                id: ServerIDFor::new(),
                name: "test_name".to_string(),
            };

            // Verify ID is None initially
            assert!(model.id.is_none());

            let args = DocumentInsertArgs {
                message: "Insert simple model".to_string(),
                author: "test".to_string(),
                spec: spec.clone(),
                ..Default::default()
            };

            // Just test the insert - don't retrieve yet
            let (result, commit_id) = client
                .insert_instance_with_commit_id(&model, args.clone())
                .await?;

            println!("Insert result: {:?}", result);
            println!("Commit ID: {}", commit_id);

            // The insert should succeed and return an ID
            assert!(result.root_id.contains("SimpleModel/"));
            assert!(!commit_id.is_empty());

            println!("✓ Successfully inserted model with server-generated ID");

            // Now try to get the raw document to see what the server returns
            let doc_json = client
                .get_document("SimpleModel/test_name", &spec, GetOpts::default())
                .await?;
            println!(
                "Retrieved document JSON: {}",
                serde_json::to_string_pretty(&doc_json)?
            );

            // Check if the id field is present in the JSON
            if let Some(id_value) = doc_json.get("id") {
                println!("ID field in JSON: {:?}", id_value);
            } else {
                println!("No 'id' field found in JSON");
            }

            // Now test the insert_and_retrieve method
            let model2 = SimpleModel {
                id: ServerIDFor::new(),
                name: "test_name2".to_string(),
            };

            println!("\nTesting insert_and_retrieve...");
            match client.insert_instance_and_retrieve(&model2, args).await {
                Ok((retrieved_model, commit_id)) => {
                    println!("✓ insert_and_retrieve succeeded!");
                    println!("Retrieved model: {:?}", retrieved_model);
                    println!("Model ID is populated: {}", retrieved_model.id.is_some());
                    if let Some(id) = retrieved_model.id.as_ref() {
                        println!("ID value: {}", id.id());
                    }
                    println!("Commit ID: {}", commit_id);
                }
                Err(e) => {
                    println!("✗ insert_and_retrieve failed: {}", e);
                    return Err(e.into());
                }
            }

            Ok(())
        })
        .await
}
