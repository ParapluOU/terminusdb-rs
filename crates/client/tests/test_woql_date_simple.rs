#![cfg(not(target_arch = "wasm32"))]

use std::collections::HashMap;
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::*;
use terminusdb_woql_builder::prelude::*;
use terminusdb_woql_builder::value::{datetime_literal, date_literal};
use terminusdb_schema::{ToJson, ToTDBInstance};
use serde_json::json;

#[tokio::test]
async fn test_date_comparison_simple() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_date_simple", |client, spec| async move {
            println!("\n=== Testing Simple Date Comparisons ===\n");

            // Create a simple schema with date field using raw JSON
            let schema_json = json!({
                "@type": "Class",
                "@id": "Event",
                "name": "xsd:string",
                "event_date": "xsd:date",
                "event_time": "xsd:dateTime"
            });

            // Insert schema using insert_document with schema type
            let schema_args = DocumentInsertArgs::from(spec.clone()).as_schema();
            client.insert_document(&schema_json, schema_args).await?;
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
            let args = DocumentInsertArgs::from(spec.clone());
            let event_refs: Vec<&serde_json::Value> = events.iter().collect();
            client.insert_documents(event_refs, args).await?;
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
                client.query_raw(Some(spec.clone()), json_query1, None).await?;

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
                client.query_raw(Some(spec.clone()), json_query2, None).await?;

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
                client.query_raw(Some(spec.clone()), json_query3, None).await?;

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
        })
        .await
}
