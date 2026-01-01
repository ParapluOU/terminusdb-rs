#![cfg(not(target_arch = "wasm32"))]

use std::collections::HashMap;
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::*;
use terminusdb_woql2::prelude::*;
use terminusdb_schema::{ToTDBInstance, ToJson, EntityIDFor};
use serde::{Deserialize, Serialize};
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

// Test models for our integration tests

#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Document {
    id: EntityIDFor<Self>,
    title: String,
    content: String,
    created_date: String, // ISO 8601 date string
}

#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct User {
    id: EntityIDFor<Self>,
    name: String,
    email: String,
    registration_date: String,
}

#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Subscription {
    id: EntityIDFor<Self>,
    name: String,
    start_date: String,
    end_date: String,
    status: String,
}

#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Event {
    id: EntityIDFor<Self>,
    title: String,
    event_date: String,
    status: String,
    attendees: i32,
}

// Helper function to setup test schemas
async fn setup_test_schemas(client: &TerminusDBHttpClient, spec: &BranchSpec) -> anyhow::Result<()> {
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<Document>(args.clone()).await?;
    client.insert_entity_schema::<User>(args.clone()).await?;
    client.insert_entity_schema::<Subscription>(args.clone()).await?;
    client.insert_entity_schema::<Event>(args.clone()).await?;
    Ok(())
}

// String operation macro tests

#[tokio::test]
async fn test_starts_with_macro() -> anyhow::Result<()> {
    println!("\n=== Testing starts_with! macro ===\n");

    let server = TerminusDBServer::test_instance().await?;

    server.with_tmp_db("test_starts_with", |client, spec| async move {
        setup_test_schemas(&client, &spec).await?;

        // Insert test documents
        let docs = vec![
            Document {
                id: EntityIDFor::new("doc1").unwrap(),
                title: "DOC-2024-001: Quarterly Report".to_string(),
                content: "Financial results for Q1".to_string(),
                created_date: "2024-03-15T10:00:00Z".to_string(),
            },
            Document {
                id: EntityIDFor::new("doc2").unwrap(),
                title: "DOC-2024-002: Budget Proposal".to_string(),
                content: "Budget for next fiscal year".to_string(),
                created_date: "2024-04-01T14:30:00Z".to_string(),
            },
            Document {
                id: EntityIDFor::new("doc3").unwrap(),
                title: "MEMO-2024-001: Team Update".to_string(),
                content: "Important team announcements".to_string(),
                created_date: "2024-03-20T09:00:00Z".to_string(),
            },
        ];

        for doc in &docs {
            let args = DocumentInsertArgs::from(spec.clone());
            client.save_instance(doc, args).await?;
        }

        // Test: Find all documents starting with "DOC-"
        let query = select!([doc_id, title], and!(
            type_!(var!(doc), "@schema:Document"),
            triple!(var!(doc), "@schema:id", var!(doc_id)),
            triple!(var!(doc), "title", var!(title)),
            starts_with!(var!(title), "DOC-")
        ));

        let json_query = query.to_instance(None).to_json();
        let results: WOQLResult<HashMap<String, serde_json::Value>> =
            client.query_raw(Some(spec.clone()), json_query, None).await?;

        println!("Documents starting with 'DOC-':");
        for binding in &results.bindings {
            println!("  - {} ({})",
                binding.get("title").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?"),
                binding.get("doc_id").and_then(|v| v.as_str()).unwrap_or("?")
            );
        }

        assert_eq!(results.bindings.len(), 2, "Should find 2 documents starting with 'DOC-'");

        println!("\nstarts_with! macro test passed!");
        Ok(())
    }).await
}

#[tokio::test]
async fn test_ends_with_macro() -> anyhow::Result<()> {
    println!("\n=== Testing ends_with! macro ===\n");

    let server = TerminusDBServer::test_instance().await?;

    server.with_tmp_db("test_ends_with", |client, spec| async move {
        setup_test_schemas(&client, &spec).await?;

        // Insert test users
        let users = vec![
            User {
                id: EntityIDFor::new("user1").unwrap(),
                name: "Alice Smith".to_string(),
                email: "alice@company.com".to_string(),
                registration_date: "2023-01-15T10:00:00Z".to_string(),
            },
            User {
                id: EntityIDFor::new("user2").unwrap(),
                name: "Bob Johnson".to_string(),
                email: "bob@company.com".to_string(),
                registration_date: "2023-02-20T14:30:00Z".to_string(),
            },
            User {
                id: EntityIDFor::new("user3").unwrap(),
                name: "Charlie Brown".to_string(),
                email: "charlie@example.org".to_string(),
                registration_date: "2023-03-10T09:00:00Z".to_string(),
            },
        ];

        for user in &users {
            let args = DocumentInsertArgs::from(spec.clone());
            client.save_instance(user, args).await?;
        }

        // Test: Find all users with company.com email
        let query = select!([user_id, name, email], and!(
            type_!(var!(user), "@schema:User"),
            triple!(var!(user), "@schema:id", var!(user_id)),
            triple!(var!(user), "@schema:name", var!(name)),
            triple!(var!(user), "@schema:email", var!(email)),
            ends_with!(var!(email), "@company.com")
        ));

        let json_query = query.to_instance(None).to_json();
        let results: WOQLResult<HashMap<String, serde_json::Value>> =
            client.query_raw(Some(spec.clone()), json_query, None).await?;

        println!("Users with @company.com email:");
        for binding in &results.bindings {
            println!("  - {} ({})",
                binding.get("name").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?"),
                binding.get("email").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?")
            );
        }

        assert_eq!(results.bindings.len(), 2, "Should find 2 users with @company.com email");

        println!("\nends_with! macro test passed!");
        Ok(())
    }).await
}

#[tokio::test]
async fn test_contains_macro() -> anyhow::Result<()> {
    println!("\n=== Testing contains! macro ===\n");

    let server = TerminusDBServer::test_instance().await?;

    server.with_tmp_db("test_contains", |client, spec| async move {
        setup_test_schemas(&client, &spec).await?;

        // Insert test documents
        let docs = vec![
            Document {
                id: EntityIDFor::new("doc1").unwrap(),
                title: "Security Audit Report".to_string(),
                content: "This document contains important security findings.".to_string(),
                created_date: "2024-01-15T10:00:00Z".to_string(),
            },
            Document {
                id: EntityIDFor::new("doc2").unwrap(),
                title: "Monthly Newsletter".to_string(),
                content: "Company updates and announcements for this month.".to_string(),
                created_date: "2024-02-01T09:00:00Z".to_string(),
            },
            Document {
                id: EntityIDFor::new("doc3").unwrap(),
                title: "Technical Documentation".to_string(),
                content: "This contains important technical specifications and guidelines.".to_string(),
                created_date: "2024-01-20T14:30:00Z".to_string(),
            },
        ];

        for doc in &docs {
            let args = DocumentInsertArgs::from(spec.clone());
            client.save_instance(doc, args).await?;
        }

        // Test: Find all documents containing "important"
        let query = select!([doc_id, title, content], and!(
            type_!(var!(doc), "@schema:Document"),
            triple!(var!(doc), "@schema:id", var!(doc_id)),
            triple!(var!(doc), "title", var!(title)),
            triple!(var!(doc), "@schema:content", var!(content)),
            contains!(var!(content), "important")
        ));

        let json_query = query.to_instance(None).to_json();
        let results: WOQLResult<HashMap<String, serde_json::Value>> =
            client.query_raw(Some(spec.clone()), json_query, None).await?;

        println!("Documents containing 'important':");
        for binding in &results.bindings {
            println!("  - {}",
                binding.get("title").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?")
            );
        }

        assert_eq!(results.bindings.len(), 2, "Should find 2 documents containing 'important'");

        println!("\ncontains! macro test passed!");
        Ok(())
    }).await
}

// Date/time macro tests

#[tokio::test]
async fn test_today_macro() -> anyhow::Result<()> {
    println!("\n=== Testing today! macro ===\n");

    let server = TerminusDBServer::test_instance().await?;

    server.with_tmp_db("test_today", |client, spec| async move {
        setup_test_schemas(&client, &spec).await?;

        // Get today's date from the macro
        let today_value = today!();

        // Verify it's a valid ISO 8601 date string
        match today_value {
            Value::Data(terminusdb_schema::XSDAnySimpleType::String(date_str)) => {
                println!("today! generated: {}", date_str);

                // Verify format
                assert!(date_str.contains("T"), "Should contain time separator");
                assert!(date_str.ends_with("Z"), "Should end with UTC timezone");

                // Verify it can be parsed
                let parsed = chrono::DateTime::parse_from_rfc3339(&date_str);
                assert!(parsed.is_ok(), "Should be valid RFC3339/ISO8601 date");
            }
            _ => panic!("today! should return a Data string value"),
        }

        // Test in a query: Find documents created today
        // First insert a document with today's date
        let today_str = match today!() {
            Value::Data(terminusdb_schema::XSDAnySimpleType::String(s)) => s,
            _ => panic!("Expected string from today!"),
        };

        let doc = Document {
            id: EntityIDFor::new("today_doc").unwrap(),
            title: "Today's Document".to_string(),
            content: "Created today".to_string(),
            created_date: today_str.clone(),
        };

        let args = DocumentInsertArgs::from(spec.clone());
        client.save_instance(&doc, args).await?;

        // First, let's debug - query ALL documents to see what dates we have
        let debug_query = select!([doc_id, title, created_date], and!(
            type_!(var!(doc), "@schema:Document"),
            triple!(var!(doc), "@schema:id", var!(doc_id)),
            triple!(var!(doc), "title", var!(title)),
            triple!(var!(doc), "created_date", var!(created_date))
        ));

        let debug_json = debug_query.to_instance(None).to_json();
        let debug_results: WOQLResult<HashMap<String, serde_json::Value>> =
            client.query_raw(Some(spec.clone()), debug_json, None).await?;

        println!("\nDEBUG: All documents in database:");
        for binding in &debug_results.bindings {
            println!("  - {} (created: {})",
                binding.get("title").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?"),
                binding.get("created_date").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?")
            );
        }

        // Query for documents created today using date range comparison
        let today = chrono::Utc::now().date_naive();
        let today_start = today.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let today_end = today.and_hms_opt(23, 59, 59).unwrap().and_utc();
        let today_start_str = today_start.format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let today_end_str = today_end.format("%Y-%m-%dT%H:%M:%SZ").to_string();

        let query = select!([doc_id, title], and!(
            type_!(var!(doc), "@schema:Document"),
            triple!(var!(doc), "@schema:id", var!(doc_id)),
            triple!(var!(doc), "title", var!(title)),
            triple!(var!(doc), "created_date", var!(created)),
            in_between!(var!(created), data!(today_start_str), data!(today_end_str))
        ));

        let json_query = query.to_instance(None).to_json();
        let results: WOQLResult<HashMap<String, serde_json::Value>> =
            client.query_raw(Some(spec.clone()), json_query, None).await?;

        println!("Documents created today:");
        for binding in &results.bindings {
            println!("  - {}",
                binding.get("title").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?")
            );
        }

        // Should find at least the document we just created
        assert!(results.bindings.len() >= 1, "Should find at least one document created today");

        println!("\ntoday! macro test passed!");
        Ok(())
    }).await
}

#[tokio::test]
async fn test_after_before_macros() -> anyhow::Result<()> {
    println!("\n=== Testing after! and before! macros ===\n");

    let server = TerminusDBServer::test_instance().await?;

    server.with_tmp_db("test_after_before", |client, spec| async move {
        setup_test_schemas(&client, &spec).await?;

        // Insert test events
        let events = vec![
            Event {
                id: EntityIDFor::new("event1").unwrap(),
                title: "Past Event".to_string(),
                event_date: "2020-01-01T10:00:00Z".to_string(),
                status: "completed".to_string(),
                attendees: 50,
            },
            Event {
                id: EntityIDFor::new("event2").unwrap(),
                title: "Recent Event".to_string(),
                event_date: "2024-06-15T14:30:00Z".to_string(),
                status: "completed".to_string(),
                attendees: 75,
            },
            Event {
                id: EntityIDFor::new("event3").unwrap(),
                title: "Future Event".to_string(),
                event_date: "2030-12-31T18:00:00Z".to_string(),
                status: "scheduled".to_string(),
                attendees: 100,
            },
        ];

        for event in &events {
            let args = DocumentInsertArgs::from(spec.clone());
            client.save_instance(event, args).await?;
        }

        // Test after!: Find events after 2024-01-01
        let query_after = select!([event_id, title, event_date], and!(
            type_!(var!(event), "@schema:Event"),
            triple!(var!(event), "@schema:id", var!(event_id)),
            triple!(var!(event), "title", var!(title)),
            triple!(var!(event), "event_date", var!(event_date)),
            after!(var!(event_date), data!("2024-01-01T00:00:00Z"))
        ));

        let json_query = query_after.to_instance(None).to_json();
        let results_after: WOQLResult<HashMap<String, serde_json::Value>> =
            client.query_raw(Some(spec.clone()), json_query, None).await?;

        println!("Events after 2024-01-01:");
        for binding in &results_after.bindings {
            println!("  - {} ({})",
                binding.get("title").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?"),
                binding.get("event_date").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?")
            );
        }

        assert_eq!(results_after.bindings.len(), 2, "Should find 2 events after 2024-01-01");

        // Test before!: Find events before 2025-01-01
        let query_before = select!([event_id, title, event_date], and!(
            type_!(var!(event), "@schema:Event"),
            triple!(var!(event), "@schema:id", var!(event_id)),
            triple!(var!(event), "title", var!(title)),
            triple!(var!(event), "event_date", var!(event_date)),
            before!(var!(event_date), data!("2025-01-01T00:00:00Z"))
        ));

        let json_query = query_before.to_instance(None).to_json();
        let results_before: WOQLResult<HashMap<String, serde_json::Value>> =
            client.query_raw(Some(spec.clone()), json_query, None).await?;

        println!("\nEvents before 2025-01-01:");
        for binding in &results_before.bindings {
            println!("  - {} ({})",
                binding.get("title").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?"),
                binding.get("event_date").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?")
            );
        }

        assert_eq!(results_before.bindings.len(), 2, "Should find 2 events before 2025-01-01");

        println!("\nafter! and before! macros test passed!");
        Ok(())
    }).await
}

#[tokio::test]
async fn test_in_between_macro() -> anyhow::Result<()> {
    println!("\n=== Testing in_between! macro ===\n");

    let server = TerminusDBServer::test_instance().await?;

    server.with_tmp_db("test_in_between", |client, spec| async move {
        setup_test_schemas(&client, &spec).await?;

        // Insert test events
        let events = vec![
            Event {
                id: EntityIDFor::new("event1").unwrap(),
                title: "Q1 Event".to_string(),
                event_date: "2024-02-15T10:00:00Z".to_string(),
                status: "completed".to_string(),
                attendees: 50,
            },
            Event {
                id: EntityIDFor::new("event2").unwrap(),
                title: "Q2 Event".to_string(),
                event_date: "2024-05-20T14:30:00Z".to_string(),
                status: "completed".to_string(),
                attendees: 75,
            },
            Event {
                id: EntityIDFor::new("event3").unwrap(),
                title: "Q3 Event".to_string(),
                event_date: "2024-08-10T09:00:00Z".to_string(),
                status: "scheduled".to_string(),
                attendees: 60,
            },
            Event {
                id: EntityIDFor::new("event4").unwrap(),
                title: "Q4 Event".to_string(),
                event_date: "2024-11-25T16:00:00Z".to_string(),
                status: "scheduled".to_string(),
                attendees: 100,
            },
        ];

        for event in &events {
            let args = DocumentInsertArgs::from(spec.clone());
            client.save_instance(event, args).await?;
        }

        // Test: Find events in Q2 2024 (April-June)
        let query = select!([event_id, title, event_date], and!(
            type_!(var!(event), "@schema:Event"),
            triple!(var!(event), "@schema:id", var!(event_id)),
            triple!(var!(event), "title", var!(title)),
            triple!(var!(event), "event_date", var!(event_date)),
            in_between!(
                var!(event_date),
                data!("2024-04-01T00:00:00Z"),
                data!("2024-06-30T23:59:59Z")
            )
        ));

        let json_query = query.to_instance(None).to_json();
        let results: WOQLResult<HashMap<String, serde_json::Value>> =
            client.query_raw(Some(spec.clone()), json_query, None).await?;

        println!("Events in Q2 2024:");
        for binding in &results.bindings {
            println!("  - {} ({})",
                binding.get("title").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?"),
                binding.get("event_date").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?")
            );
        }

        assert_eq!(results.bindings.len(), 1, "Should find 1 event in Q2 2024");

        println!("\nin_between! macro test passed!");
        Ok(())
    }).await
}

#[tokio::test]
async fn test_today_in_between_macro() -> anyhow::Result<()> {
    println!("\n=== Testing today_in_between! macro ===\n");

    let server = TerminusDBServer::test_instance().await?;

    server.with_tmp_db("test_today_in_between", |client, spec| async move {
        setup_test_schemas(&client, &spec).await?;

        // Calculate dates relative to today
        let today = chrono::Utc::now();
        let past_date = (today - chrono::Duration::days(30)).format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        let future_date = (today + chrono::Duration::days(30)).format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        let long_past = (today - chrono::Duration::days(365)).format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        let long_future = (today + chrono::Duration::days(365)).format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        // Insert test subscriptions
        let subscriptions = vec![
            Subscription {
                id: EntityIDFor::new("sub1").unwrap(),
                name: "Active Subscription".to_string(),
                start_date: past_date.clone(),
                end_date: future_date.clone(),
                status: "active".to_string(),
            },
            Subscription {
                id: EntityIDFor::new("sub2").unwrap(),
                name: "Expired Subscription".to_string(),
                start_date: long_past.clone(),
                end_date: past_date.clone(),
                status: "expired".to_string(),
            },
            Subscription {
                id: EntityIDFor::new("sub3").unwrap(),
                name: "Future Subscription".to_string(),
                start_date: future_date.clone(),
                end_date: long_future.clone(),
                status: "pending".to_string(),
            },
        ];

        for sub in &subscriptions {
            let args = DocumentInsertArgs::from(spec.clone());
            client.save_instance(sub, args).await?;
        }

        // Test: Find active subscriptions (where today is between start and end dates)
        let query = select!([sub_id, name, start_date, end_date], and!(
            type_!(var!(sub), "@schema:Subscription"),
            triple!(var!(sub), "@schema:id", var!(sub_id)),
            triple!(var!(sub), "@schema:name", var!(name)),
            triple!(var!(sub), "@schema:start_date", var!(start_date)),
            triple!(var!(sub), "@schema:end_date", var!(end_date)),
            today_in_between!(var!(start_date), var!(end_date))
        ));

        let json_query = query.to_instance(None).to_json();
        let results: WOQLResult<HashMap<String, serde_json::Value>> =
            client.query_raw(Some(spec.clone()), json_query, None).await?;

        println!("Active subscriptions (today is between start and end):");
        for binding in &results.bindings {
            println!("  - {}",
                binding.get("name").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?")
            );
        }

        assert_eq!(results.bindings.len(), 1, "Should find 1 active subscription");

        // Verify it's the correct subscription
        let active_sub = &results.bindings[0];
        let sub_name = active_sub.get("name")
            .and_then(|v| v.get("@value"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert_eq!(sub_name, "Active Subscription", "Should find the Active Subscription");

        println!("\ntoday_in_between! macro test passed!");
        Ok(())
    }).await
}

// Compare macro tests

#[tokio::test]
async fn test_compare_macro_operators() -> anyhow::Result<()> {
    println!("\n=== Testing compare! macro with all operators ===\n");

    let server = TerminusDBServer::test_instance().await?;

    server.with_tmp_db("test_compare", |client, spec| async move {
        setup_test_schemas(&client, &spec).await?;

        // Insert test events with different attendee counts
        let events = vec![
            Event {
                id: EntityIDFor::new("event1").unwrap(),
                title: "Small Event".to_string(),
                event_date: "2024-01-01T10:00:00Z".to_string(),
                status: "completed".to_string(),
                attendees: 25,
            },
            Event {
                id: EntityIDFor::new("event2").unwrap(),
                title: "Medium Event".to_string(),
                event_date: "2024-02-01T10:00:00Z".to_string(),
                status: "completed".to_string(),
                attendees: 50,
            },
            Event {
                id: EntityIDFor::new("event3").unwrap(),
                title: "Large Event".to_string(),
                event_date: "2024-03-01T10:00:00Z".to_string(),
                status: "completed".to_string(),
                attendees: 100,
            },
            Event {
                id: EntityIDFor::new("event4").unwrap(),
                title: "Another Medium Event".to_string(),
                event_date: "2024-04-01T10:00:00Z".to_string(),
                status: "scheduled".to_string(),
                attendees: 50,
            },
        ];

        for event in &events {
            let args = DocumentInsertArgs::from(spec.clone());
            client.save_instance(event, args).await?;
        }

        // First, let's see all events
        println!("All events:");
        let all_events_query = select!([title, attendees], and!(
            type_!(var!(event), "@schema:Event"),
            triple!(var!(event), "title", var!(title)),
            triple!(var!(event), "attendees", var!(attendees))
        ));

        let all_results: WOQLResult<HashMap<String, serde_json::Value>> =
            client.query_raw(Some(spec.clone()), all_events_query.to_instance(None).to_json(), None).await?;

        for binding in &all_results.bindings {
            println!("  - {} ({} attendees)",
                binding.get("title").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?"),
                binding.get("attendees").and_then(|v| v.get("@value")).and_then(|v| v.as_i64()).unwrap_or(0)
            );
        }

        // Test 1: Greater than (>)
        println!("\nTest 1: Events with more than 50 attendees");
        let query_gt = select!([title, attendees], and!(
            type_!(var!(event), "@schema:Event"),
            triple!(var!(event), "title", var!(title)),
            triple!(var!(event), "attendees", var!(attendees)),
            compare!((var!(attendees)) > (50u32))
        ));

        let results_gt: WOQLResult<HashMap<String, serde_json::Value>> =
            client.query_raw(Some(spec.clone()), query_gt.to_instance(None).to_json(), None).await?;

        for binding in &results_gt.bindings {
            println!("  - {} ({} attendees)",
                binding.get("title").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?"),
                binding.get("attendees").and_then(|v| v.get("@value")).and_then(|v| v.as_i64()).unwrap_or(0)
            );
        }
        assert_eq!(results_gt.bindings.len(), 1, "Should find 1 event with >50 attendees");

        // Test 2: Less than (<)
        println!("\nTest 2: Events with less than 50 attendees");
        let query_lt = select!([title, attendees], and!(
            type_!(var!(event), "@schema:Event"),
            triple!(var!(event), "title", var!(title)),
            triple!(var!(event), "attendees", var!(attendees)),
            compare!((var!(attendees)) < (50))
        ));

        let results_lt: WOQLResult<HashMap<String, serde_json::Value>> =
            client.query_raw(Some(spec.clone()), query_lt.to_instance(None).to_json(), None).await?;

        for binding in &results_lt.bindings {
            println!("  - {} ({} attendees)",
                binding.get("title").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?"),
                binding.get("attendees").and_then(|v| v.get("@value")).and_then(|v| v.as_i64()).unwrap_or(0)
            );
        }
        assert_eq!(results_lt.bindings.len(), 1, "Should find 1 event with <50 attendees");

        // Test 3: Greater than or equal (>=)
        println!("\nTest 3: Events with 50 or more attendees");
        let query_gte = select!([title, attendees], and!(
            type_!(var!(event), "@schema:Event"),
            triple!(var!(event), "title", var!(title)),
            triple!(var!(event), "attendees", var!(attendees)),
            compare!((var!(attendees)) >= (50))
        ));

        let results_gte: WOQLResult<HashMap<String, serde_json::Value>> =
            client.query_raw(Some(spec.clone()), query_gte.to_instance(None).to_json(), None).await?;

        for binding in &results_gte.bindings {
            println!("  - {} ({} attendees)",
                binding.get("title").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?"),
                binding.get("attendees").and_then(|v| v.get("@value")).and_then(|v| v.as_i64()).unwrap_or(0)
            );
        }
        assert_eq!(results_gte.bindings.len(), 3, "Should find 3 events with >=50 attendees");

        // Test 4: Equals (==)
        println!("\nTest 4: Events with exactly 50 attendees");
        let query_eq = select!([title, attendees], and!(
            type_!(var!(event), "@schema:Event"),
            triple!(var!(event), "title", var!(title)),
            triple!(var!(event), "attendees", var!(attendees)),
            compare!((var!(attendees)) == (50))
        ));

        let results_eq: WOQLResult<HashMap<String, serde_json::Value>> =
            client.query_raw(Some(spec.clone()), query_eq.to_instance(None).to_json(), None).await?;

        for binding in &results_eq.bindings {
            println!("  - {} ({} attendees)",
                binding.get("title").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?"),
                binding.get("attendees").and_then(|v| v.get("@value")).and_then(|v| v.as_i64()).unwrap_or(0)
            );
        }
        assert_eq!(results_eq.bindings.len(), 2, "Should find 2 events with exactly 50 attendees");

        // Test 5: Not equals (!=)
        println!("\nTest 5: Events with status != 'scheduled'");
        let query_ne = select!([title, status], and!(
            type_!(var!(event), "@schema:Event"),
            triple!(var!(event), "title", var!(title)),
            triple!(var!(event), "status", var!(status)),
            compare!((var!(status)) != (data!("scheduled")))
        ));

        let results_ne: WOQLResult<HashMap<String, serde_json::Value>> =
            client.query_raw(Some(spec.clone()), query_ne.to_instance(None).to_json(), None).await?;

        for binding in &results_ne.bindings {
            println!("  - {} ({})",
                binding.get("title").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?"),
                binding.get("status").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?")
            );
        }
        assert_eq!(results_ne.bindings.len(), 3, "Should find 3 events with status != 'scheduled'");

        println!("\ncompare! macro with all operators test passed!");
        Ok(())
    }).await
}

// Complex query combining multiple macros

#[tokio::test]
async fn test_complex_query_with_multiple_macros() -> anyhow::Result<()> {
    println!("\n=== Testing complex query with multiple macros ===\n");

    let server = TerminusDBServer::test_instance().await?;

    server.with_tmp_db("test_complex", |client, spec| async move {
        setup_test_schemas(&client, &spec).await?;

        // Calculate dates
        let today = chrono::Utc::now();
        let last_month = (today - chrono::Duration::days(30)).format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        let last_week = (today - chrono::Duration::days(7)).format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        let yesterday = (today - chrono::Duration::days(1)).format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        // Insert test documents
        let docs = vec![
            Document {
                id: EntityIDFor::new("doc1").unwrap(),
                title: "DOC-2024-001: Security Report".to_string(),
                content: "This document contains important security findings from last month.".to_string(),
                created_date: last_month.clone(),
            },
            Document {
                id: EntityIDFor::new("doc2").unwrap(),
                title: "DOC-2024-002: Performance Analysis".to_string(),
                content: "Performance metrics and analysis for the system.".to_string(),
                created_date: last_week.clone(),
            },
            Document {
                id: EntityIDFor::new("doc3").unwrap(),
                title: "MEMO-2024-001: Team Update".to_string(),
                content: "Important updates for the development team.".to_string(),
                created_date: yesterday.clone(),
            },
            Document {
                id: EntityIDFor::new("doc4").unwrap(),
                title: "DOC-2024-003: Budget Report".to_string(),
                content: "Financial analysis and budget projections.".to_string(),
                created_date: yesterday.clone(),
            },
        ];

        for doc in &docs {
            let args = DocumentInsertArgs::from(spec.clone());
            client.save_instance(doc, args).await?;
        }

        // Complex query: Find DOC- prefixed documents created in the last 10 days containing "important"
        let ten_days_ago = (today - chrono::Duration::days(10)).format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        let query = select!([doc_id, title, content, created_date], and!(
            type_!(var!(doc), "@schema:Document"),
            triple!(var!(doc), "@schema:id", var!(doc_id)),
            triple!(var!(doc), "title", var!(title)),
            triple!(var!(doc), "@schema:content", var!(content)),
            triple!(var!(doc), "@schema:created_date", var!(created_date)),
            starts_with!(var!(title), "DOC-"),
            contains!(var!(content), "important"),
            after!(var!(created_date), data!(ten_days_ago))
        ));

        let json_query = query.to_instance(None).to_json();
        let results: WOQLResult<HashMap<String, serde_json::Value>> =
            client.query_raw(Some(spec.clone()), json_query, None).await?;

        println!("DOC- documents created in last 10 days containing 'important':");
        for binding in &results.bindings {
            println!("  - {} (created: {})",
                binding.get("title").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?"),
                binding.get("created_date").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?")
            );
        }

        // Should find DOC-2024-001 (has "important" and starts with DOC-, but created last month so outside 10 days)
        // Should NOT find DOC-2024-002 (starts with DOC- and within 10 days, but no "important")
        // Should NOT find MEMO-2024-001 (has "important" and within 10 days, but doesn't start with DOC-)
        // Should NOT find DOC-2024-003 (starts with DOC- and within 10 days, but no "important")
        assert_eq!(results.bindings.len(), 0, "Should find 0 documents matching all criteria");

        // Now test a query that should find results
        let query2 = select!([doc_id, title], and!(
            type_!(var!(doc), "@schema:Document"),
            triple!(var!(doc), "@schema:id", var!(doc_id)),
            triple!(var!(doc), "title", var!(title)),
            triple!(var!(doc), "@schema:content", var!(content)),
            triple!(var!(doc), "@schema:created_date", var!(created_date)),
            starts_with!(var!(title), "DOC-"),
            in_between!(var!(created_date), data!(last_month), today!())
        ));

        let json_query2 = query2.to_instance(None).to_json();
        let results2: WOQLResult<HashMap<String, serde_json::Value>> =
            client.query_raw(Some(spec.clone()), json_query2, None).await?;

        println!("\nAll DOC- documents created between last month and today:");
        for binding in &results2.bindings {
            println!("  - {}",
                binding.get("title").and_then(|v| v.get("@value")).and_then(|v| v.as_str()).unwrap_or("?")
            );
        }

        assert_eq!(results2.bindings.len(), 3, "Should find 3 DOC- documents");

        println!("\nComplex query with multiple macros test passed!");
        Ok(())
    }).await
}
