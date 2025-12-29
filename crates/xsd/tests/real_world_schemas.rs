//! Integration tests for real-world XSD schemas: DITA and NISO-STS
//!
//! These tests verify that the XSD to TerminusDB schema converter can handle
//! complex, production-quality XSD schemas used in document publishing.
//!
//! Note: These tests are marked #[ignore] by default as they require the
//! pubbin repository's schema files to be present. Run with:
//!   cargo test -p terminusdb-xsd --test real_world_schemas -- --ignored

use std::path::Path;
use terminusdb_schema::{Schema, Property, TypeFamily};
use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
use terminusdb_xsd::schema_model::XsdSchema;
use terminusdb_xsd::XsdModel;

// ============================================================================
// DITA Schema Tests
// ============================================================================

/// Path to DITA schemas relative to the pubbin repository
const DITA_BASE_PATH: &str = "../../../schemas/dita/xsd/xsd1.2-url/base/xsd";

/// Check if DITA schemas are available
fn dita_schemas_available() -> bool {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let dita_path = Path::new(manifest_dir).join(DITA_BASE_PATH);
    dita_path.exists()
}

#[test]
#[ignore = "Requires DITA schemas from pubbin repository"]
fn test_dita_basetopic_schema_loads() {
    if !dita_schemas_available() {
        println!("DITA schemas not available, skipping test");
        return;
    }

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let basetopic_path = Path::new(manifest_dir)
        .join(DITA_BASE_PATH)
        .join("basetopic.xsd");

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
#[ignore = "Requires DITA schemas from pubbin repository"]
fn test_dita_basetopic_generates_schemas() {
    if !dita_schemas_available() {
        return;
    }

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let basetopic_path = Path::new(manifest_dir)
        .join(DITA_BASE_PATH)
        .join("basetopic.xsd");

    let xsd_schema = XsdSchema::from_xsd_file(&basetopic_path, None::<&str>)
        .expect("Failed to parse DITA basetopic.xsd");

    let generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).expect("Failed to generate schemas");

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
    println!("Sample classes: {:?}", &class_names[..class_names.len().min(10)]);

    // DITA should have common element types
    // Note: Actual names depend on xmlschema-rs parsing
    assert!(
        class_names.iter().any(|n| n.contains("Topic") || n.contains("topic")),
        "DITA should have topic-related classes"
    );
}

#[test]
#[ignore = "Requires DITA schemas from pubbin repository"]
fn test_dita_basemap_schema_loads() {
    if !dita_schemas_available() {
        return;
    }

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let basemap_path = Path::new(manifest_dir)
        .join(DITA_BASE_PATH)
        .join("basemap.xsd");

    let xsd_schema = XsdSchema::from_xsd_file(&basemap_path, None::<&str>)
        .expect("Failed to parse DITA basemap.xsd");

    let generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).expect("Failed to generate schemas");

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
        class_names.iter().any(|n| n.contains("Map") || n.contains("map")),
        "DITA basemap should have map-related classes"
    );
}

// ============================================================================
// NISO-STS Schema Tests
// ============================================================================

/// Path to NISO-STS schemas relative to the pubbin repository
const NISO_STS_PATH: &str = "../../../reference-impl/lib/module/niso/schemas/ext/NISO-STS-extended-1-MathML3-XSD";

/// Check if NISO-STS schemas are available
fn niso_schemas_available() -> bool {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let niso_path = Path::new(manifest_dir).join(NISO_STS_PATH);
    niso_path.exists()
}

#[test]
#[ignore = "Requires NISO-STS schemas from pubbin repository"]
fn test_niso_sts_schema_loads() {
    if !niso_schemas_available() {
        println!("NISO-STS schemas not available, skipping test");
        return;
    }

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let niso_path = Path::new(manifest_dir)
        .join(NISO_STS_PATH)
        .join("NISO-STS-extended-1-mathml3.xsd");

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
#[ignore = "Requires NISO-STS schemas from pubbin repository"]
fn test_niso_sts_generates_schemas() {
    if !niso_schemas_available() {
        return;
    }

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let niso_path = Path::new(manifest_dir)
        .join(NISO_STS_PATH)
        .join("NISO-STS-extended-1-mathml3.xsd");

    let xsd_schema = XsdSchema::from_xsd_file(&niso_path, None::<&str>)
        .expect("Failed to parse NISO-STS schema");

    let generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).expect("Failed to generate schemas");

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
    println!("Sample NISO-STS classes: {:?}", &class_names[..class_names.len().min(20)]);

    assert!(
        schemas.len() > 50,
        "NISO-STS should generate many schemas, got {}",
        schemas.len()
    );
}

#[test]
#[ignore = "Requires NISO-STS schemas from pubbin repository"]
fn test_niso_sts_has_expected_elements() {
    if !niso_schemas_available() {
        return;
    }

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let niso_path = Path::new(manifest_dir)
        .join(NISO_STS_PATH)
        .join("NISO-STS-extended-1-mathml3.xsd");

    let model = XsdModel::from_file(&niso_path, None::<&str>)
        .expect("Failed to load NISO-STS model");

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
#[ignore = "Requires NISO-STS schemas from pubbin repository"]
fn test_niso_sts_schema_properties() {
    if !niso_schemas_available() {
        return;
    }

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let niso_path = Path::new(manifest_dir)
        .join(NISO_STS_PATH)
        .join("NISO-STS-extended-1-mathml3.xsd");

    let model = XsdModel::from_file(&niso_path, None::<&str>)
        .expect("Failed to load NISO-STS model");

    let schemas = model.schemas();

    // Verify all schemas have valid structure
    for schema in schemas {
        if let Schema::Class { id, properties, key, .. } = schema {
            // All properties should have valid names and classes
            for prop in properties {
                assert!(!prop.name.is_empty(), "Property name should not be empty in {}", id);
                assert!(!prop.class.is_empty(), "Property class should not be empty for {}.{}", id, prop.name);

                // Class should either be an XSD type or PascalCase
                if !prop.class.starts_with("xsd:") {
                    // Custom type should be PascalCase (first char uppercase)
                    let first_char = prop.class.chars().next().unwrap();
                    assert!(
                        first_char.is_uppercase() || prop.class.starts_with("xsd:"),
                        "Custom type {} should be PascalCase in {}.{}",
                        prop.class, id, prop.name
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
#[ignore = "Requires DITA schemas from pubbin repository"]
fn test_dita_directory_generation() {
    if !dita_schemas_available() {
        return;
    }

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let dita_dir = Path::new(manifest_dir).join(DITA_BASE_PATH);

    let generator = XsdToSchemaGenerator::new();

    // This tests the entry point detection and directory parsing
    let schemas = generator
        .generate_from_directory(&dita_dir, None::<&str>)
        .expect("Failed to generate from DITA directory");

    println!("Generated {} unique schemas from DITA directory", schemas.len());

    // Should deduplicate common types
    assert!(
        schemas.len() > 10,
        "Should generate schemas from DITA directory"
    );
}

#[test]
#[ignore = "Requires NISO-STS schemas from pubbin repository"]
fn test_niso_directory_generation() {
    if !niso_schemas_available() {
        return;
    }

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let niso_dir = Path::new(manifest_dir).join(NISO_STS_PATH);

    let generator = XsdToSchemaGenerator::new();

    let schemas = generator
        .generate_from_directory(&niso_dir, None::<&str>)
        .expect("Failed to generate from NISO-STS directory");

    println!("Generated {} unique schemas from NISO-STS directory", schemas.len());

    assert!(
        schemas.len() > 50,
        "Should generate many schemas from NISO-STS directory"
    );
}

// ============================================================================
// Schema Deduplication Tests
// ============================================================================

#[test]
#[ignore = "Requires NISO-STS schemas from pubbin repository"]
fn test_schema_deduplication() {
    if !niso_schemas_available() {
        return;
    }

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let niso_path = Path::new(manifest_dir)
        .join(NISO_STS_PATH)
        .join("NISO-STS-extended-1-mathml3.xsd");

    let xsd_schema = XsdSchema::from_xsd_file(&niso_path, None::<&str>)
        .expect("Failed to parse NISO-STS schema");

    let generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).unwrap();

    // Check for duplicates by class ID
    let mut seen_ids = std::collections::HashSet::new();
    let mut duplicates = Vec::new();

    for schema in &schemas {
        if let Schema::Class { id, .. } = schema {
            if !seen_ids.insert(id.clone()) {
                duplicates.push(id.clone());
            }
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
