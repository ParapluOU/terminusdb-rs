#![cfg(not(target_arch = "wasm32"))]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use terminusdb_woql_builder::prelude::*;
use terminusdb_woql_builder::value::datetime_literal;

#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Event {
    id: EntityIDFor<Self>,
    name: String,
    event_date: String,
    event_time: DateTime<Utc>,
}

#[tokio::test]
async fn test_debug_datetime_storage() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_date_debug", |client, spec| async move {
            println!("\n=== Debugging DateTime Storage ===\n");

            // Insert schema
            let args = DocumentInsertArgs::from(spec.clone());
            client.insert_entity_schema::<Event>(args).await?;
            println!("✓ Schema inserted");

            // Insert one event
            let event = Event {
                id: EntityIDFor::new("test_event").unwrap(),
                name: "Test Event".to_string(),
                event_date: "2025-01-01".to_string(),
                event_time: DateTime::parse_from_rfc3339("2025-01-01T14:30:00Z").unwrap().with_timezone(&Utc),
            };

            println!("\nInserting event with DateTime: {}", event.event_time);

            let args = DocumentInsertArgs::from(spec.clone());
            client.create_instance(&event, args).await?;
            println!("✓ Event inserted");

            // Query to see what was actually stored
            println!("\nQuerying all Event data...");
            let (id, name, date, time) = vars!("ID", "Name", "Date", "Time");

            let query = WoqlBuilder::new()
                .triple(id.clone(), "rdf:type", "@schema:Event")
                .triple(id.clone(), "name", name.clone())
                .triple(id.clone(), "event_date", date.clone())
                .triple(id.clone(), "event_time", time.clone())
                .select(vec![id.clone(), name.clone(), date.clone(), time.clone()])
                .finalize();

            let json_query = query.to_json();
            let results: WOQLResult<HashMap<String, serde_json::Value>> =
                client.query_raw(Some(spec.clone()), json_query, None).await?;

            println!("\nQuery results:");
            for binding in &results.bindings {
                println!("  ID: {}", binding.get("ID").and_then(|v| v.as_str()).unwrap_or("?"));
                println!("  Name: {}", binding.get("Name").and_then(|v| v.as_str()).unwrap_or("?"));
                println!("  Date: {}", binding.get("Date").and_then(|v| v.as_str()).unwrap_or("?"));
                println!("  Time: {:?}", binding.get("Time"));
            }

            // Try different comparison approaches
            println!("\n\nTesting DateTime comparisons:");

            // Test 1: Compare with datetime literal (ISO 8601 format)
            println!("\n1. DateTime literal (ISO 8601 Z suffix):");
            let (id1, time1) = vars!("ID1", "Time1");
            let query1 = WoqlBuilder::new()
                .triple(id1.clone(), "event_time", time1.clone())
                .less(time1.clone(), datetime_literal("2025-06-01T00:00:00Z"))
                .select(vec![id1.clone()])
                .finalize();

            let results1: WOQLResult<HashMap<String, serde_json::Value>> =
                client.query_raw(Some(spec.clone()), query1.to_json(), None).await?;
            println!("  Found {} results with ISO 8601 comparison", results1.bindings.len());

            // Test 2: Compare with full RFC3339 (offset format)
            println!("\n2. DateTime literal (RFC3339 offset format):");
            let (id2, time2) = vars!("ID2", "Time2");
            let query2 = WoqlBuilder::new()
                .triple(id2.clone(), "event_time", time2.clone())
                .less(time2.clone(), datetime_literal("2025-06-01T00:00:00+00:00"))
                .select(vec![id2.clone()])
                .finalize();

            let results2: WOQLResult<HashMap<String, serde_json::Value>> =
                client.query_raw(Some(spec.clone()), query2.to_json(), None).await?;
            println!("  Found {} results with RFC3339 offset comparison", results2.bindings.len());

            // Test 3: Compare with datetime literal (another format)
            println!("\n3. DateTime literal (different timestamp):");
            let (id3, time3) = vars!("ID3", "Time3");
            let query3 = WoqlBuilder::new()
                .triple(id3.clone(), "event_time", time3.clone())
                .less(time3.clone(), datetime_literal("2025-06-01T00:00:00Z"))
                .select(vec![id3.clone()])
                .finalize();

            let json3 = query3.to_json();
            println!("  Query JSON: {}", serde_json::to_string_pretty(&json3).unwrap());

            let results3: WOQLResult<HashMap<String, serde_json::Value>> =
                client.query_raw(Some(spec.clone()), json3, None).await?;
            println!("  Found {} results with datetime literal", results3.bindings.len());

            Ok(())
        })
        .await
}
