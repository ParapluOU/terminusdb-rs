#![cfg(not(target_arch = "wasm32"))]

use std::collections::HashMap;
use terminusdb_client::*;
use terminusdb_woql_builder::prelude::*;
use terminusdb_woql_builder::value::{datetime_literal, date_literal};

#[tokio::test]
#[ignore] // This test requires a running TerminusDB instance
async fn test_date_comparison_simple() -> anyhow::Result<()> {
    println!("\n=== Testing Simple Date Comparisons ===\n");
    
    // Setup client
    let client = TerminusDBHttpClient::local_node_test().await?;
    let spec = BranchSpec::from("test_date_simple");
    
    // Create database if it doesn't exist
    if let Err(_) = client.get_database(&spec).await {
        client.create_database(&spec).await?;
    }
    
    // Create a simple class with date field
    let schema_query = WoqlBuilder::new()
        .doctype("Event")
        .property("name", "xsd:string")
        .property("event_date", "xsd:date")
        .property("event_time", "xsd:dateTime")
        .finalize();
    
    let schema_json = schema_query.to_instance(None).to_json();
    client.query_raw(Some(spec.clone()), schema_json).await?;
    println!("✓ Schema created");
    
    // Insert test data using WOQL
    let insert_query = WoqlBuilder::new()
        .when(
            WoqlBuilder::new()
                .insert("Event/event1", "Event")
                .property("name", "Past Event")
                .property("event_date", date_literal("2020-01-01"))
                .property("event_time", datetime_literal("2020-01-01T10:00:00Z"))
                .finalize()
        )
        .when(
            WoqlBuilder::new()
                .insert("Event/event2", "Event")
                .property("name", "Recent Event")
                .property("event_date", date_literal("2025-01-01"))
                .property("event_time", datetime_literal("2025-01-01T14:30:00Z"))
                .finalize()
        )
        .when(
            WoqlBuilder::new()
                .insert("Event/event3", "Event")
                .property("name", "Future Event")
                .property("event_date", date_literal("2030-12-31"))
                .property("event_time", datetime_literal("2030-12-31T23:59:59Z"))
                .finalize()
        )
        .finalize();
    
    let insert_json = insert_query.to_instance(None).to_json();
    client.query_raw(Some(spec.clone()), insert_json).await?;
    println!("✓ Test data inserted");
    
    // Test 1: Query events with date greater than 2024-01-01
    println!("\nTest 1: Date comparison (greater than 2024-01-01)");
    let (event_id, event_name, event_date_var) = vars!("EventID", "EventName", "EventDate");
    
    let query1 = WoqlBuilder::new()
        .triple(event_id.clone(), "rdf:type", "@schema:Event")
        .triple(event_id.clone(), "name", event_name.clone())
        .triple(event_id.clone(), "event_date", event_date_var.clone())
        .greater(event_date_var.clone(), date_literal("2024-01-01"))
        .select(vec![event_id.clone(), event_name.clone()])
        .finalize();
    
    let json_query1 = query1.to_instance(None).to_json();
    let results1: WOQLResult<HashMap<String, serde_json::Value>> = 
        client.query_raw(Some(spec.clone()), json_query1).await?;
    
    println!("Found {} events after 2024-01-01", results1.bindings.len());
    for binding in &results1.bindings {
        println!("  - {} ({})", 
            binding.get("EventName").and_then(|v| v.as_str()).unwrap_or("?"),
            binding.get("EventID").and_then(|v| v.as_str()).unwrap_or("?")
        );
    }
    
    assert_eq!(results1.bindings.len(), 2, "Should find 2 events after 2024-01-01");
    
    // Test 2: Query events with datetime less than 2025-06-01
    println!("\nTest 2: DateTime comparison (less than 2025-06-01T00:00:00Z)");
    let (event_id2, event_name2, event_time_var) = vars!("EventID2", "EventName2", "EventTime");
    
    let query2 = WoqlBuilder::new()
        .triple(event_id2.clone(), "rdf:type", "@schema:Event")
        .triple(event_id2.clone(), "name", event_name2.clone())
        .triple(event_id2.clone(), "event_time", event_time_var.clone())
        .less(event_time_var.clone(), datetime_literal("2025-06-01T00:00:00Z"))
        .select(vec![event_id2.clone(), event_name2.clone()])
        .finalize();
    
    let json_query2 = query2.to_instance(None).to_json();
    let results2: WOQLResult<HashMap<String, serde_json::Value>> = 
        client.query_raw(Some(spec.clone()), json_query2).await?;
    
    println!("Found {} events before 2025-06-01", results2.bindings.len());
    for binding in &results2.bindings {
        println!("  - {} ({})", 
            binding.get("EventName2").and_then(|v| v.as_str()).unwrap_or("?"),
            binding.get("EventID2").and_then(|v| v.as_str()).unwrap_or("?")
        );
    }
    
    assert_eq!(results2.bindings.len(), 2, "Should find 2 events before 2025-06-01");
    
    // Test 3: Query for exact date match
    println!("\nTest 3: Date equality (equals 2025-01-01)");
    let (event_id3, event_name3, event_date_var3) = vars!("EventID3", "EventName3", "EventDate3");
    
    let query3 = WoqlBuilder::new()
        .triple(event_id3.clone(), "rdf:type", "@schema:Event")
        .triple(event_id3.clone(), "name", event_name3.clone())
        .triple(event_id3.clone(), "event_date", event_date_var3.clone())
        .eq(event_date_var3.clone(), date_literal("2025-01-01"))
        .select(vec![event_id3.clone(), event_name3.clone()])
        .finalize();
    
    let json_query3 = query3.to_instance(None).to_json();
    let results3: WOQLResult<HashMap<String, serde_json::Value>> = 
        client.query_raw(Some(spec.clone()), json_query3).await?;
    
    println!("Found {} events on 2025-01-01", results3.bindings.len());
    for binding in &results3.bindings {
        println!("  - {} ({})", 
            binding.get("EventName3").and_then(|v| v.as_str()).unwrap_or("?"),
            binding.get("EventID3").and_then(|v| v.as_str()).unwrap_or("?")
        );
    }
    
    assert_eq!(results3.bindings.len(), 1, "Should find exactly 1 event on 2025-01-01");
    
    println!("\n✅ All date comparison tests passed!");
    println!("   TerminusDB correctly:");
    println!("   - Compares dates using greater/less operators");
    println!("   - Compares datetimes with timezone information");
    println!("   - Matches exact dates using equality");
    
    Ok(())
}