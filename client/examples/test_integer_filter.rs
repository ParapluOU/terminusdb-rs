use terminusdb_client::{BranchSpec, TerminusDBHttpClient, DocumentInsertArgs};
use terminusdb_schema_derive::TerminusDBModel;
use terminusdb_schema::ToTDBInstance;
use serde::{Serialize, Deserialize};
use std::io::Read;

#[derive(TerminusDBModel, Clone, Debug, Serialize, Deserialize, PartialEq)]
struct TestItem {
    name: String,
    value: i32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing integer filtering...");
    
    let client = TerminusDBHttpClient::local_node().await;
    let test_db = "test_int_filter";
    let spec = BranchSpec::new(test_db);
    
    // Clean up and create test database
    let _ = client.delete_database(test_db).await;
    client.reset_database(test_db).await?;
    
    // Insert schema
    client.insert_schemas::<(TestItem,)>(DocumentInsertArgs::from(spec.clone()).as_schema()).await?;
    
    // Insert test data
    let items = vec![
        TestItem { name: "First".to_string(), value: 100 },
        TestItem { name: "Second".to_string(), value: 200 },
        TestItem { name: "Third".to_string(), value: 100 },
    ];
    
    for item in &items {
        client.insert_instance(item, DocumentInsertArgs::from(spec.clone())).await?;
    }
    
    // First, list all items to verify they were inserted
    println!("\nAll items:");
    let all_items: Vec<TestItem> = client.list_instances(&spec, None, None).await?;
    for item in &all_items {
        println!("  - {}: {}", item.name, item.value);
    }
    
    // Enable query logging to see what query is being sent
    println!("\nEnabling query logging...");
    
    // Test filtering by integer value
    println!("\nFiltering for value = 100...");
    let result: Vec<TestItem> = client.list_instances_where(
        &spec,
        None,
        None,
        vec![("value", 100)],
    ).await?;
    
    // Also check the last query that was executed
    if let Some(last_query) = client.last_query() {
        println!("\nLast query executed:");
        println!("{}", serde_json::to_string_pretty(&last_query).unwrap_or_else(|_| "Failed to serialize".to_string()));
    }
    
    println!("Found {} items with value 100", result.len());
    for item in &result {
        println!("  - {}: {}", item.name, item.value);
    }
    
    if result.len() == 2 {
        println!("\n✅ Integer filtering works correctly!");
    } else {
        println!("\n❌ Integer filtering failed - expected 2 items, got {}", result.len());
    }
    
    // Don't clean up immediately to allow inspection
    println!("\nDatabase '{}' left for inspection. Clean up manually.", test_db);
    // client.delete_database(test_db).await?;
    
    Ok(())
}