#![cfg(not(target_arch = "wasm32"))]

use serde_json::json;
use std::collections::HashMap;
use terminusdb_client::*;

#[tokio::test]
#[ignore] // This test requires a running TerminusDB instance
async fn test_date_comparison_queries_raw() -> anyhow::Result<()> {
    println!("\n=== Testing Date Comparisons with Raw WOQL ===\n");
    
    // Setup client
    let client = TerminusDBHttpClient::local_node_test().await?;
    let spec = BranchSpec::from("test_date_comparisons_raw");
    
    // Create a simple schema with date fields using raw JSON
    let schema_json = json!({
        "@type": "Class",
        "@id": "Event",
        "name": "xsd:string",
        "event_date": "xsd:date",
        "event_time": "xsd:dateTime"
    });
    
    // Insert schema using post_schema
    let path = format!("api/db/{}/{}/schema", spec.org, spec.db);
    let response = client
        .client
        .post(client.url(&path))
        .json(&vec![schema_json])
        .send()
        .await?;
    
    if !response.status().is_success() {
        let error_text = response.text().await?;
        anyhow::bail!("Failed to create schema: {}", error_text);
    }
    
    println!("✓ Schema created");
    
    // Insert test data using raw JSON
    let events = vec![
        json!({
            "@type": "Event",
            "@id": "Event/event1",
            "name": "Past Event",
            "event_date": "2020-01-01",
            "event_time": "2020-01-01T10:00:00Z"
        }),
        json!({
            "@type": "Event",
            "@id": "Event/event2",
            "name": "Recent Event",
            "event_date": "2025-01-01",
            "event_time": "2025-01-01T14:30:00Z"
        }),
        json!({
            "@type": "Event",
            "@id": "Event/event3",
            "name": "Future Event",
            "event_date": "2030-12-31",
            "event_time": "2030-12-31T23:59:59Z"
        }),
    ];
    
    // Insert events using insert_documents
    let path = format!("api/db/{}/{}/document", spec.org, spec.db);
    let response = client
        .client
        .post(client.url(&path))
        .json(&events)
        .send()
        .await?;
    
    if !response.status().is_success() {
        let error_text = response.text().await?;
        anyhow::bail!("Failed to insert documents: {}", error_text);
    }
    
    println!("✓ Test data inserted");
    
    // Test 1: Query events with date greater than 2024-01-01
    println!("\nTest 1: Date comparison (greater than 2024-01-01)");
    let query1 = json!({
        "@type": "Select",
        "variables": ["EventID", "EventName", "EventDate"],
        "query": {
            "@type": "And",
            "and": [
                {
                    "@type": "Triple",
                    "subject": {"@type": "NodeValue", "variable": "EventID"},
                    "predicate": {"@type": "NodeValue", "node": "rdf:type"},
                    "object": {"@type": "Value", "node": "@schema:Event"}
                },
                {
                    "@type": "Triple",
                    "subject": {"@type": "NodeValue", "variable": "EventID"},
                    "predicate": {"@type": "NodeValue", "node": "name"},
                    "object": {"@type": "Value", "variable": "EventName"}
                },
                {
                    "@type": "Triple",
                    "subject": {"@type": "NodeValue", "variable": "EventID"},
                    "predicate": {"@type": "NodeValue", "node": "event_date"},
                    "object": {"@type": "Value", "variable": "EventDate"}
                },
                {
                    "@type": "Greater",
                    "left": {"@type": "DataValue", "variable": "EventDate"},
                    "right": {"@type": "DataValue", "data": {"@type": "xsd:date", "@value": "2024-01-01"}}
                }
            ]
        }
    });
    
    let results1: WOQLResult<HashMap<String, serde_json::Value>> = 
        client.query_raw(Some(spec.clone()), query1).await?;
    
    println!("Found {} events after 2024-01-01", results1.bindings.len());
    for binding in &results1.bindings {
        println!("  - {} ({}): {}", 
            binding.get("EventName").and_then(|v| v.as_str()).unwrap_or("?"),
            binding.get("EventID").and_then(|v| v.as_str()).unwrap_or("?"),
            binding.get("EventDate").and_then(|v| v.as_str()).unwrap_or("?")
        );
    }
    
    assert_eq!(results1.bindings.len(), 2, "Should find 2 events after 2024-01-01");
    
    // Test 2: Query events with datetime less than 2025-06-01
    println!("\nTest 2: DateTime comparison (less than 2025-06-01T00:00:00Z)");
    let query2 = json!({
        "@type": "Select",
        "variables": ["EventID", "EventName", "EventTime"],
        "query": {
            "@type": "And",
            "and": [
                {
                    "@type": "Triple",
                    "subject": {"@type": "NodeValue", "variable": "EventID"},
                    "predicate": {"@type": "NodeValue", "node": "rdf:type"},
                    "object": {"@type": "Value", "node": "@schema:Event"}
                },
                {
                    "@type": "Triple",
                    "subject": {"@type": "NodeValue", "variable": "EventID"},
                    "predicate": {"@type": "NodeValue", "node": "name"},
                    "object": {"@type": "Value", "variable": "EventName"}
                },
                {
                    "@type": "Triple",
                    "subject": {"@type": "NodeValue", "variable": "EventID"},
                    "predicate": {"@type": "NodeValue", "node": "event_time"},
                    "object": {"@type": "Value", "variable": "EventTime"}
                },
                {
                    "@type": "Less",
                    "left": {"@type": "DataValue", "variable": "EventTime"},
                    "right": {"@type": "DataValue", "data": {"@type": "xsd:dateTime", "@value": "2025-06-01T00:00:00Z"}}
                }
            ]
        }
    });
    
    let results2: WOQLResult<HashMap<String, serde_json::Value>> = 
        client.query_raw(Some(spec.clone()), query2).await?;
    
    println!("Found {} events before 2025-06-01", results2.bindings.len());
    for binding in &results2.bindings {
        println!("  - {} ({}): {}", 
            binding.get("EventName").and_then(|v| v.as_str()).unwrap_or("?"),
            binding.get("EventID").and_then(|v| v.as_str()).unwrap_or("?"),
            binding.get("EventTime").and_then(|v| v.as_str()).unwrap_or("?")
        );
    }
    
    assert_eq!(results2.bindings.len(), 2, "Should find 2 events before 2025-06-01");
    
    // Test 3: Query for exact date match
    println!("\nTest 3: Date equality (equals 2025-01-01)");
    let query3 = json!({
        "@type": "Select",
        "variables": ["EventID", "EventName", "EventDate"],
        "query": {
            "@type": "And",
            "and": [
                {
                    "@type": "Triple",
                    "subject": {"@type": "NodeValue", "variable": "EventID"},
                    "predicate": {"@type": "NodeValue", "node": "rdf:type"},
                    "object": {"@type": "Value", "node": "@schema:Event"}
                },
                {
                    "@type": "Triple",
                    "subject": {"@type": "NodeValue", "variable": "EventID"},
                    "predicate": {"@type": "NodeValue", "node": "name"},
                    "object": {"@type": "Value", "variable": "EventName"}
                },
                {
                    "@type": "Triple",
                    "subject": {"@type": "NodeValue", "variable": "EventID"},
                    "predicate": {"@type": "NodeValue", "node": "event_date"},
                    "object": {"@type": "Value", "variable": "EventDate"}
                },
                {
                    "@type": "Equals",
                    "left": {"@type": "Value", "variable": "EventDate"},
                    "right": {"@type": "Value", "data": {"@type": "xsd:date", "@value": "2025-01-01"}}
                }
            ]
        }
    });
    
    let results3: WOQLResult<HashMap<String, serde_json::Value>> = 
        client.query_raw(Some(spec.clone()), query3).await?;
    
    println!("Found {} events on 2025-01-01", results3.bindings.len());
    for binding in &results3.bindings {
        println!("  - {} ({}): {}", 
            binding.get("EventName").and_then(|v| v.as_str()).unwrap_or("?"),
            binding.get("EventID").and_then(|v| v.as_str()).unwrap_or("?"),
            binding.get("EventDate").and_then(|v| v.as_str()).unwrap_or("?")
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