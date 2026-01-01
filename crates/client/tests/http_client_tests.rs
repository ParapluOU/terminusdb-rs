// Integration tests for TerminusDBHttpClient

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::deserialize::DefaultTDBDeserializer;
use terminusdb_client::*;
use terminusdb_schema::{ToTDBInstance, ToJson};
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use terminusdb_woql_builder::prelude::*;

// --- Helper Structs/Enums for Tests ---

#[derive(
    Debug, Clone, PartialEq, Default, TerminusDBModel, FromTDBInstance,
)]
enum TestStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(
    Debug, Clone, PartialEq, Default, TerminusDBModel, FromTDBInstance,
)]
struct TestItem {
    name: String,
    status: TestStatus,
}

#[derive(
    Debug, Clone, PartialEq, Default, TerminusDBModel, FromTDBInstance,
)]
struct TestDoc {
    name: String,
    val: String,
}

impl TestDoc {
    fn new(name: &str, value: &str) -> Self {
        Self {
            name: name.to_string(),
            val: value.to_string(),
        }
    }
}

// --- Test Cases ---

#[tokio::test]
async fn test_info() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    let client = server.client().await?;

    let info_response = client.info().await?;
    println!("Server info: {:?}", info_response);
    assert!(!info_response.info.authority.is_empty());
    Ok(())
}

#[tokio::test]
async fn test_insert_and_get_document() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server.with_tmp_db("test_get_doc", |client, spec| async move {
        let args = DocumentInsertArgs::from(spec.clone());
        client.insert_entity_schema::<TestDoc>(args.clone()).await?;

        let doc = TestDoc::new("doc1", "123");

        // Insert
        let insert_result = client.insert_instance(&doc, args.clone()).await?;
        println!("Insert result: {:?}", insert_result);
        // Get the *actual* inserted ID (short form) from the result
        let inserted_id_full = match &insert_result.root_result {
            TDBInsertInstanceResult::Inserted(id) => id.clone(),
            TDBInsertInstanceResult::AlreadyExists(id) => id.clone(),
        };
        let id_parts: Vec<&str> = inserted_id_full.split("terminusdb:///data/").collect();
        let short_id = id_parts.get(1).expect("Could not parse ID").to_string();

        // Get using get_instance and the parsed short_id
        let mut deserializer = DefaultTDBDeserializer;
        let retrieved_doc = client
            .get_instance::<TestDoc>(&short_id, &spec, &mut deserializer)
            .await?;
        println!("Retrieved instance: {:?}", retrieved_doc);
        assert_eq!(doc, retrieved_doc);

        Ok(())
    }).await
}

#[tokio::test]
async fn test_insert_and_get_instance() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server.with_tmp_db("test_get_inst", |client, spec| async move {
        let args = DocumentInsertArgs::from(spec.clone());
        client.insert_entity_schema::<TestDoc>(args.clone()).await?;

        let doc = TestDoc::new("instance1", "456");

        // Insert
        let insert_result_map = client.insert_instance(&doc, args).await?;

        // Get the *actual* inserted ID (short form) from the result
        let inserted_id_full = match insert_result_map.root_result {
            TDBInsertInstanceResult::Inserted(id) => id.clone(),
            TDBInsertInstanceResult::AlreadyExists(id) => id.clone(),
        };
        let id_parts: Vec<&str> = inserted_id_full.split("terminusdb:///data/").collect();
        let short_id = id_parts.get(1).expect("Could not parse ID").to_string();

        // Get - Use DefaultTDBDeserializer, passing the parsed short_id
        let mut deserializer = DefaultTDBDeserializer;
        let retrieved_doc = client
            .get_instance::<TestDoc>(&short_id, &spec, &mut deserializer)
            .await?;
        println!("Retrieved instance: {:?}\n", retrieved_doc);
        assert_eq!(doc, retrieved_doc);

        Ok(())
    }).await
}

#[tokio::test]
async fn test_basic_woql_query_raw() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server.with_tmp_db("test_woql_raw", |client, spec| async move {
        let args = DocumentInsertArgs::from(spec.clone());
        client.insert_entity_schema::<TestDoc>(args.clone()).await?;

        // Insert a doc to query
        let doc = TestDoc::new("query_target", "789");
        client.insert_instance(&doc, args).await?;

        // Build WOQL query using builder, then serialize to JSON for raw query test
        // This ensures the JSON format is correct
        let v_id = vars!("id");
        let v_name = vars!("name");
        let builder_query = WoqlBuilder::new()
            .triple(v_id.clone(), "name", v_name.clone())
            .isa(v_id.clone(), node("TestDoc"))
            .finalize();

        // Convert to JSON-LD and use query_raw to test raw query interface
        // Must use to_instance(None).to_json() to get proper JSON-LD format with @type tags
        let query_json = builder_query.to_instance(None).to_json();
        println!("Query JSON: {}", serde_json::to_string_pretty(&query_json)?);

        // Execute raw query
        let response = client
            .query_raw::<HashMap<String, Value>>(Some(spec.clone()), query_json, None)
            .await?;
        println!("Query response: {:?}", response);
        assert!(!response.bindings.is_empty());
        // Add more specific assertions about bindings if needed
        let binding = response.bindings.first().unwrap();
        assert!(binding.get("id").is_some());
        assert!(binding.get("name").is_some());

        Ok(())
    }).await
}

#[tokio::test]
async fn test_basic_woql_query_builder() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server.with_tmp_db("test_woql_builder", |client, spec| async move {
        let args = DocumentInsertArgs::from(spec.clone());
        client.insert_entity_schema::<TestDoc>(args.clone()).await?;

        // Insert a doc to query
        let doc = TestDoc::new("query_target", "789");
        client.insert_instance(&doc, args).await?;

        // Build WOQL query
        let v_id = vars!("id");
        let v_name = vars!("name");
        let query = WoqlBuilder::new()
            .triple(v_id.clone(), "name", v_name.clone())
            .isa(v_id.clone(), node("TestDoc"))
            .finalize();

        // Execute builder query
        let response = client
            .query::<HashMap<String, Value>>(Some(spec.clone()), query)
            .await?;
        println!("Query response: {:#?}", response);
        assert!(!response.bindings.is_empty());
        // Add more specific assertions about bindings if needed
        let binding = response.bindings.first().unwrap();
        assert!(binding.get("id").is_some());
        assert!(binding.get("name").is_some());

        Ok(())
    }).await
}

#[tokio::test]
async fn test_woql_read_doc() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server.with_tmp_db("test_woql_read", |client, spec| async move {
        let args = DocumentInsertArgs::from(spec.clone());
        client.insert_entity_schema::<TestDoc>(args.clone()).await?;

        // Insert a doc to query
        let doc = TestDoc::new("query_target", "789");
        client.insert_instance(&doc, args).await?;

        // Build WOQL query
        let v_id = vars!("id");
        let v_doc = vars!("doc");
        let query = WoqlBuilder::new()
            .read_document(v_id.clone(), v_doc.clone())
            .isa(v_id.clone(), node("TestDoc"))
            .select(vec![v_doc.clone()])
            .finalize();

        dbg!(&query);

        // Execute builder query
        let response = client
            .query::<HashMap<String, TestDoc>>(Some(spec.clone()), query)
            .await?;
        println!("Query response: {:#?}", response);
        assert!(!response.bindings.is_empty());

        Ok(())
    }).await
}

#[tokio::test]
async fn test_commit_added_entities_query() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server.with_tmp_db("test_commit_query", |client, spec| async move {
        let args = DocumentInsertArgs::from(spec.clone());
        client.insert_entity_schema::<TestDoc>(args.clone()).await?;

        // --- Setup: Create a commit by inserting a document ---
        let doc = TestDoc::new("added_entity_test", "101");
        let insert_msg = "commit_for_added_entities_test";
        let mut insert_args = args.clone();
        insert_args.message = insert_msg.to_string();
        client.insert_instance(&doc, insert_args).await?;

        // Find the commit log entry corresponding to the insert
        let log_entries = client
            .log(
                &spec,
                LogOpts {
                    count: Some(1),
                    ..Default::default()
                },
            )
            .await?;
        let commit = log_entries
            .first()
            .expect("No commit log entry found after insert");
        assert_eq!(commit.message, insert_msg);
        println!("Found commit: {:?}", commit);

        // --- Test: Query for added entities in that commit ---
        let added_ids = client
            .commit_added_entities_ids::<TestDoc>(&spec, commit, None)
            .await?;
        println!("Added IDs: {:?}", added_ids);

        // Verify that at least one entity was added
        assert!(
            !added_ids.is_empty(),
            "Should have at least one added entity ID"
        );

        Ok(())
    }).await
}

#[tokio::test]
async fn test_enum_serialization() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server.with_tmp_db("test_enum_ser", |client, spec| async move {
        let args = DocumentInsertArgs::from(spec.clone());

        // Insert Schema for Enum and Item
        client.insert_entity_schema::<TestItem>(args.clone()).await?;

        // Create instance
        let item = TestItem {
            name: "Task 1".to_string(),
            status: TestStatus::Completed,
        };

        // Insert instance
        let insert_result = client.insert_instance(&item, args.clone()).await?;
        let inserted_id_full = match insert_result.root_result {
            TDBInsertInstanceResult::Inserted(id) => id.clone(),
            TDBInsertInstanceResult::AlreadyExists(id) => id.clone(),
        };

        // Use the ID returned by the server, stripping the base URI prefix if present
        let id_to_get = inserted_id_full
            .strip_prefix("terminusdb:///data/")
            .unwrap_or(&inserted_id_full);

        // Retrieve document using the ID from the insert result
        let retrieved_json = client
            .get_document(&id_to_get, &spec, GetOpts::default())
            .await?;

        println!("Retrieved JSON: {:#?}", retrieved_json);

        // Assert: check if 'status' field is a JSON string with the correct value
        let status_field = retrieved_json
            .get("status")
            .expect("Status field not found in retrieved document");

        assert!(status_field.is_string(), "Status field should be a string");
        // Status is stored lowercase in TerminusDB
        assert_eq!(
            status_field.as_str().unwrap().to_lowercase(),
            "completed",
            "Status field should be 'completed'"
        );

        Ok(())
    }).await
}
