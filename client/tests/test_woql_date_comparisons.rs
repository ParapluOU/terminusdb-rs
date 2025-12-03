#![cfg(not(target_arch = "wasm32"))]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use terminusdb_woql_builder::prelude::*;
use terminusdb_woql_builder::value::{datetime_literal, date_literal};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Event {
    id: EntityIDFor<Self>,
    name: String,
    event_date: String, // Store date as string in ISO format
    event_time: DateTime<Utc>,
}

#[tokio::test]
async fn test_date_comparison_queries() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_date_comparisons", |client, spec| async move {
            // Insert schema
            let args = DocumentInsertArgs::from(spec.clone());
            client.insert_entity_schema::<Event>(args).await?;

            // Insert test data with different dates
            let events = vec![
                Event {
                    id: EntityIDFor::new("event1").unwrap(),
                    name: "Past Event".to_string(),
                    event_date: "2020-01-01".to_string(),
                    event_time: DateTime::parse_from_rfc3339("2020-01-01T10:00:00Z").unwrap().with_timezone(&Utc),
                },
                Event {
                    id: EntityIDFor::new("event2").unwrap(),
                    name: "Recent Event".to_string(),
                    event_date: "2025-01-01".to_string(),
                    event_time: DateTime::parse_from_rfc3339("2025-01-01T14:30:00Z").unwrap().with_timezone(&Utc),
                },
                Event {
                    id: EntityIDFor::new("event3").unwrap(),
                    name: "Future Event".to_string(),
                    event_date: "2030-12-31".to_string(),
                    event_time: DateTime::parse_from_rfc3339("2030-12-31T23:59:59Z").unwrap().with_timezone(&Utc),
                },
            ];

            // Insert events
            for event in &events {
                let args = DocumentInsertArgs::from(spec.clone());
                client.create_instance(event, args).await?;
            }

            // Test 1: Query events with date greater than 2024-01-01
            let (event_id, event_date_var) = vars!("EventID", "EventDate");
            let date_cutoff = date_literal("2024-01-01");

            let query1 = WoqlBuilder::new()
                .triple(event_id.clone(), "event_date", event_date_var.clone())
                .greater(event_date_var.clone(), date_cutoff)
                .select(vec![event_id.clone()])
                .finalize();

            let json_query1 = query1.to_instance(None).to_json();
            let results1: WOQLResult<HashMap<String, serde_json::Value>> =
                client.query_raw(Some(spec.clone()), json_query1, None).await?;

            println!("Query 1 returned {} results", results1.bindings.len());

            // Should return event2 and event3
            let result_ids: Vec<String> = results1.bindings
                .iter()
                .filter_map(|binding| binding.get("EventID")?.as_str().map(|s| s.to_string()))
                .collect();

            println!("Query 1 results: {:?}", result_ids);
            assert_eq!(result_ids.len(), 2);
            assert!(result_ids.iter().any(|id| id.contains("event2")));
            assert!(result_ids.iter().any(|id| id.contains("event3")));

            // Test 2: Query events with datetime less than 2025-06-01T00:00:00Z
            let (event_id2, event_time_var) = vars!("EventID2", "EventTime");
            let datetime_cutoff = datetime_literal("2025-06-01T00:00:00Z");

            let query2 = WoqlBuilder::new()
                .triple(event_id2.clone(), "event_time", event_time_var.clone())
                .less(event_time_var.clone(), datetime_cutoff)
                .select(vec![event_id2.clone()])
                .finalize();

            let json_query2 = query2.to_instance(None).to_json();
            let results2: WOQLResult<HashMap<String, serde_json::Value>> =
                client.query_raw(Some(spec.clone()), json_query2, None).await?;

            println!("Query 2 returned {} results", results2.bindings.len());

            // Should return event1 and event2
            let result_ids2: Vec<String> = results2.bindings
                .iter()
                .filter_map(|binding| binding.get("EventID2")?.as_str().map(|s| s.to_string()))
                .collect();

            println!("Query 2 results: {:?}", result_ids2);
            assert_eq!(result_ids2.len(), 2);
            assert!(result_ids2.iter().any(|id| id.contains("event1")));
            assert!(result_ids2.iter().any(|id| id.contains("event2")));

            // Test 3: Query for exact date match
            let (event_id3, event_date_var3) = vars!("EventID3", "EventDate3");
            let exact_date = date_literal("2025-01-01");

            let query3 = WoqlBuilder::new()
                .triple(event_id3.clone(), "event_date", event_date_var3.clone())
                .eq(event_date_var3.clone(), exact_date)
                .select(vec![event_id3.clone()])
                .finalize();

            let json_query3 = query3.to_instance(None).to_json();
            let results3: WOQLResult<HashMap<String, serde_json::Value>> =
                client.query_raw(Some(spec.clone()), json_query3, None).await?;

            println!("Query 3 returned {} results", results3.bindings.len());

            // Should return only event2
            let result_ids3: Vec<String> = results3.bindings
                .iter()
                .filter_map(|binding| binding.get("EventID3")?.as_str().map(|s| s.to_string()))
                .collect();

            println!("Query 3 results: {:?}", result_ids3);
            assert_eq!(result_ids3.len(), 1);
            assert!(result_ids3[0].contains("event2"));

            println!("\n✅ All date comparison tests passed!");

            Ok(())
        })
        .await
}
