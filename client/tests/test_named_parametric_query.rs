use terminusdb_client::TerminusDBHttpClient;
use terminusdb_woql2::*;
use terminusdb_woql2::query::{NamedParametricQuery, Query};
use terminusdb_schema::ToTDBSchema;
use serde_json::json;

#[tokio::test]
#[ignore] // Requires running TerminusDB instance
async fn test_insert_named_parametric_query() -> anyhow::Result<()> {
    let client = TerminusDBHttpClient::local_node().await;
    let db_name = "test_npq";
    
    // Clean up and create database
    let _ = client.delete_database(db_name).await;
    client.db(db_name);
    
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
    
    // Try to insert as raw JSON since NPQ doesn't implement ToTDBInstance
    let insert_query = json!({
        "@type": "InsertDocument",
        "document": json_value
    });
    
    match client.execute_query(db_name, &insert_query).await {
        Ok(result) => {
            println!("Successfully inserted NPQ!");
            println!("Result: {:?}", result);
        }
        Err(e) => {
            println!("Failed to insert NPQ: {:?}", e);
        }
    }
    
    // Try to call the named query
    let call_query = call!("test_query");
    println!("\nTrying to call the query...");
    match client.execute_query(db_name, &call_query).await {
        Ok(result) => {
            println!("Call succeeded! Result: {:?}", result);
        }
        Err(e) => {
            println!("Call failed: {:?}", e);
        }
    }
    
    // Clean up
    client.delete_database(db_name).await?;
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires running TerminusDB instance  
async fn test_parametric_query_with_params() -> anyhow::Result<()> {
    let client = TerminusDBHttpClient::local_node().await;
    let db_name = "test_npq_params";
    
    // Clean up if exists
    let _ = client.delete_database(db_name).await;
    
    // Create database
    client.create_database(db_name, "Test NPQ Params", "Testing parametric queries").await?;
    
    // Create a parametric query with one parameter
    let npq = NamedParametricQuery {
        name: "find_by_type".to_string(),
        parameters: vec!["type".to_string()],
        query: triple!(var!(x), "rdf:type", var!(type)),
    };
    
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
    
    let insert_query = json!({
        "@type": "InsertDocument",
        "document": json_npq
    });
    
    match client.execute_query(db_name, &insert_query).await {
        Ok(result) => {
            println!("Successfully inserted parametric query!");
            println!("Result: {:?}", result);
            
            // Now try to call it with an argument
            let call_query = call!("find_by_type", [node!("Person")]);
            match client.execute_query(db_name, &call_query).await {
                Ok(result) => println!("Call with params succeeded: {:?}", result),
                Err(e) => println!("Call with params failed: {:?}", e),
            }
        }
        Err(e) => {
            println!("Failed to insert parametric query: {:?}", e);
        }
    }
    
    // Clean up
    client.delete_database(db_name).await?;
    
    Ok(())
}