//! Integration tests for real-world XSD schemas: DITA and NISO-STS
//!
//! These tests verify that the XSD to TerminusDB schema converter can handle
//! complex, production-quality XSD schemas used in document publishing.
//!
//! Schemas are provided by the `schemas-dita` and `schemas-niso-sts` crates
//! from the parapluou/schemas-rs repository.

use schemas_dita::{Dita12, SchemaBundle};
use schemas_niso_sts::NisoSts;
use std::sync::LazyLock;
use tempfile::TempDir;
use terminusdb_schema::Schema;
use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
use terminusdb_xsd::schema_model::XsdSchema;
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

/// Lazily extracted NISO-STS schemas (shared across tests)
static NISO_DIR: LazyLock<TempDir> = LazyLock::new(|| {
    let dir = TempDir::new().expect("Failed to create temp dir for NISO schemas");
    NisoSts::write_to_directory(dir.path()).expect("Failed to extract NISO schemas");
    dir
});

// ============================================================================
// DITA Schema Tests
// ============================================================================

#[test]
fn test_dita_basetopic_schema_loads() {
    let basetopic_path = DITA_DIR.path().join("xsd1.2-url/base/xsd/basetopic.xsd");

    let xsd_schema = XsdSchema::from_xsd_file(&basetopic_path, None::<&str>)
        .expect("Failed to parse DITA basetopic.xsd");

    // DITA basetopic should have many complex types
    assert!(
        xsd_schema.complex_types.len() > 10,
        "DITA basetopic should have many complex types, found {}",
        xsd_schema.complex_types.len()
    );

    println!("DITA basetopic.xsd loaded successfully:");
    println!("  Complex types: {}", xsd_schema.complex_types.len());
    println!("  Simple types: {}", xsd_schema.simple_types.len());
    println!("  Root elements: {}", xsd_schema.root_elements.len());
}

#[test]
fn test_dita_basetopic_generates_schemas() {
    let basetopic_path = DITA_DIR.path().join("xsd1.2-url/base/xsd/basetopic.xsd");

    let xsd_schema = XsdSchema::from_xsd_file(&basetopic_path, None::<&str>)
        .expect("Failed to parse DITA basetopic.xsd");

    let generator = XsdToSchemaGenerator::new();
    let schemas = generator
        .generate(&xsd_schema)
        .expect("Failed to generate schemas");

    // Should generate many schemas from DITA
    assert!(
        schemas.len() > 10,
        "DITA should generate many schemas, got {}",
        schemas.len()
    );

    // Collect class names
    let class_names: Vec<&str> = schemas
        .iter()
        .filter_map(|s| match s {
            Schema::Class { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();

    println!("DITA basetopic generated {} schemas", schemas.len());
    println!(
        "Sample classes: {:?}",
        &class_names[..class_names.len().min(10)]
    );

    // DITA should have common element types
    // Note: Actual names depend on xmlschema-rs parsing
    assert!(
        class_names
            .iter()
            .any(|n| n.contains("Topic") || n.contains("topic")),
        "DITA should have topic-related classes"
    );
}

#[test]
fn test_dita_basemap_schema_loads() {
    let basemap_path = DITA_DIR.path().join("xsd1.2-url/base/xsd/basemap.xsd");

    let xsd_schema = XsdSchema::from_xsd_file(&basemap_path, None::<&str>)
        .expect("Failed to parse DITA basemap.xsd");

    let generator = XsdToSchemaGenerator::new();
    let schemas = generator
        .generate(&xsd_schema)
        .expect("Failed to generate schemas");

    println!("DITA basemap.xsd generated {} schemas", schemas.len());

    // basemap should have map-related types
    let class_names: Vec<&str> = schemas
        .iter()
        .filter_map(|s| match s {
            Schema::Class { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();

    assert!(
        class_names
            .iter()
            .any(|n| n.contains("Map") || n.contains("map")),
        "DITA basemap should have map-related classes"
    );
}

// ============================================================================
// NISO-STS Schema Tests
// ============================================================================

#[test]
fn test_niso_sts_schema_loads() {
    let niso_path = NISO_DIR
        .path()
        .join("NISO-STS-extended-1-MathML3-XSD/NISO-STS-extended-1-mathml3.xsd");

    let xsd_schema = XsdSchema::from_xsd_file(&niso_path, None::<&str>)
        .expect("Failed to parse NISO-STS schema");

    // NISO-STS is a large schema with many types
    println!("NISO-STS loaded successfully:");
    println!("  Complex types: {}", xsd_schema.complex_types.len());
    println!("  Simple types: {}", xsd_schema.simple_types.len());
    println!("  Root elements: {}", xsd_schema.root_elements.len());

    assert!(
        xsd_schema.complex_types.len() > 50,
        "NISO-STS should have many complex types, found {}",
        xsd_schema.complex_types.len()
    );
}

#[test]
fn test_niso_sts_generates_schemas() {
    let niso_path = NISO_DIR
        .path()
        .join("NISO-STS-extended-1-MathML3-XSD/NISO-STS-extended-1-mathml3.xsd");

    let xsd_schema = XsdSchema::from_xsd_file(&niso_path, None::<&str>)
        .expect("Failed to parse NISO-STS schema");

    let generator = XsdToSchemaGenerator::new();
    let schemas = generator
        .generate(&xsd_schema)
        .expect("Failed to generate schemas");

    println!("NISO-STS generated {} schemas", schemas.len());

    // Collect class names
    let class_names: Vec<&str> = schemas
        .iter()
        .filter_map(|s| match s {
            Schema::Class { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();

    // NISO-STS should have standard-related types
    println!(
        "Sample NISO-STS classes: {:?}",
        &class_names[..class_names.len().min(20)]
    );

    assert!(
        schemas.len() > 50,
        "NISO-STS should generate many schemas, got {}",
        schemas.len()
    );
}

#[test]
fn test_niso_sts_has_expected_elements() {
    let niso_path = NISO_DIR
        .path()
        .join("NISO-STS-extended-1-MathML3-XSD/NISO-STS-extended-1-mathml3.xsd");

    let model =
        XsdModel::from_file(&niso_path, None::<&str>).expect("Failed to load NISO-STS model");

    let stats = model.stats();
    println!("NISO-STS XsdModel stats:");
    println!("  XSD schemas: {}", stats.xsd_schema_count);
    println!("  TDB schemas: {}", stats.tdb_schema_count);
    println!("  Complex types: {}", stats.total_complex_types);
    println!("  Simple types: {}", stats.total_simple_types);
    println!("  Root elements: {}", stats.total_root_elements);

    // NISO-STS has specific document elements
    // These assertions may need adjustment based on actual schema content
    assert!(stats.total_complex_types > 0, "Should have complex types");
    assert!(stats.tdb_schema_count > 0, "Should generate TDB schemas");
}

#[test]
fn test_niso_sts_schema_properties() {
    let niso_path = NISO_DIR
        .path()
        .join("NISO-STS-extended-1-MathML3-XSD/NISO-STS-extended-1-mathml3.xsd");

    let model =
        XsdModel::from_file(&niso_path, None::<&str>).expect("Failed to load NISO-STS model");

    let schemas = model.schemas();

    // Verify all schemas have valid structure
    for schema in schemas {
        if let Schema::Class { id, properties, .. } = schema {
            // All properties should have valid names and classes
            for prop in properties {
                assert!(
                    !prop.name.is_empty(),
                    "Property name should not be empty in {}",
                    id
                );
                assert!(
                    !prop.class.is_empty(),
                    "Property class should not be empty for {}.{}",
                    id,
                    prop.name
                );

                // Class should either be an XSD type, sys: type, or PascalCase
                if !prop.class.starts_with("xsd:") && !prop.class.starts_with("sys:") {
                    // Custom type should be PascalCase (first char uppercase)
                    let first_char = prop.class.chars().next().unwrap();
                    assert!(
                        first_char.is_uppercase(),
                        "Custom type {} should be PascalCase in {}.{}",
                        prop.class,
                        id,
                        prop.name
                    );
                }
            }
        }
    }

    println!("All NISO-STS schemas have valid property structure");
}

// ============================================================================
// Directory-based Generation Tests
// ============================================================================

#[test]
fn test_dita_directory_generation() {
    let dita_dir = DITA_DIR.path().join("xsd1.2-url/base/xsd");

    let generator = XsdToSchemaGenerator::new();

    // This tests the entry point detection and directory parsing
    let schemas = generator
        .generate_from_directory(&dita_dir, None::<&str>)
        .expect("Failed to generate from DITA directory");

    println!(
        "Generated {} unique schemas from DITA directory",
        schemas.len()
    );

    // Should deduplicate common types
    assert!(
        schemas.len() > 10,
        "Should generate schemas from DITA directory"
    );
}

#[test]
fn test_niso_directory_generation() {
    let niso_dir = NISO_DIR.path().join("NISO-STS-extended-1-MathML3-XSD");

    let generator = XsdToSchemaGenerator::new();

    let schemas = generator
        .generate_from_directory(&niso_dir, None::<&str>)
        .expect("Failed to generate from NISO-STS directory");

    println!(
        "Generated {} unique schemas from NISO-STS directory",
        schemas.len()
    );

    assert!(
        schemas.len() > 50,
        "Should generate many schemas from NISO-STS directory"
    );
}

// ============================================================================
// Schema Deduplication Tests
// ============================================================================

#[test]
fn test_schema_deduplication() {
    let niso_path = NISO_DIR
        .path()
        .join("NISO-STS-extended-1-MathML3-XSD/NISO-STS-extended-1-mathml3.xsd");

    let xsd_schema = XsdSchema::from_xsd_file(&niso_path, None::<&str>)
        .expect("Failed to parse NISO-STS schema");

    let generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).unwrap();

    // Check for duplicates by class ID
    let mut seen_ids = std::collections::HashSet::new();

    for schema in &schemas {
        if let Schema::Class { id, .. } = schema {
            seen_ids.insert(id.clone());
        }
    }

    // After deduplication, there should be no duplicates
    let deduplicated = generator.deduplicate_schemas(schemas.clone());
    let mut dedup_ids = std::collections::HashSet::new();
    for schema in &deduplicated {
        if let Schema::Class { id, .. } = schema {
            assert!(
                dedup_ids.insert(id.clone()),
                "Duplicate found after deduplication: {}",
                id
            );
        }
    }

    println!(
        "Deduplication: {} -> {} schemas",
        schemas.len(),
        deduplicated.len()
    );
}
