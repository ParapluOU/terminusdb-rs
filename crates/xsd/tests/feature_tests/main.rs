//! Feature-based XSD test infrastructure
//!
//! This module provides shared helpers for feature-based XSD tests.
//! Each feature test validates a single XSD feature through the full pipeline:
//!
//! 1. Load XSD schema (from real DITA or NISO-STS schemas)
//! 2. Generate TerminusDB Schema
//! 3. Create minimal XML fixture demonstrating the feature
//! 4. Validate XML against XSD bundle
//! 5. Parse XML to TerminusDB Instances
//! 6. Insert into TerminusDBServer

use schemas_dita::{Dita12, SchemaBundle};
use schemas_niso_sts::NisoSts;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::LazyLock;
use tempfile::TempDir;
use terminusdb_schema::Schema;

// Include test modules
pub mod test_elements;
pub mod test_types;
pub mod test_content_models;
pub mod test_composition;
pub mod test_advanced;

// ============================================================================
// Lazy Schema Extraction
// ============================================================================

/// Lazily extracted DITA 1.2 schemas (shared across all tests)
pub static DITA_DIR: LazyLock<TempDir> = LazyLock::new(|| {
    let dir = TempDir::new().expect("Failed to create temp dir for DITA schemas");
    Dita12::write_to_directory(dir.path()).expect("Failed to extract DITA schemas");
    dir
});

/// Lazily extracted NISO-STS schemas (shared across all tests)
pub static NISO_DIR: LazyLock<TempDir> = LazyLock::new(|| {
    let dir = TempDir::new().expect("Failed to create temp dir for NISO schemas");
    NisoSts::write_to_directory(dir.path()).expect("Failed to extract NISO schemas");
    dir
});

// ============================================================================
// Path Helpers
// ============================================================================

/// Get path to a DITA schema file relative to the extracted bundle
///
/// # Example
/// ```ignore
/// let path = dita_path("xsd1.2-url/base/xsd/basetopic.xsd");
/// ```
pub fn dita_path(relative: &str) -> PathBuf {
    DITA_DIR.path().join(relative)
}

/// Get path to a NISO-STS schema file relative to the extracted bundle
///
/// # Example
/// ```ignore
/// let path = niso_path("NISO-STS-extended-1-MathML3-XSD/NISO-STS-extended-1-mathml3.xsd");
/// ```
pub fn niso_path(relative: &str) -> PathBuf {
    NISO_DIR.path().join(relative)
}

/// Standard path to DITA basetopic.xsd
pub fn dita_topic_xsd() -> PathBuf {
    dita_path("xsd1.2-url/base/xsd/basetopic.xsd")
}

/// Standard path to DITA concept.xsd (uses xs:redefine)
pub fn dita_concept_xsd() -> PathBuf {
    dita_path("xsd1.2-url/technicalContent/xsd/concept.xsd")
}

/// Standard path to DITA table module
pub fn dita_table_mod() -> PathBuf {
    dita_path("xsd1.2-url/base/xsd/tblDeclMod.xsd")
}

/// Standard path to NISO-STS extended schema
pub fn niso_sts_xsd() -> PathBuf {
    niso_path("NISO-STS-extended-1-MathML3-XSD/NISO-STS-extended-1-mathml3.xsd")
}

/// Path to feature test fixtures directory
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("feature_tests")
        .join("fixtures")
}

/// Load a fixture XML file by name (without .xml extension)
pub fn load_fixture(name: &str) -> String {
    let path = fixtures_dir().join(format!("{}.xml", name));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to load fixture '{}' from {:?}: {}", name, path, e))
}

// ============================================================================
// Schema Helpers
// ============================================================================

/// Find a class schema by name (case-insensitive prefix match)
pub fn find_class<'a>(schemas: &'a [Schema], name: &str) -> Option<&'a Schema> {
    let name_lower = name.to_lowercase();
    schemas.iter().find(|s| match s {
        Schema::Class { id, .. } => id.to_lowercase() == name_lower,
        _ => false,
    })
}

/// Find a class schema by exact name
pub fn find_class_exact<'a>(schemas: &'a [Schema], name: &str) -> Option<&'a Schema> {
    schemas.iter().find(|s| match s {
        Schema::Class { id, .. } => id == name,
        _ => false,
    })
}

/// Find an enum schema by name
pub fn find_enum<'a>(schemas: &'a [Schema], name: &str) -> Option<&'a Schema> {
    let name_lower = name.to_lowercase();
    schemas.iter().find(|s| match s {
        Schema::Enum { id, .. } => id.to_lowercase() == name_lower,
        _ => false,
    })
}

/// Check if a class has a property with the given name
pub fn has_property(schema: &Schema, prop_name: &str) -> bool {
    match schema {
        Schema::Class { properties, .. } => {
            properties.iter().any(|p| p.name == prop_name)
        }
        _ => false,
    }
}

/// Get property type for a class property
pub fn get_property_type(schema: &Schema, prop_name: &str) -> Option<String> {
    match schema {
        Schema::Class { properties, .. } => {
            properties.iter()
                .find(|p| p.name == prop_name)
                .map(|p| p.class.clone())
        }
        _ => None,
    }
}

/// Get all property names from a class schema
pub fn get_property_names(schema: &Schema) -> Vec<&str> {
    match schema {
        Schema::Class { properties, .. } => {
            properties.iter().map(|p| p.name.as_str()).collect()
        }
        _ => vec![],
    }
}

/// Check if a class inherits from another class
pub fn inherits_from(schema: &Schema, parent_name: &str) -> bool {
    match schema {
        Schema::Class { inherits, .. } => inherits.contains(&parent_name.to_string()),
        _ => false,
    }
}

/// Get all class names from schemas
pub fn class_names(schemas: &[Schema]) -> Vec<&str> {
    schemas
        .iter()
        .filter_map(|s| match s {
            Schema::Class { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect()
}

/// Get all enum names from schemas
pub fn enum_names(schemas: &[Schema]) -> Vec<&str> {
    schemas
        .iter()
        .filter_map(|s| match s {
            Schema::Enum { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect()
}

// ============================================================================
// Schema Dependency Helpers
// ============================================================================

/// Collect all defined type names (classes and enums)
pub fn defined_types(schemas: &[Schema]) -> HashSet<String> {
    schemas
        .iter()
        .filter_map(|s| match s {
            Schema::Class { id, .. } => Some(id.clone()),
            Schema::Enum { id, .. } => Some(id.clone()),
            Schema::OneOfClass { id, .. } => Some(id.clone()),
            Schema::TaggedUnion { id, .. } => Some(id.clone()),
        })
        .collect()
}

/// Find all type references that are not defined (excluding builtins)
pub fn find_missing_dependencies(schemas: &[Schema]) -> Vec<String> {
    let defined = defined_types(schemas);
    let mut referenced: HashSet<String> = HashSet::new();

    for schema in schemas {
        if let Schema::Class { properties, inherits, .. } = schema {
            for prop in properties {
                // Skip builtins: xsd: primitives and sys: system types
                if !prop.class.starts_with("xsd:") && !prop.class.starts_with("sys:") {
                    referenced.insert(prop.class.clone());
                }
            }
            for parent in inherits {
                referenced.insert(parent.clone());
            }
        }
    }

    referenced.difference(&defined).cloned().collect()
}

/// Filter schemas to only include those with all dependencies satisfied
pub fn filter_valid_schemas(schemas: &[Schema]) -> Vec<Schema> {
    let defined = defined_types(schemas);

    schemas
        .iter()
        .filter(|s| {
            if let Schema::Class { properties, inherits, .. } = s {
                let props_ok = properties.iter().all(|p| {
                    p.class.starts_with("xsd:")
                        || p.class.starts_with("sys:")
                        || defined.contains(&p.class)
                });
                let inherits_ok = inherits.iter().all(|parent| defined.contains(parent));
                props_ok && inherits_ok
            } else {
                true
            }
        })
        .cloned()
        .collect()
}

// ============================================================================
// Test Assertion Helpers
// ============================================================================

/// Assert that a class exists with the given name
#[track_caller]
pub fn assert_class_exists(schemas: &[Schema], name: &str) {
    assert!(
        find_class(schemas, name).is_some(),
        "Expected class '{}' to exist in schemas. Available classes: {:?}",
        name,
        class_names(schemas).into_iter().take(20).collect::<Vec<_>>()
    );
}

/// Assert that a class has a specific property
#[track_caller]
pub fn assert_has_property(schemas: &[Schema], class_name: &str, prop_name: &str) {
    let schema = find_class(schemas, class_name)
        .unwrap_or_else(|| panic!("Class '{}' not found", class_name));
    assert!(
        has_property(schema, prop_name),
        "Expected class '{}' to have property '{}'. Properties: {:?}",
        class_name,
        prop_name,
        get_property_names(schema)
    );
}

/// Assert that an enum exists with the given name
#[track_caller]
pub fn assert_enum_exists(schemas: &[Schema], name: &str) {
    assert!(
        find_enum(schemas, name).is_some(),
        "Expected enum '{}' to exist in schemas. Available enums: {:?}",
        name,
        enum_names(schemas)
    );
}

// ============================================================================
// Debug Helpers
// ============================================================================

/// Print schema summary for debugging
pub fn print_schema_summary(schemas: &[Schema]) {
    let class_count = schemas.iter().filter(|s| matches!(s, Schema::Class { .. })).count();
    let enum_count = schemas.iter().filter(|s| matches!(s, Schema::Enum { .. })).count();

    println!("Schema Summary:");
    println!("  Total: {}", schemas.len());
    println!("  Classes: {}", class_count);
    println!("  Enums: {}", enum_count);
}

/// Print class details for debugging
pub fn print_class_details(schema: &Schema) {
    if let Schema::Class { id, properties, inherits, .. } = schema {
        println!("Class: {}", id);
        if !inherits.is_empty() {
            println!("  Inherits: {:?}", inherits);
        }
        println!("  Properties ({}):", properties.len());
        for prop in properties.iter().take(10) {
            println!("    - {}: {} ({})", prop.name, prop.class, if prop.r#type.is_some() { "array" } else { "single" });
        }
        if properties.len() > 10 {
            println!("    ... and {} more", properties.len() - 10);
        }
    }
}
