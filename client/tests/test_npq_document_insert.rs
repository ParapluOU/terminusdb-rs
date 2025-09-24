use terminusdb_client::{TerminusDBHttpClient, DocumentInsertArgs};
use terminusdb_woql2::*;
use terminusdb_woql2::query::{NamedParametricQuery, Query};
use serde_json::json;

#[tokio::test]
#[ignore] // Requires running TerminusDB instance
async fn test_insert_npq_as_document() -> anyhow::Result<()> {
    let client = TerminusDBHttpClient::local_node().await;
    let db_name = "test_npq_doc";
    
    // Create test database
    let _ = client.delete_database(db_name).await;
    let client = client.db(db_name);
    
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
    match client.insert_document(&npq_json, DocumentInsertArgs::default()).await {
        Ok(result) => {
            println!("Successfully inserted NPQ as document!");
            println!("Result client: {:?}", result.instance_read_write_url);
            
            // Now try to call it
            let call_query = call!("simple_test");
            println!("\nAttempting to call the stored query...");
            
            // We need to execute the call as a query
            let call_json = json!({
                "@type": "Call",
                "name": "simple_test",
                "arguments": []
            });
            
            // Try executing through WOQL endpoint
            match client.post_woql(&call_json).await {
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
    
    // Clean up
    let base_client = TerminusDBHttpClient::local_node().await;
    base_client.delete_database(db_name).await?;
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires running TerminusDB instance
async fn test_insert_parametric_npq() -> anyhow::Result<()> {
    let client = TerminusDBHttpClient::local_node().await;
    let db_name = "test_npq_params";
    
    // Create test database
    let _ = client.delete_database(db_name).await;
    let client = client.db(db_name);
    
    // First insert some test data
    let person_data = json!({
        "@type": "InsertDocument",
        "document": [
            {
                "@type": "Person",
                "@id": "Person/alice",
                "name": "Alice"
            },
            {
                "@type": "Person", 
                "@id": "Person/bob",
                "name": "Bob"
            }
        ]
    });
    
    match client.post_woql(&person_data).await {
        Ok(_) => println!("Test data inserted"),
        Err(e) => println!("Failed to insert test data: {:?}", e),
    }
    
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
    
    match client.insert_document(&npq_json, DocumentInsertArgs::default()).await {
        Ok(result) => {
            println!("Successfully inserted parametric NPQ!");
            
            // Try to call with parameters
            let call_json = json!({
                "@type": "Call",
                "name": "find_by_type",
                "arguments": [{"node": "Person"}]
            });
            
            println!("\nCalling parametric query with args: {:#?}", call_json);
            
            match client.post_woql(&call_json).await {
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
    
    // Clean up
    let base_client = TerminusDBHttpClient::local_node().await;
    base_client.delete_database(db_name).await?;
    
    Ok(())
}