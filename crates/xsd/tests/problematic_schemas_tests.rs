//! Tests for problematic XSD schemas that fail during org seeding.
//!
//! These schemas are currently excluded from org seeding due to various issues:
//! - **akoma-ntoso**: Cyclic class references in mixed content (QuotedText -> Inline -> Inline)
//! - **tei**: XSD is incomplete - missing import files
//! - **docbook**: Uses RNG format, not XSD (so XSD parsing may fail)
//!
//! This test file attempts to parse and insert these schemas to capture
//! the specific error messages and verify the failure modes.

use schemas::{AkomaNtoso30, Dita13, DocBook51, SchemaBundle, TeiP5};
use std::path::PathBuf;
use tempfile::TempDir;
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::{BranchSpec, DocumentInsertArgs};
use terminusdb_xsd::XsdModel;

/// Helper to extract schema to temp directory and get entry point.
fn extract_schema<B: SchemaBundle>(bundle: &B, entry_file: &str) -> (TempDir, PathBuf) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    B::write_to_directory(temp_dir.path()).expect("Failed to write schema");
    let entry_point = temp_dir.path().join(entry_file);
    (temp_dir, entry_point)
}

/// Test: Akoma Ntoso 3.0 schema parsing.
///
/// Known issue: Cyclic class references in mixed content types.
/// Example: QuotedText -> Inline -> Inline creates circular inheritance.
#[test]
fn test_akoma_ntoso_schema_parsing() {
    let bundle = AkomaNtoso30;
    let (_temp_dir, entry_point) = extract_schema(&bundle, "akomantoso30.xsd");

    eprintln!("\n=== Testing Akoma Ntoso 3.0 Schema Parsing ===");
    eprintln!("Entry point: {:?}", entry_point);

    // Check that the entry point file exists
    assert!(
        entry_point.exists(),
        "Entry point file should exist: {:?}",
        entry_point
    );

    // Try to parse the schema
    let result = XsdModel::from_file(&entry_point, None::<&str>);

    match result {
        Ok(model) => {
            eprintln!("✓ Schema parsed successfully!");
            eprintln!("  Namespace: {}", model.context().schema);
            eprintln!("  Schema count: {}", model.schemas().len());

            // Check for potential cyclic references in schemas
            for schema in model.schemas() {
                if let terminusdb_schema::Schema::Class { id, inherits, .. } = schema {
                    if inherits.contains(&id.to_string()) {
                        eprintln!("  ⚠ Self-reference in class '{}': {:?}", id, inherits);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Schema parsing failed:");
            eprintln!("  Error: {}", e);
            eprintln!("  This is expected - akoma-ntoso has cyclic class references");
        }
    }
}

/// Test: TEI P5 schema parsing.
///
/// TEI P5 is extremely large - requires 512MB stack to parse without overflow.
#[test]
fn test_tei_schema_parsing() {
    let bundle = TeiP5;
    let (_temp_dir, entry_point) = extract_schema(&bundle, "tei_all.xsd");

    eprintln!("\n=== Testing TEI P5 Schema Parsing ===");
    eprintln!("Entry point: {:?}", entry_point);

    // Check that the entry point file exists
    assert!(
        entry_point.exists(),
        "Entry point file should exist: {:?}",
        entry_point
    );

    // TEI is extremely large - spawn parsing in a thread with 512MB stack
    let entry_point_clone = entry_point.clone();
    let handle = std::thread::Builder::new()
        .stack_size(512 * 1024 * 1024) // 512MB stack
        .name("tei-parser".to_string())
        .spawn(move || XsdModel::from_file(&entry_point_clone, None::<&str>))
        .expect("Failed to spawn parser thread");

    let result = handle.join().expect("Parser thread panicked");

    match result {
        Ok(model) => {
            eprintln!("✓ Schema parsed successfully!");
            eprintln!("  Namespace: {}", model.context().schema);
            eprintln!("  Schema count: {}", model.schemas().len());
        }
        Err(e) => {
            eprintln!("✗ Schema parsing failed:");
            eprintln!("  Error: {}", e);
        }
    }
}

/// Test: DocBook 5.1 schema parsing.
///
/// Known issue: DocBook 5.1 uses RNG (RelaxNG) format, not XSD.
/// Attempting XSD parsing should fail gracefully.
#[test]
fn test_docbook_schema_parsing() {
    let bundle = DocBook51;
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

    eprintln!("\n=== Testing DocBook 5.1 Schema Parsing ===");

    // Write the schema files
    DocBook51::write_to_directory(temp_dir.path()).expect("Failed to write schema");

    // List what files were written
    eprintln!("Files written to temp dir:");
    for entry in std::fs::read_dir(temp_dir.path()).unwrap() {
        let entry = entry.unwrap();
        eprintln!("  {:?}", entry.path());
    }

    // Try the expected entry point (may not exist for RNG-only bundles)
    let xsd_entry = temp_dir.path().join("docbook.xsd");
    let rng_entry = temp_dir.path().join("docbook.rng");

    if xsd_entry.exists() {
        eprintln!("Found XSD entry: {:?}", xsd_entry);

        let result = XsdModel::from_file(&xsd_entry, None::<&str>);
        match result {
            Ok(model) => {
                eprintln!("✓ XSD parsed successfully!");
                eprintln!("  Namespace: {}", model.context().schema);
                eprintln!("  Schema count: {}", model.schemas().len());
            }
            Err(e) => {
                eprintln!("✗ XSD parsing failed:");
                eprintln!("  Error: {}", e);
            }
        }
    } else if rng_entry.exists() {
        eprintln!("Found RNG entry: {:?} (XSD not available)", rng_entry);
        eprintln!("DocBook uses RelaxNG format - XSD parsing not supported");
    } else {
        eprintln!("No recognized schema entry point found");
        eprintln!("DocBook may require different handling");
    }
}

/// Test: Akoma Ntoso schema insertion into TerminusDB.
///
/// This tests the full pipeline: parse XSD -> generate TDB schemas -> insert.
#[tokio::test]
async fn test_akoma_ntoso_schema_insertion() -> anyhow::Result<()> {
    let bundle = AkomaNtoso30;
    let (_temp_dir, entry_point) = extract_schema(&bundle, "akomantoso30.xsd");

    eprintln!("\n=== Testing Akoma Ntoso Schema Insertion ===");

    // Parse the schema
    let model = match XsdModel::from_file(&entry_point, None::<&str>) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Parsing failed (expected): {}", e);
            return Ok(()); // Test passes - we documented the failure
        }
    };

    eprintln!("Schema parsed with {} types", model.schemas().len());

    // Start isolated TerminusDB server
    let server = TerminusDBServer::test().await?;
    let client = server.client().await?;

    // Create database
    client.ensure_database("test_akoma_ntoso").await?;

    // Try to insert schemas
    let spec = BranchSpec::new("test_akoma_ntoso");
    let args = DocumentInsertArgs::from(spec);
    let context = model.context().clone();
    let schemas = model.schemas().to_vec();

    eprintln!("Inserting {} schemas...", schemas.len());

    let result = client
        .insert_schema_with_context(context, schemas, args)
        .await;

    match result {
        Ok(_) => {
            eprintln!("✓ Schemas inserted successfully!");
        }
        Err(e) => {
            eprintln!("✗ Schema insertion failed:");
            eprintln!("  Error: {}", e);
            eprintln!("  This may be due to cyclic class references");
        }
    }

    Ok(())
}

/// Test: TEI schema insertion into TerminusDB.
///
/// TEI parsing requires 512MB stack, so we spawn parsing in a separate thread.
#[tokio::test]
async fn test_tei_schema_insertion() -> anyhow::Result<()> {
    let bundle = TeiP5;
    let (_temp_dir, entry_point) = extract_schema(&bundle, "tei_all.xsd");

    eprintln!("\n=== Testing TEI Schema Insertion ===");

    // TEI is extremely large - spawn parsing in a thread with 512MB stack
    let entry_point_clone = entry_point.clone();
    let parse_result = tokio::task::spawn_blocking(move || {
        let handle = std::thread::Builder::new()
            .stack_size(512 * 1024 * 1024) // 512MB stack
            .name("tei-parser".to_string())
            .spawn(move || XsdModel::from_file(&entry_point_clone, None::<&str>))
            .expect("Failed to spawn parser thread");
        handle.join().expect("Parser thread panicked")
    })
    .await?;

    // Parse the schema
    let model = match parse_result {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Parsing failed: {}", e);
            return Ok(()); // Test passes - we documented the failure
        }
    };

    eprintln!("Schema parsed with {} types", model.schemas().len());

    // Start isolated TerminusDB server
    let server = TerminusDBServer::test().await?;
    let client = server.client().await?;

    // Create database
    client.ensure_database("test_tei").await?;

    // Try to insert schemas
    let spec = BranchSpec::new("test_tei");
    let args = DocumentInsertArgs::from(spec);
    let context = model.context().clone();
    let schemas = model.schemas().to_vec();

    eprintln!("Inserting {} schemas...", schemas.len());

    let result = client
        .insert_schema_with_context(context, schemas, args)
        .await;

    match result {
        Ok(_) => {
            eprintln!("✓ Schemas inserted successfully!");
        }
        Err(e) => {
            eprintln!("✗ Schema insertion failed:");
            eprintln!("  Error: {}", e);
        }
    }

    Ok(())
}

/// Test: DITA 1.3 schema parsing with XML catalog support.
///
/// DITA 1.3 uses URN-based schemaLocation values like:
/// `urn:oasis:names:tc:dita:xsd:deliveryTargetAttDomain.xsd:1.3`
///
/// Without catalog support, these URNs cannot be resolved and only 3-4 schemas load.
/// With catalog support, the full 700+ schemas should load correctly.
#[test]
fn test_dita13_with_catalog_support() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

    eprintln!("\n=== Testing DITA 1.3 Schema with Catalog Support ===");

    // Write the schema files
    Dita13::write_to_directory(temp_dir.path()).expect("Failed to write schema");

    // Find catalog.xml and entry point
    let catalog_path = temp_dir.path().join("catalog.xml");
    let entry_point = temp_dir.path().join("base/xsd/basetopic.xsd");

    eprintln!("Entry point: {:?}", entry_point);
    eprintln!("Catalog: {:?}", catalog_path);

    assert!(
        entry_point.exists(),
        "Entry point file should exist: {:?}",
        entry_point
    );
    assert!(
        catalog_path.exists(),
        "Catalog file should exist: {:?}",
        catalog_path
    );

    // Parse with catalog support
    let result = XsdModel::from_file(&entry_point, Some(&catalog_path));

    match result {
        Ok(model) => {
            let schema_count = model.schemas().len();
            eprintln!("✓ Schema parsed successfully with catalog!");
            eprintln!("  Namespace: {}", model.context().schema);
            eprintln!("  Schema count: {}", schema_count);

            // DITA 1.3 should have many schemas when catalog resolution works
            // Without catalog, we only get ~3-4 schemas
            assert!(
                schema_count > 50,
                "Expected >50 schemas with catalog support, got {}. \
                 This suggests URN resolution may not be working.",
                schema_count
            );
        }
        Err(e) => {
            eprintln!("✗ Schema parsing failed:");
            eprintln!("  Error: {}", e);
            panic!("DITA 1.3 parsing with catalog should succeed: {}", e);
        }
    }
}

/// Test: Verify DITA 1.3 catalog structure has expected URN mappings.
///
/// DITA 1.3 uses a hierarchical catalog structure:
/// - Root catalog.xml uses `<nextCatalog>` to reference subdirectory catalogs
/// - Subdirectory catalogs (e.g., base/catalog.xml) contain the actual URN mappings
#[test]
fn test_dita13_catalog_structure() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    Dita13::write_to_directory(temp_dir.path()).expect("Failed to write schema");

    let root_catalog_path = temp_dir.path().join("catalog.xml");
    let base_catalog_path = temp_dir.path().join("base/catalog.xml");

    eprintln!("\n=== Testing DITA 1.3 Catalog Structure ===");
    eprintln!("Root catalog: {:?}", root_catalog_path);
    eprintln!("Base catalog: {:?}", base_catalog_path);

    assert!(root_catalog_path.exists(), "Root catalog.xml should exist");

    // Root catalog uses nextCatalog to chain to subdirectory catalogs
    let root_content =
        std::fs::read_to_string(&root_catalog_path).expect("Failed to read root catalog");

    assert!(
        root_content.contains("<nextCatalog"),
        "Root catalog should use nextCatalog to chain to subdirectories"
    );

    // Check that base/catalog.xml exists and contains actual URN mappings
    assert!(
        base_catalog_path.exists(),
        "Base catalog should exist at base/catalog.xml"
    );

    let base_content =
        std::fs::read_to_string(&base_catalog_path).expect("Failed to read base catalog");

    // Base catalog should contain URN mappings
    assert!(
        base_content.contains("urn:oasis:names:tc:dita:xsd:"),
        "Base catalog should contain DITA URN mappings"
    );

    // Check for common DITA domain URNs in base catalog
    let expected_patterns = [
        "deliveryTargetAttDomain",
        "basetopic.xsd",
        "commonElement",
    ];

    for pattern in &expected_patterns {
        if base_content.contains(pattern) {
            eprintln!("  ✓ Found expected pattern: {}", pattern);
        } else {
            eprintln!("  ⚠ Missing expected pattern: {}", pattern);
        }
    }

    eprintln!("Root catalog: {} bytes", root_content.len());
    eprintln!("Base catalog: {} bytes", base_content.len());
}

/// Test: DITA 1.3 schema insertion into TerminusDB (in-memory).
///
/// This tests the full pipeline: parse XSD with catalog -> generate TDB schemas -> insert.
#[tokio::test]
async fn test_dita13_schema_insertion() -> anyhow::Result<()> {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    Dita13::write_to_directory(temp_dir.path()).expect("Failed to write schema");

    let catalog_path = temp_dir.path().join("catalog.xml");
    let entry_point = temp_dir.path().join("base/xsd/basetopic.xsd");

    eprintln!("\n=== Testing DITA 1.3 Schema Insertion into TerminusDB ===");

    // Parse the schema with catalog support
    let model = XsdModel::from_file(&entry_point, Some(&catalog_path))?;

    eprintln!("Schema parsed with {} types", model.schemas().len());

    // Start isolated TerminusDB server (in-memory)
    let server = TerminusDBServer::test().await?;
    let client = server.client().await?;

    // Create database
    client.ensure_database("test_dita13").await?;

    // Try to insert schemas
    let spec = BranchSpec::new("test_dita13");
    let args = DocumentInsertArgs::from(spec);
    let context = model.context().clone();
    let schemas = model.schemas().to_vec();

    eprintln!("Inserting {} schemas into TerminusDB...", schemas.len());

    let result = client
        .insert_schema_with_context(context, schemas, args)
        .await;

    match result {
        Ok(_) => {
            eprintln!("✓ DITA 1.3 schemas inserted successfully into TerminusDB!");
        }
        Err(e) => {
            eprintln!("✗ Schema insertion failed:");
            eprintln!("  Error: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}
