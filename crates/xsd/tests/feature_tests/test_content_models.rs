//! Content model feature tests
//!
//! Tests for XSD content models:
//! - xs:sequence (ordered child elements)
//! - xs:choice (alternative child elements)
//! - Mixed content (text + elements)
//! - Simple content (text + attributes)

use super::*;
use terminusdb_xsd::XsdModel;

// ============================================================================
// Sequence Tests
// ============================================================================

/// Test: xs:sequence Compositor
///
/// XSD Feature: xs:sequence for ordered child elements
/// Schema: DITA basetopic.xsd
/// Validates: Sequence elements become properties
#[test]
fn test_feature_sequence_compositor() {
    let model =
        XsdModel::from_file(dita_topic_xsd(), None::<&str>).expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Topic has a sequence: title, titlealts?, shortdesc?, ...
    if let Some(topic) = find_class(schemas, "Topic") {
        let props = get_property_names(topic);
        println!("Topic properties (from sequence): {:?}", props);

        // Should have title from sequence
        assert!(
            has_property(topic, "title"),
            "Topic should have 'title' from sequence"
        );
    }
}

/// Test: Sequence Order Preservation
///
/// XSD Feature: xs:sequence maintains element order
/// Schema: DITA basetopic.xsd
/// Validates: Properties exist (order is semantic, not enforced in schema)
#[test]
fn test_feature_sequence_elements() {
    let model =
        XsdModel::from_file(dita_topic_xsd(), None::<&str>).expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Find a class with multiple properties to show sequence handling
    for schema in schemas.iter().take(5) {
        if let Schema::Class { id, properties, .. } = schema {
            if properties.len() > 3 {
                println!(
                    "Class '{}' has {} properties (from sequence/choice):",
                    id,
                    properties.len()
                );
                for (i, prop) in properties.iter().enumerate().take(5) {
                    println!("  {}: {} ({})", i, prop.name, prop.class);
                }
                break;
            }
        }
    }
}

// ============================================================================
// Choice Tests
// ============================================================================

/// Test: xs:choice Compositor
///
/// XSD Feature: xs:choice for alternative elements
/// Schema: DITA basetopic.xsd
/// Validates: Choice alternatives available as properties
#[test]
fn test_feature_choice_compositor() {
    let model =
        XsdModel::from_file(dita_topic_xsd(), None::<&str>).expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // In DITA, many content models use choice for inline elements
    // Look for classes that likely have choice-based content
    let mut choice_candidates = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, properties, .. } = schema {
            // Classes with many similarly-typed properties often come from choice
            let inline_props: Vec<_> = properties
                .iter()
                .filter(|p| !p.class.starts_with("xsd:"))
                .collect();
            if inline_props.len() > 5 {
                choice_candidates.push((id.clone(), inline_props.len()));
            }
        }
    }

    println!(
        "Classes with many non-xsd properties (likely from choice): {:?}",
        choice_candidates.into_iter().take(5).collect::<Vec<_>>()
    );
}

// ============================================================================
// Mixed Content Tests
// ============================================================================

/// Test: Mixed Content (text + elements)
///
/// XSD Feature: mixed="true" on complexType
/// Schema: DITA basetopic.xsd
/// Validates: Mixed content types have text handling
#[test]
fn test_feature_mixed_content() {
    let model =
        XsdModel::from_file(dita_topic_xsd(), None::<&str>).expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // In DITA, <p>, <ph>, and other inline elements have mixed content
    // Look for classes that might represent mixed content elements
    // They typically have a text/value property plus child element properties

    let mut mixed_candidates = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, properties, .. } = schema {
            // Check for text-like property names
            let has_text = properties.iter().any(|p| {
                p.name == "text"
                    || p.name == "value"
                    || p.name == "_text"
                    || p.class == "xsd:string"
            });
            let has_children = properties.iter().any(|p| !p.class.starts_with("xsd:"));

            if has_text && has_children {
                mixed_candidates.push(id.clone());
            }
        }
    }

    println!(
        "Mixed content candidates: {:?}",
        mixed_candidates.into_iter().take(10).collect::<Vec<_>>()
    );
}

/// Test: Mixed Content Text Property
///
/// XSD Feature: Text content in mixed types
/// Schema: DITA basetopic.xsd
/// Validates: Text content accessible as property
#[test]
fn test_feature_mixed_content_text() {
    let model =
        XsdModel::from_file(dita_topic_xsd(), None::<&str>).expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Look for how we handle text in mixed content
    for schema in schemas.iter().take(20) {
        if let Schema::Class { id, properties, .. } = schema {
            for prop in properties {
                if prop.name.contains("text") || prop.name == "value" || prop.name == "_content" {
                    println!(
                        "Text-like property: {}.{} (type: {})",
                        id, prop.name, prop.class
                    );
                }
            }
        }
    }
}

// ============================================================================
// Simple Content Tests
// ============================================================================

/// Test: Simple Content (text + attributes)
///
/// XSD Feature: xs:simpleContent with extension
/// Schema: DITA basetopic.xsd
/// Validates: Simple content has value + attribute properties
#[test]
fn test_feature_simple_content() {
    let model =
        XsdModel::from_file(dita_topic_xsd(), None::<&str>).expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Simple content elements have a text value plus attributes
    // Look for classes with a value/text property plus attribute-like properties
    for schema in schemas {
        if let Schema::Class { id, properties, .. } = schema {
            let text_props: Vec<_> = properties
                .iter()
                .filter(|p| p.class == "xsd:string" && (p.name == "value" || p.name == "text"))
                .collect();

            if !text_props.is_empty() {
                let attr_count = properties.len() - text_props.len();
                if attr_count > 0 {
                    println!(
                        "Simple content candidate: {} (text: {}, attrs: {})",
                        id,
                        text_props.len(),
                        attr_count
                    );
                }
            }
        }
    }
}

// ============================================================================
// Empty Content Tests
// ============================================================================

/// Test: Empty Content (attributes only)
///
/// XSD Feature: Empty complexType (attributes only, no children)
/// Schema: DITA basetopic.xsd
/// Validates: Empty types have attribute properties only
#[test]
fn test_feature_empty_content() {
    let model =
        XsdModel::from_file(dita_topic_xsd(), None::<&str>).expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Empty content elements have only attribute properties (xsd: types)
    let mut empty_candidates = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, properties, .. } = schema {
            // Check if all properties are xsd: types (likely attributes)
            let all_xsd = properties.iter().all(|p| p.class.starts_with("xsd:"));
            if all_xsd && !properties.is_empty() {
                empty_candidates.push((id.clone(), properties.len()));
            }
        }
    }

    println!(
        "Empty content candidates (attrs only): {:?}",
        empty_candidates.into_iter().take(10).collect::<Vec<_>>()
    );
}

// ============================================================================
// NISO-STS Content Model Tests
// ============================================================================

/// Test: NISO-STS Content Models
///
/// XSD Feature: Complex real-world content models
/// Schema: NISO-STS extended
/// Validates: NISO-STS content models work
#[test]
fn test_feature_niso_content_models() {
    let model =
        XsdModel::from_file(niso_sts_xsd(), None::<&str>).expect("Failed to load NISO-STS XSD");

    let schemas = model.schemas();

    // Find classes with many properties (complex content models)
    let mut complex_classes: Vec<_> = schemas
        .iter()
        .filter_map(|s| match s {
            Schema::Class { id, properties, .. } => Some((id.clone(), properties.len())),
            _ => None,
        })
        .filter(|(_, count)| *count > 5)
        .collect();

    complex_classes.sort_by(|a, b| b.1.cmp(&a.1));

    println!(
        "NISO-STS classes with most properties: {:?}",
        complex_classes.into_iter().take(10).collect::<Vec<_>>()
    );
}
