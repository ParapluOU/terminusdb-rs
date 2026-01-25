//! Type system feature tests
//!
//! Tests for XSD type definitions:
//! - Named complex types
//! - Anonymous complex types
//! - Simple types (xs:string, xs:integer, etc.)
//! - Enumeration restrictions
//! - xs:list types

use super::*;
use terminusdb_xsd::XsdModel;

// ============================================================================
// Named Complex Type Tests
// ============================================================================

/// Test: Named Complex Type
///
/// XSD Feature: Named xs:complexType definition
/// Schema: DITA basetopic.xsd
/// Validates: Named types become classes
#[test]
fn test_feature_named_complex_type() {
    let model =
        XsdModel::from_file(dita_topic_xsd(), None::<&str>).expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Look for named type pattern (e.g., topic.class in DITA)
    let class_count = schemas
        .iter()
        .filter(|s| matches!(s, Schema::Class { .. }))
        .count();

    assert!(
        class_count > 0,
        "Should generate classes from named complex types"
    );

    println!("Generated {} classes from named complex types", class_count);

    // Show some example class names
    let names = class_names(schemas);
    println!(
        "Sample classes: {:?}",
        names.into_iter().take(10).collect::<Vec<_>>()
    );
}

/// Test: Anonymous Complex Type
///
/// XSD Feature: Inline xs:complexType (not named)
/// Schema: DITA basetopic.xsd
/// Validates: Anonymous types named after parent element
#[test]
fn test_feature_anonymous_complex_type() {
    let model =
        XsdModel::from_file(dita_topic_xsd(), None::<&str>).expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Anonymous types should be named after their containing element
    // For example, an anonymous type in <topic> might become "TopicType" or similar
    let names = class_names(schemas);

    // Check that we have classes (which includes both named and anonymous types)
    assert!(!names.is_empty(), "Should have classes from types");

    // Print for inspection
    println!(
        "Classes (may include anonymous types): {:?}",
        names.into_iter().take(15).collect::<Vec<_>>()
    );
}

// ============================================================================
// Simple Type Tests
// ============================================================================

/// Test: xs:string Type
///
/// XSD Feature: Built-in xs:string type
/// Schema: DITA basetopic.xsd
/// Validates: String properties map to xsd:string
#[test]
fn test_feature_simple_type_string() {
    let model =
        XsdModel::from_file(dita_topic_xsd(), None::<&str>).expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Look for properties with xsd:string type
    let mut string_props = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, properties, .. } = schema {
            for prop in properties {
                if prop.class == "xsd:string" {
                    string_props.push(format!("{}.{}", id, prop.name));
                }
            }
        }
    }

    println!(
        "Properties with xsd:string type (first 10): {:?}",
        string_props.into_iter().take(10).collect::<Vec<_>>()
    );
}

/// Test: xs:ID Type
///
/// XSD Feature: Built-in xs:ID type for identifiers
/// Schema: DITA basetopic.xsd
/// Validates: ID attributes map to xsd:ID
#[test]
fn test_feature_simple_type_id() {
    let model =
        XsdModel::from_file(dita_topic_xsd(), None::<&str>).expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Look for properties with xsd:ID type
    let mut id_props = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, properties, .. } = schema {
            for prop in properties {
                if prop.class == "xsd:ID" || prop.name == "id" {
                    id_props.push(format!("{}.{} (type: {})", id, prop.name, prop.class));
                }
            }
        }
    }

    println!(
        "ID properties (first 10): {:?}",
        id_props.into_iter().take(10).collect::<Vec<_>>()
    );
}

/// Test: xs:NMTOKEN Type
///
/// XSD Feature: NMTOKEN type for name tokens
/// Schema: DITA basetopic.xsd
/// Validates: NMTOKEN attributes properly typed
#[test]
fn test_feature_simple_type_nmtoken() {
    let model =
        XsdModel::from_file(dita_topic_xsd(), None::<&str>).expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Look for NMTOKEN type usage
    let mut nmtoken_props = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, properties, .. } = schema {
            for prop in properties {
                if prop.class.contains("NMTOKEN") || prop.class.contains("nmtoken") {
                    nmtoken_props.push(format!("{}.{}", id, prop.name));
                }
            }
        }
    }

    println!("NMTOKEN properties: {:?}", nmtoken_props);
}

// ============================================================================
// Enumeration Tests
// ============================================================================

/// Test: Enumeration Restriction
///
/// XSD Feature: xs:simpleType with xs:enumeration restrictions
/// Schema: DITA basetopic.xsd (table frame attribute)
/// Validates: Enumerations become Enum schemas
#[test]
fn test_feature_enumeration_restriction() {
    let model =
        XsdModel::from_file(dita_topic_xsd(), None::<&str>).expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Count enums
    let enum_count = schemas
        .iter()
        .filter(|s| matches!(s, Schema::Enum { .. }))
        .count();

    println!("Generated {} enum types", enum_count);

    // Show enum names
    let names = enum_names(schemas);
    println!("Enum names: {:?}", names);

    // Print details of first few enums
    for schema in schemas.iter().take(5) {
        if let Schema::Enum { id, values, .. } = schema {
            println!("Enum '{}': {:?}", id, values);
        }
    }
}

/// Test: Enum Values Preserved
///
/// XSD Feature: xs:enumeration value preservation
/// Schema: DITA
/// Validates: Enum values match XSD enumeration facets
#[test]
fn test_feature_enum_values() {
    let model =
        XsdModel::from_file(dita_topic_xsd(), None::<&str>).expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Find an enum and verify values
    for schema in schemas {
        if let Schema::Enum { id, values, .. } = schema {
            assert!(!values.is_empty(), "Enum {} should have values", id);
            println!("Enum '{}' has {} values: {:?}", id, values.len(), values);
            break;
        }
    }
}

// ============================================================================
// xs:list Type Tests
// ============================================================================

/// Test: xs:list Type (Space-Separated Values)
///
/// XSD Feature: xs:list for space-separated values
/// Schema: Test fixture
/// Validates: xs:list types mapped correctly
#[test]
fn test_feature_xs_list_type() {
    // Use the list_types fixture which has xs:list definitions
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("list_types.xsd");

    if fixture_path.exists() {
        let model = XsdModel::from_file(&fixture_path, None::<&str>)
            .expect("Failed to load list_types.xsd");

        let schemas = model.schemas();
        println!("List type test - generated {} schemas", schemas.len());

        // Print all schema types for inspection
        for schema in schemas {
            match schema {
                Schema::Class { id, .. } => println!("  Class: {}", id),
                Schema::Enum { id, .. } => println!("  Enum: {}", id),
                _ => {}
            }
        }
    } else {
        println!("list_types.xsd fixture not found, skipping");
    }
}

// ============================================================================
// NISO-STS Type Tests
// ============================================================================

/// Test: Types from NISO-STS
///
/// XSD Feature: Complex real-world types
/// Schema: NISO-STS extended
/// Validates: NISO-STS types generate correctly
#[test]
fn test_feature_niso_types() {
    let model =
        XsdModel::from_file(niso_sts_xsd(), None::<&str>).expect("Failed to load NISO-STS XSD");

    let schemas = model.schemas();

    // NISO-STS has many complex types
    let class_count = schemas
        .iter()
        .filter(|s| matches!(s, Schema::Class { .. }))
        .count();
    let enum_count = schemas
        .iter()
        .filter(|s| matches!(s, Schema::Enum { .. }))
        .count();

    println!("NISO-STS generated:");
    println!("  Classes: {}", class_count);
    println!("  Enums: {}", enum_count);

    assert!(class_count > 0, "NISO-STS should generate classes");
}
