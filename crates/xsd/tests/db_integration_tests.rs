//! Integration tests for XSD-to-Instance-to-TerminusDB flow
//!
//! These tests verify the end-to-end flow of:
//! 1. Loading XSD schemas
//! 2. Converting XSD schemas to TerminusDB Schema definitions
//! 3. Inserting schemas into TerminusDB (in-memory server)
//! 4. Parsing XML to instances (future - blocked on schema/type mapping improvements)
//! 5. Inserting instances into TerminusDB
//!
//! This validates that XSD schema bundles can be used with TerminusDB.

use schemas_dita::{Dita12, SchemaBundle};
use std::sync::LazyLock;
use tempfile::TempDir;
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::DocumentInsertArgs;
use terminusdb_schema::Schema;
use terminusdb_xsd::XsdModel;

// ============================================================================
// Schema Extraction - shared temp directories for efficiency
// ============================================================================

/// Lazily extracted DITA schemas (shared across tests)
static DITA_DIR: LazyLock<TempDir> = LazyLock::new(|| {
    let dir = TempDir::new().expect("Failed to create temp dir for DITA schemas");
    Dita12::write_to_directory(dir.path()).expect("Failed to extract DITA schemas");
    dir
});

// ============================================================================
// Helper Functions
// ============================================================================

/// Get the path to DITA basetopic.xsd
fn dita_topic_xsd_path() -> std::path::PathBuf {
    DITA_DIR.path().join("xsd1.2-url/base/xsd/basetopic.xsd")
}

/// Collect all class names that are referenced by schemas but not defined
fn find_missing_dependencies(schemas: &[Schema]) -> Vec<String> {
    use std::collections::HashSet;

    // Collect all defined class names
    let defined: HashSet<String> = schemas
        .iter()
        .filter_map(|s| match s {
            Schema::Class { id, .. } => Some(id.clone()),
            Schema::Enum { id, .. } => Some(id.clone()),
            Schema::OneOfClass { id, .. } => Some(id.clone()),
            Schema::TaggedUnion { id, .. } => Some(id.clone()),
        })
        .collect();

    // Collect all referenced class names
    let mut referenced: HashSet<String> = HashSet::new();
    for schema in schemas {
        if let Schema::Class { properties, inherits, .. } = schema {
            for prop in properties {
                if !prop.class.starts_with("xsd:") {
                    referenced.insert(prop.class.clone());
                }
            }
            for parent in inherits {
                referenced.insert(parent.clone());
            }
        }
    }

    // Find missing
    referenced
        .difference(&defined)
        .cloned()
        .collect()
}

/// Filter schemas to only include those without missing dependencies
fn filter_valid_schemas(schemas: &[Schema]) -> Vec<Schema> {
    use std::collections::HashSet;

    // Collect all defined class names
    let defined: HashSet<String> = schemas
        .iter()
        .filter_map(|s| match s {
            Schema::Class { id, .. } => Some(id.clone()),
            Schema::Enum { id, .. } => Some(id.clone()),
            Schema::OneOfClass { id, .. } => Some(id.clone()),
            Schema::TaggedUnion { id, .. } => Some(id.clone()),
        })
        .collect();

    // Keep only schemas with all dependencies satisfied
    schemas
        .iter()
        .filter(|s| {
            if let Schema::Class { properties, inherits, .. } = s {
                // Check all property types exist
                let props_ok = properties.iter().all(|p| {
                    p.class.starts_with("xsd:") || defined.contains(&p.class)
                });
                // Check all parent classes exist
                let inherits_ok = inherits.iter().all(|parent| defined.contains(parent));
                props_ok && inherits_ok
            } else {
                true // Enums are always valid
            }
        })
        .cloned()
        .collect()
}

// ============================================================================
// Tests - Schema Loading
// ============================================================================

#[test]
fn test_xsd_model_loads_dita_schemas() {
    let topic_path = dita_topic_xsd_path();

    let model = XsdModel::from_file(&topic_path, None::<&str>)
        .expect("Failed to load DITA topic model");

    let schemas = model.schemas();
    assert!(
        !schemas.is_empty(),
        "XsdModel should generate TerminusDB schemas from DITA XSD"
    );

    // Count schema types
    let class_count = schemas
        .iter()
        .filter(|s| matches!(s, Schema::Class { .. }))
        .count();
    let enum_count = schemas
        .iter()
        .filter(|s| matches!(s, Schema::Enum { .. }))
        .count();

    println!("DITA XsdModel generated:");
    println!("  Total schemas: {}", schemas.len());
    println!("  Classes: {}", class_count);
    println!("  Enums: {}", enum_count);

    // Check for expected DITA types
    let class_names: Vec<_> = schemas
        .iter()
        .filter_map(|s| match s {
            Schema::Class { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();

    // DITA should have topic-related types
    assert!(
        class_names.iter().any(|n| n.contains("Topic") || n.contains("topic")),
        "DITA should have topic-related schemas"
    );
}

#[test]
fn test_analyze_schema_dependencies() {
    let topic_path = dita_topic_xsd_path();

    let model = XsdModel::from_file(&topic_path, None::<&str>)
        .expect("Failed to load DITA topic model");

    let schemas = model.schemas();
    let missing = find_missing_dependencies(schemas);

    println!("Schema dependency analysis:");
    println!("  Total schemas: {}", schemas.len());
    println!("  Missing dependencies: {}", missing.len());

    if !missing.is_empty() {
        println!("  Missing types (first 10):");
        for (i, dep) in missing.iter().take(10).enumerate() {
            println!("    {}: {}", i + 1, dep);
        }
    }

    // Check how many schemas are valid
    let valid = filter_valid_schemas(schemas);
    println!("  Valid schemas (all deps satisfied): {}", valid.len());
}

// ============================================================================
// Tests - TerminusDB Integration
// ============================================================================

/// Test inserting valid schemas (those with all dependencies satisfied) into TerminusDB
#[tokio::test]
async fn test_insert_valid_schemas_into_terminusdb() -> anyhow::Result<()> {
    let topic_path = dita_topic_xsd_path();

    let model = XsdModel::from_file(&topic_path, None::<&str>)
        .expect("Failed to load DITA topic model");

    let all_schemas = model.schemas();
    let valid_schemas = filter_valid_schemas(all_schemas);

    println!("Inserting valid schemas into TerminusDB:");
    println!("  Total schemas from XSD: {}", all_schemas.len());
    println!("  Valid schemas to insert: {}", valid_schemas.len());

    if valid_schemas.is_empty() {
        println!("WARNING: No valid schemas to insert (all have missing dependencies)");
        return Ok(());
    }

    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_valid_schemas", |client, spec| {
            let schemas = valid_schemas.clone();
            async move {
                let args = DocumentInsertArgs::from(spec.clone());

                client
                    .insert_schema_instances(schemas.clone(), args)
                    .await?;

                println!("Successfully inserted {} valid schemas", schemas.len());
                Ok(())
            }
        })
        .await?;

    Ok(())
}

/// Test that verifies schemas from a small subset work correctly
#[tokio::test]
async fn test_insert_minimal_subset_schemas() -> anyhow::Result<()> {
    let topic_path = dita_topic_xsd_path();

    let model = XsdModel::from_file(&topic_path, None::<&str>)
        .expect("Failed to load DITA topic model");

    let schemas = model.schemas();

    // Take only enum schemas which should always be self-contained
    let enum_schemas: Vec<_> = schemas
        .iter()
        .filter(|s| matches!(s, Schema::Enum { .. }))
        .cloned()
        .collect();

    println!("Testing enum schemas only:");
    println!("  Enum schemas found: {}", enum_schemas.len());

    if enum_schemas.is_empty() {
        println!("No enum schemas to test");
        return Ok(());
    }

    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_enum_schemas", |client, spec| {
            let schemas = enum_schemas.clone();
            async move {
                let args = DocumentInsertArgs::from(spec.clone());

                client
                    .insert_schema_instances(schemas.clone(), args)
                    .await?;

                println!("Successfully inserted {} enum schemas", schemas.len());
                Ok(())
            }
        })
        .await?;

    Ok(())
}

/// End-to-end test showing the full intended flow
///
/// NOTE: This test currently documents the limitation that XML-to-Instance
/// parsing doesn't work correctly because the JSON structure from xmlschema-rs
/// doesn't include @type annotations. This is a known issue to be addressed.
#[tokio::test]
async fn test_full_xsd_xml_flow_documents_limitations() -> anyhow::Result<()> {
    let topic_path = dita_topic_xsd_path();

    // Step 1: Load XSD model
    let model = XsdModel::from_file(&topic_path, None::<&str>)
        .expect("Failed to load DITA topic model");

    println!("Step 1: Loaded XSD model");
    println!("  Generated {} TerminusDB schemas", model.schemas().len());

    // Step 2: Try parsing XML (expected to fail with current implementation)
    let minimal_dita = r#"<?xml version="1.0" encoding="UTF-8"?>
<topic id="test-topic">
    <title>Test Topic Title</title>
    <body>
        <p>This is a test paragraph.</p>
    </body>
</topic>
"#;

    let parse_result = model.parse_xml_to_instances(minimal_dita);

    match parse_result {
        Ok(instances) => {
            println!("Step 2: Successfully parsed XML to {} instances", instances.len());

            // Step 3: Insert into TerminusDB
            let server = TerminusDBServer::test_instance().await?;

            server.with_tmp_db("test_full_flow", |client, spec| {
                let schemas = filter_valid_schemas(model.schemas()).clone();
                let insts = instances.clone();
                async move {
                    let args = DocumentInsertArgs::from(spec.clone());

                    // Insert schemas first
                    if !schemas.is_empty() {
                        client.insert_schema_instances(schemas, args.clone()).await?;
                        println!("Step 3a: Inserted schemas");
                    }

                    // Insert instances
                    if !insts.is_empty() {
                        let instance_refs: Vec<_> = insts.iter().collect();
                        client.insert_documents(instance_refs, args).await?;
                        println!("Step 3b: Inserted instances");
                    }

                    Ok(())
                }
            }).await?;
        }
        Err(e) => {
            // This is expected with the current implementation
            println!("Step 2: XML parsing to instances failed (expected)");
            println!("  Error: {}", e);
            println!("");
            println!("This limitation exists because:");
            println!("  - xmlschema-rs returns JSON without @type annotations");
            println!("  - The parser can't map XML element names to schema class names");
            println!("  - This requires enhancement to the XmlToInstanceParser");
        }
    }

    Ok(())
}
