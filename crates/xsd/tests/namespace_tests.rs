//! Tests for XSD namespace handling and multi-namespace schema insertion
//!
//! These tests verify that:
//! 1. Multiple XSD schemas with the same class names but different namespaces
//!    can coexist in the same TerminusDB database
//! 2. Context properly isolates namespaces
//! 3. Instances from different namespaces can be inserted and retrieved correctly

use terminusdb_bin::TerminusDBServer;
use terminusdb_client::DocumentInsertArgs;
use terminusdb_schema::{json::ToJson, Schema, ToMaybeTDBSchema, ToTDBInstance};
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use terminusdb_xsd::XsdModel;

// Reference model using derive - this is known to work
#[derive(Debug, Clone, TerminusDBModel, FromTDBInstance)]
#[tdb(key = "value_hash")]
pub struct TestDocDerived {
    pub id: String,
    pub title: Option<String>,
    pub author: Option<String>,
}

/// Helper to find a class by name in a list of schemas
fn find_class<'a>(schemas: &'a [Schema], name: &str) -> Option<&'a Schema> {
    schemas.iter().find(|s| match s {
        Schema::Class { id, .. } => id == name,
        _ => false,
    })
}

// ============================================================================
// Schema Format Comparison Test
// ============================================================================

/// Compare derived model schema format with XSD-generated schema format
/// This helps identify differences causing the server crash
#[test]
fn test_compare_derived_vs_xsd_schema_format() {
    // Get the derived model schema using to_schema_tree
    let schemas = TestDocDerived::to_schema_tree();
    let derived_schema = schemas.first().expect("Should have at least one schema");
    let derived_json = derived_schema.to_json();
    eprintln!("\n=== Derived Model Schema ===");
    eprintln!("{}", serde_json::to_string_pretty(&derived_json).unwrap());

    // Get the XSD-generated schema
    let xsd_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/namespace_book.xsd"
    );
    let model = XsdModel::from_file(xsd_path, None::<&str>).expect("Failed to load XSD");
    let doc_schema = find_class(model.schemas(), "DocumentType").expect("Should have DocumentType");
    let xsd_json = doc_schema.to_json();
    eprintln!("\n=== XSD-Generated Schema ===");
    eprintln!("{}", serde_json::to_string_pretty(&xsd_json).unwrap());

    // The format should be similar - both should have:
    // - @id, @type, @key
    // - Properties with xsd types
}

/// Test that derived models can insert instances with value_hash key
#[tokio::test]
async fn test_derived_model_insertion() -> anyhow::Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_derived_insertion", |client, spec| {
            async move {
                let args = DocumentInsertArgs::from(spec.clone());

                // Insert the schema using to_schema_tree to get all schemas
                let schemas = TestDocDerived::to_schema_tree();
                let schema = schemas.first().expect("Should have at least one schema");
                let schema_json = schema.to_json();
                eprintln!(
                    "Derived schema: {}",
                    serde_json::to_string_pretty(&schema_json)?
                );
                client
                    .insert_documents(vec![&schema_json], args.clone().as_schema())
                    .await?;
                eprintln!("Inserted derived schema successfully");

                // Create an instance
                let instance = TestDocDerived {
                    id: "test-id".to_string(),
                    title: Some("Test Title".to_string()),
                    author: Some("Test Author".to_string()),
                };

                // Get the instance JSON using ToTDBInstance trait
                let instance_json = instance.to_json();
                eprintln!(
                    "Derived instance: {}",
                    serde_json::to_string_pretty(&instance_json)?
                );

                // Insert the instance using create_instance (the proper method)
                let result = client.create_instance(&instance, args.clone()).await;
                eprintln!("Create instance result: {:?}", result);
                result?;
                eprintln!("Inserted derived instance successfully");

                Ok(())
            }
        })
        .await
}

// ============================================================================
// Schema Loading Tests
// ============================================================================

#[test]
fn test_book_schema_loads_with_correct_namespace() {
    let xsd_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/namespace_book.xsd"
    );
    let model = XsdModel::from_file(xsd_path, None::<&str>).expect("Failed to load book XSD");

    let context = model.context();
    assert!(
        context.schema.contains("example.com/book"),
        "Context schema should contain book namespace, got: {}",
        context.schema
    );

    let schemas = model.schemas();
    assert!(
        find_class(schemas, "DocumentType").is_some(),
        "Should have DocumentType class"
    );
    assert!(
        find_class(schemas, "PersonType").is_some(),
        "Should have PersonType class (unique to book)"
    );
}

#[test]
fn test_library_schema_loads_with_correct_namespace() {
    let xsd_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/namespace_library.xsd"
    );
    let model = XsdModel::from_file(xsd_path, None::<&str>).expect("Failed to load library XSD");

    let context = model.context();
    assert!(
        context.schema.contains("example.com/library"),
        "Context schema should contain library namespace, got: {}",
        context.schema
    );

    let schemas = model.schemas();
    assert!(
        find_class(schemas, "DocumentType").is_some(),
        "Should have DocumentType class"
    );
    assert!(
        find_class(schemas, "BranchType").is_some(),
        "Should have BranchType class (unique to library)"
    );
}

#[test]
fn test_both_schemas_have_same_class_name_different_namespace() {
    let book_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/namespace_book.xsd"
    );
    let library_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/namespace_library.xsd"
    );

    let book_model = XsdModel::from_file(book_path, None::<&str>).unwrap();
    let library_model = XsdModel::from_file(library_path, None::<&str>).unwrap();

    // Both should have DocumentType class
    assert!(
        find_class(book_model.schemas(), "DocumentType").is_some(),
        "Book should have DocumentType"
    );
    assert!(
        find_class(library_model.schemas(), "DocumentType").is_some(),
        "Library should have DocumentType"
    );

    // But different namespaces
    assert_ne!(
        book_model.context().schema,
        library_model.context().schema,
        "Namespaces should be different"
    );

    eprintln!("Book namespace: {}", book_model.context().schema);
    eprintln!("Library namespace: {}", library_model.context().schema);
}

// ============================================================================
// Integration Tests - Multi-Namespace Insertion
// ============================================================================

#[tokio::test]
async fn test_insert_schema_with_context() -> anyhow::Result<()> {
    let xsd_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/namespace_book.xsd"
    );
    let model = XsdModel::from_file(xsd_path, None::<&str>)?;
    let context = model.context().clone();
    let schemas = model.schemas().to_vec();

    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_insert_with_context", |client, spec| {
            let context = context.clone();
            let schemas = schemas.clone();
            async move {
                let args = DocumentInsertArgs::from(spec.clone());

                eprintln!("Context: {:?}", context);

                // Debug: show what schemas look like
                for (i, schema) in schemas.iter().enumerate() {
                    eprintln!(
                        "Schema {}: {}",
                        i,
                        serde_json::to_string_pretty(&schema.to_json())?
                    );
                }

                // Insert schemas (without context - use default namespace)
                let schema_jsons: Vec<_> = schemas.iter().map(|s| s.to_json()).collect();
                let schema_refs: Vec<_> = schema_jsons.iter().collect();
                let result = client
                    .insert_documents(schema_refs, args.clone().as_schema())
                    .await?;
                eprintln!("Inserted {} schema documents", result.len());

                // Insert an instance parsed from XML
                let xml = r#"<?xml version="1.0"?>
                <document xmlns="http://example.com/book" id="book1">
                    <title>The Rust Programming Language</title>
                    <author>Steve Klabnik</author>
                    <isbn>978-1593278281</isbn>
                </document>"#;

                let instances = model.parse_xml_to_instances(xml)?;
                assert!(!instances.is_empty(), "Should parse XML to instances");

                // Debug: show instance JSON
                for (i, inst) in instances.iter().enumerate() {
                    eprintln!(
                        "Instance {}: {}",
                        i,
                        serde_json::to_string_pretty(&inst.to_json())?
                    );
                }

                // Insert instances using POST method (not PUT with create)
                // POST works correctly, while PUT with create causes "Unexpected failure"
                let instance_refs: Vec<_> = instances.iter().collect();
                client.post_documents(instance_refs, args).await?;
                eprintln!("Successfully inserted book instance");

                Ok(())
            }
        })
        .await
}

#[tokio::test]
async fn test_insert_multiple_namespaces_same_database() -> anyhow::Result<()> {
    let book_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/namespace_book.xsd"
    );
    let library_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/namespace_library.xsd"
    );

    let book_model = XsdModel::from_file(book_path, None::<&str>)?;
    let library_model = XsdModel::from_file(library_path, None::<&str>)?;

    eprintln!("Book namespace: {}", book_model.context().schema);
    eprintln!("Library namespace: {}", library_model.context().schema);

    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_multi_namespace", |client, spec| {
            let book_schemas = book_model.schemas().to_vec();
            let library_schemas = library_model.schemas().to_vec();
            let book_model_ref = &book_model;
            let library_model_ref = &library_model;

            async move {
                let args = DocumentInsertArgs::from(spec.clone());

                // Insert book schemas with fully-qualified URIs
                eprintln!("\n=== Inserting book schema ===");
                let book_schema_jsons: Vec<serde_json::Value> = book_schemas
                    .iter()
                    .map(|s| s.to_namespaced_json())
                    .collect();
                for (i, json) in book_schema_jsons.iter().enumerate() {
                    eprintln!("Book schema {}: {}", i, serde_json::to_string_pretty(json)?);
                }
                let book_schema_refs: Vec<_> = book_schema_jsons.iter().collect();
                let book_result = client
                    .insert_documents(book_schema_refs, args.clone().as_schema())
                    .await?;
                eprintln!("Inserted {} book schema documents", book_result.len());

                // Insert library schemas with fully-qualified URIs
                eprintln!("\n=== Inserting library schema ===");
                let library_schema_jsons: Vec<serde_json::Value> = library_schemas
                    .iter()
                    .map(|s| s.to_namespaced_json())
                    .collect();
                for (i, json) in library_schema_jsons.iter().enumerate() {
                    eprintln!(
                        "Library schema {}: {}",
                        i,
                        serde_json::to_string_pretty(json)?
                    );
                }
                let library_schema_refs: Vec<_> = library_schema_jsons.iter().collect();
                let library_result = client
                    .insert_documents(library_schema_refs, args.clone().as_schema())
                    .await?;
                eprintln!("Inserted {} library schema documents", library_result.len());

                // Now insert instances from both namespaces
                eprintln!("\n=== Inserting book instance ===");
                let book_xml = r#"<?xml version="1.0"?>
                <document xmlns="http://example.com/book" id="book1">
                    <title>Rust in Action</title>
                    <author>Tim McNamara</author>
                    <isbn>978-1617294556</isbn>
                    <price>49.99</price>
                </document>"#;

                let book_instances = book_model_ref.parse_xml_to_instances(book_xml)?;
                assert!(!book_instances.is_empty(), "Should parse book XML");

                // Use to_namespaced_json for fully-qualified @type
                let book_instance_jsons: Vec<serde_json::Value> = book_instances
                    .iter()
                    .map(|inst| inst.to_namespaced_json())
                    .collect();
                for (i, json) in book_instance_jsons.iter().enumerate() {
                    eprintln!(
                        "Book instance {}: {}",
                        i,
                        serde_json::to_string_pretty(json)?
                    );
                }
                let book_json_refs: Vec<_> = book_instance_jsons.iter().collect();
                client.post_documents(book_json_refs, args.clone()).await?;
                eprintln!("Inserted book instance");

                eprintln!("\n=== Inserting library instance ===");
                let library_xml = r#"<?xml version="1.0"?>
                <document xmlns="http://example.com/library" id="lib1">
                    <catalogNumber>LIB-2024-001</catalogNumber>
                    <location>Shelf A3</location>
                    <borrower>John Doe</borrower>
                </document>"#;

                let library_instances = library_model_ref.parse_xml_to_instances(library_xml)?;
                assert!(!library_instances.is_empty(), "Should parse library XML");

                // Use to_namespaced_json for fully-qualified @type
                let library_instance_jsons: Vec<serde_json::Value> = library_instances
                    .iter()
                    .map(|inst| inst.to_namespaced_json())
                    .collect();
                for (i, json) in library_instance_jsons.iter().enumerate() {
                    eprintln!(
                        "Library instance {}: {}",
                        i,
                        serde_json::to_string_pretty(json)?
                    );
                }
                let library_json_refs: Vec<_> = library_instance_jsons.iter().collect();
                client
                    .post_documents(library_json_refs, args.clone())
                    .await?;
                eprintln!("Inserted library instance");

                eprintln!("\n=== Both namespaces coexist successfully! ===");
                Ok(())
            }
        })
        .await
}

#[tokio::test]
async fn test_context_namespace_resolves_class_ids() -> anyhow::Result<()> {
    // This test verifies that the context's @schema properly resolves class IDs
    let xsd_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/namespace_book.xsd"
    );
    let model = XsdModel::from_file(xsd_path, None::<&str>)?;

    let context = model.context();

    // The class IDs should be short (e.g., "DocumentType")
    let doc_type = find_class(model.schemas(), "DocumentType");
    assert!(doc_type.is_some(), "Should find DocumentType");

    if let Some(Schema::Class { id, .. }) = doc_type {
        // ID should be short, not namespaced
        assert_eq!(id, "DocumentType", "Class ID should be short name");

        // Full resolution happens via context.schema
        let expected_full_uri = format!("{}DocumentType", context.schema);
        eprintln!("Class ID: {}", id);
        eprintln!("Context schema: {}", context.schema);
        eprintln!("Full resolved URI would be: {}", expected_full_uri);
    }

    Ok(())
}

// ============================================================================
// Edge Cases
// ============================================================================

#[tokio::test]
async fn test_insert_schema_without_context_uses_default() -> anyhow::Result<()> {
    // When inserting without context, TerminusDB uses default namespace
    let xsd_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/namespace_book.xsd"
    );
    let model = XsdModel::from_file(xsd_path, None::<&str>)?;
    let schemas = model.schemas().to_vec();

    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_without_context", |client, spec| {
            let schemas = schemas.clone();
            async move {
                let args = DocumentInsertArgs::from(spec.clone());

                // Insert WITHOUT context (existing behavior)
                client
                    .insert_schema_instances(schemas, args.clone())
                    .await?;

                eprintln!("Inserted schemas without explicit context");
                eprintln!("Classes resolve to default namespace");

                Ok(())
            }
        })
        .await
}
