// Integration tests for TerminusDBHttpClient
// Note: These tests require a running TerminusDB instance (likely local).
// They are marked #[ignore] by default.

use terminusdb_client::deserialize::DefaultTDBDeserializer;
use terminusdb_client::*;
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema::ToTDBSchema;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use terminusdb_woql2::prelude::Query;
use terminusdb_woql_builder::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use uuid;
// --- Helper Structs/Enums for Tests ---

#[derive(
    Serialize, Deserialize, Debug, Clone, PartialEq, Default, TerminusDBModel, FromTDBInstance,
)]
// #[model(enum_uri = "terminusdb:///schema/TestStatus")] // Incorrect attribute
// No explicit tdb attribute needed here for now, defaults should work or be handled by derive.
enum TestStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(
    Serialize, Deserialize, Debug, Clone, PartialEq, Default, TerminusDBModel, FromTDBInstance,
)]
struct TestItem {
    name: String,
    status: TestStatus,
}

#[derive(
    Serialize, Deserialize, Debug, Clone, PartialEq, Default, TerminusDBModel, FromTDBInstance,
)]
struct TestDoc {
    // #[serde(rename = "@type")] // Derive likely handles this
    // schema_type: String,
    name: String,
    val: String,
}

impl TestDoc {
    fn new(name: &str, value: &str) -> Self {
        Self {
            // schema_type: "TestDoc".to_string(), // Derive handles type
            name: name.to_string(),
            val: value.to_string(),
        }
    }
}

// Helper function to get a test client and ensure test DB exists
async fn setup_test_client() -> anyhow::Result<TerminusDBHttpClient> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    // Use From trait for BranchSpec
    let spec = BranchSpec::from("test");
    // Use From trait for DocumentInsertArgs
    let args = DocumentInsertArgs::from(spec);
    client.insert_entity_schema::<TestDoc>(args).await.ok();
    Ok(client)
}

// --- Test Cases ---

#[tokio::test]
async fn test_info() -> anyhow::Result<()> {
    let client = setup_test_client().await?;
    let info_response = client.info().await?;
    println!("Server info: {:?}", info_response);
    assert!(!info_response.info.authority.is_empty());
    Ok(())
}

#[tokio::test]
async fn test_insert_and_get_document() -> anyhow::Result<()> {
    let client = setup_test_client().await?;
    let doc = TestDoc::new("doc1", "123");
    let spec = BranchSpec::from("test");
    let args = DocumentInsertArgs::from(spec.clone());

    // Insert
    let insert_result = client.insert_instance(&doc, args.clone()).await?;
    println!("Insert result: {:?}", insert_result);
    // Get the *actual* inserted ID (short form) from the result
    let inserted_id_full = match &insert_result.root_result {
        TDBInsertInstanceResult::Inserted(id) => id.clone(),
        TDBInsertInstanceResult::AlreadyExists(id) => id.clone(), // Use existing ID if it already existed
    };
    let id_parts: Vec<&str> = inserted_id_full.split("terminusdb:///data/").collect();
    let short_id = id_parts.get(1).expect("Could not parse ID").to_string();

    // Get using get_instance and the parsed short_id
    let mut deserializer = DefaultTDBDeserializer; // Use the client's deserializer
    let retrieved_doc = client
        .get_instance::<TestDoc>(&short_id, &spec, &mut deserializer)
        .await?;
    println!("Retrieved instance: {:?}", retrieved_doc);
    assert_eq!(doc, retrieved_doc);

    Ok(())
}

#[tokio::test]
async fn test_insert_and_get_instance() -> anyhow::Result<()> {
    let client = setup_test_client().await?;
    let doc = TestDoc::new("instance1", "456");
    let spec = BranchSpec::from("test");
    let args = DocumentInsertArgs::from(spec.clone());

    // Insert
    let insert_result_map = client.insert_instance(&doc, args).await?;

    // Get the *actual* inserted ID (short form) from the result
    let inserted_id_full = match insert_result_map
        .root_result
    {
        TDBInsertInstanceResult::Inserted(id) => id.clone(),
        TDBInsertInstanceResult::AlreadyExists(id) => id.clone(), // Use existing ID if it already existed
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
}

#[tokio::test]
async fn test_basic_woql_query_raw() -> anyhow::Result<()> {
    let client = setup_test_client().await?;
    let spec = BranchSpec::from("test");

    // Insert a doc to query
    let doc = TestDoc::new("query_target", "789");
    client
        .insert_instance(&doc, DocumentInsertArgs::from(spec.clone()))
        .await?;

    // Build WOQL query
    let query = json!({
        "@type": ("And"),
        "and":  [
             {
                "@type": ("Triple"),
                "graph": ("instance"),
                "object":  {
                    "@type": ("Value"),
                    "variable": ("name"),
                },
                "predicate":  {
                    "@type": ("NodeValue"),
                    "node": ("name"),
                },
                "subject":  {
                    "@type": ("NodeValue"),
                    "variable": ("id"),
                },
            },
             {
                "@type": ("Triple"),
                "graph": ("instance"),
                "object":  {
                    "@type": ("Value"),
                    "node": ("TestDoc"),
                },
                "predicate":  {
                    "@type": ("NodeValue"),
                    "node": ("@type"),
                },
                "subject":  {
                    "@type": ("NodeValue"),
                    "variable": ("id"),
                },
            },
        ],
    });

    // Execute builder query
    let response = client
        .query_raw::<HashMap<String, Value>>(Some(spec), query)
        .await?;
    println!("Query response: {:?}", response);
    assert!(!response.bindings.is_empty());
    // Add more specific assertions about bindings if needed
    let binding = response.bindings.first().unwrap();
    assert!(binding.get("id").is_some());
    assert!(binding.get("name").is_some());

    Ok(())
}

#[tokio::test]
async fn test_basic_woql_query_builder() -> anyhow::Result<()> {
    let client = setup_test_client().await?;
    let spec = BranchSpec::from("test");

    // Insert a doc to query
    let doc = TestDoc::new("query_target", "789");
    client
        .insert_instance(&doc, DocumentInsertArgs::from(spec.clone()))
        .await
        .unwrap();

    // Build WOQL query
    let v_id = vars!("id");
    let v_name = vars!("name");
    let query = WoqlBuilder::new()
        .triple(v_id.clone(), "name", v_name.clone())
        .isa(v_id.clone(), node("TestDoc"))
        .finalize();

    // let schema = Query::to_schema();
    // assert_eq!(schema.base(), Some(&"terminusdb://woql/data/".to_string()));

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
    struct TestDocRes {
        // #[serde(rename = "@type")] // Derive likely handles this
        // schema_type: String,
        name: String,
        value: StringValueObj,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
    struct StringValueObj {
        #[serde(rename = "@type")]
        ty: String,
        #[serde(rename = "@value")]
        value: String,
    }

    // Execute builder query
    let response = client
        .query::<HashMap<String, Value>>(Some(spec), query)
        .await?;
    println!("Query response: {:#?}", response);
    assert!(!response.bindings.is_empty());
    // Add more specific assertions about bindings if needed
    let binding = response.bindings.first().unwrap();
    assert!(binding.get("id").is_some());
    assert!(binding.get("name").is_some());

    Ok(())
}

#[tokio::test]
async fn test_woql_read_doc() -> anyhow::Result<()> {
    let client = setup_test_client().await?;
    let spec = BranchSpec::from("test");

    // Insert a doc to query
    let doc = TestDoc::new("query_target", "789");
    client
        .insert_instance(&doc, DocumentInsertArgs::from(spec.clone()))
        .await
        .unwrap();

    // Build WOQL query
    let v_id = vars!("id");
    let v_doc = vars!("doc");
    let query = WoqlBuilder::new()
        .read_document(v_id.clone(), v_doc.clone())
        .isa(v_id.clone(), node("TestDoc"))
        .select(vec![v_doc.clone()])
        .finalize();

    dbg!(&query);

    dbg!(query.to_json());

    // Execute builder query
    let response = client
        .query::<HashMap<String, TestDoc>>(Some(spec), query)
        .await?;
    println!("Query response: {:#?}", response);
    assert!(!response.bindings.is_empty());
    // Add more specific assertions about bindings if needed
    let binding = response.bindings.first().unwrap();
    // assert!(binding.get("id").is_some());

    Ok(())
}

#[tokio::test]
async fn test_commit_added_entities_query() -> anyhow::Result<()> {
    let client = setup_test_client().await?;
    let spec = BranchSpec::from("test");

    // --- Setup: Create a commit by inserting a document ---
    let doc = TestDoc::new("added_entity_test", "101");
    let insert_msg = "commit_for_added_entities_test";
    let mut args = DocumentInsertArgs::from(spec.clone());
    args.message = insert_msg.to_string();
    client.insert_instance(&doc, args).await?;

    // Find the commit log entry corresponding to the insert
    // This might be brittle; depends on log ordering and timing
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

    // Check if the ID of the inserted doc (short form) is present
    let doc_id_string_for_check = TestDoc::id().expect("TestDoc should have an ID"); // Store the string
    let id_parts: Vec<&str> = doc_id_string_for_check.split('/').collect(); // Split the stored string
    let short_id = id_parts.last().unwrap_or(&"").to_string();
    assert!(
        added_ids.contains(&short_id),
        "Expected ID {} not found in added IDs",
        short_id
    );

    Ok(())
}

#[tokio::test]
async fn test_enum_serialization() -> anyhow::Result<()> {
    let client = TerminusDBHttpClient::local_node().await; // Use local_node for unique DB
    let db_name = format!("test_enum_{}", uuid::Uuid::new_v4().simple());
    let spec = BranchSpec::from(db_name.as_str());
    let args = DocumentInsertArgs::from(spec.clone());

    // Ensure DB exists
    client.ensure_database(&db_name).await?;

    // Insert Schema for Enum and Item
    // Schema insertion needs both types to be registered
    client
        .insert_entity_schema::<TestItem>(args.clone())
        .await?; // TestItem schema implies TestStatus schema

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
    assert_eq!(
        status_field.as_str().unwrap(),
        "Completed",
        "Status field should be 'Completed'"
    );

    // Clean up: Delete the test database
    client.delete_database(&db_name).await?;

    Ok(())
}
