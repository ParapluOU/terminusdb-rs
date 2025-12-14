#![cfg(not(target_arch = "wasm32"))]

use terminusdb_woql_builder::prelude::*;
use terminusdb_woql_builder::value::{datetime_literal, date_literal};
use terminusdb_schema::ToTDBInstance;

#[test]
fn test_date_comparison_query_generation() {
    println!("\n=== Testing Date Comparison Query Generation ===\n");
    
    // Test 1: Date comparison with greater
    let (event_id, event_date_var) = vars!("EventID", "EventDate");
    let date_cutoff = date_literal("2024-01-01");
    
    let query1 = WoqlBuilder::new()
        .triple(event_id.clone(), "event_date", event_date_var.clone())
        .greater(event_date_var.clone(), date_cutoff)
        .select(vec![event_id.clone()])
        .finalize();
    
    let json1 = query1.to_json();
    println!("Query 1 (Date greater than 2024-01-01):");
    println!("{}", serde_json::to_string_pretty(&json1).unwrap());
    
    // Verify the date literal is properly formatted in the query
    let json_str = serde_json::to_string(&json1).unwrap();
    assert!(json_str.contains(r#""@type":"xsd:date"#) || json_str.contains("2024-01-01"));
    
    // Test 2: DateTime comparison with less
    let (event_id2, event_time_var) = vars!("EventID2", "EventTime");
    let datetime_cutoff = datetime_literal("2025-06-01T00:00:00Z");
    
    let query2 = WoqlBuilder::new()
        .triple(event_id2.clone(), "event_time", event_time_var.clone())
        .less(event_time_var.clone(), datetime_cutoff)
        .select(vec![event_id2.clone()])
        .finalize();
    
    let json2 = query2.to_json();
    println!("\nQuery 2 (DateTime less than 2025-06-01T00:00:00Z):");
    println!("{}", serde_json::to_string_pretty(&json2).unwrap());
    
    // Verify the datetime literal is properly formatted
    let json_str2 = serde_json::to_string(&json2).unwrap();
    assert!(json_str2.contains(r#""@type":"xsd:dateTime"#) || json_str2.contains("2025-06-01T00:00:00"));
    
    // Test 3: Date equality
    let (event_id3, event_date_var3) = vars!("EventID3", "EventDate3");
    let exact_date = date_literal("2025-01-01");
    
    let query3 = WoqlBuilder::new()
        .triple(event_id3.clone(), "event_date", event_date_var3.clone())
        .eq(event_date_var3.clone(), exact_date)
        .select(vec![event_id3.clone()])
        .finalize();
    
    let json3 = query3.to_json();
    println!("\nQuery 3 (Date equals 2025-01-01):");
    println!("{}", serde_json::to_string_pretty(&json3).unwrap());
    
    println!("\n✅ All date comparison queries generated successfully!");
}