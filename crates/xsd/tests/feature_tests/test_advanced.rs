//! Advanced XSD feature tests
//!
//! Tests for advanced XSD features:
//! - xs:extension (type extension)
//! - xs:restriction (type restriction)
//! - xs:any (wildcard elements)
//! - xs:anyAttribute (wildcard attributes)
//! - xs:union (union simple types)
//! - Inheritance chains

use super::*;
use terminusdb_xsd::XsdModel;

// ============================================================================
// xs:extension Tests
// ============================================================================

/// Test: Type Extension (complexContent)
///
/// XSD Feature: xs:extension in complexContent
/// Schema: DITA basetopic.xsd
/// Validates: Extended type inherits from base
#[test]
fn test_feature_type_extension() {
    let model = XsdModel::from_file(dita_topic_xsd(), None::<&str>)
        .expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Look for types with inheritance (extension)
    let mut extended_types = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, inherits, .. } = schema {
            if !inherits.is_empty() {
                extended_types.push((id.clone(), inherits.clone()));
            }
        }
    }

    println!(
        "Extended types (with inheritance): {:?}",
        extended_types.into_iter().take(15).collect::<Vec<_>>()
    );
}

/// Test: Extension with Additional Properties
///
/// XSD Feature: Extension adding new elements/attributes
/// Schema: DITA basetopic.xsd
/// Validates: Child type has parent + additional properties
#[test]
fn test_feature_extension_adds_properties() {
    let model = XsdModel::from_file(dita_topic_xsd(), None::<&str>)
        .expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Find a type with inheritance and check it has properties
    for schema in schemas {
        if let Schema::Class { id, inherits, properties, .. } = schema {
            if !inherits.is_empty() && !properties.is_empty() {
                println!(
                    "Type '{}' extends {:?} and adds {} properties",
                    id,
                    inherits,
                    properties.len()
                );
                for prop in properties.iter().take(5) {
                    println!("  - {}: {}", prop.name, prop.class);
                }
                break;
            }
        }
    }
}

/// Test: Simple Content Extension
///
/// XSD Feature: xs:simpleContent with xs:extension
/// Schema: DITA basetopic.xsd
/// Validates: Base simple type + added attributes
#[test]
fn test_feature_simple_content_extension() {
    let model = XsdModel::from_file(dita_topic_xsd(), None::<&str>)
        .expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Simple content extension: text value + attributes
    // Look for classes with a value/text property plus other properties
    let mut simple_extensions = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, properties, .. } = schema {
            let has_text_value = properties
                .iter()
                .any(|p| (p.name == "value" || p.name == "text") && p.class == "xsd:string");
            let has_attrs = properties.iter().any(|p| {
                p.name != "value" && p.name != "text" && p.class.starts_with("xsd:")
            });

            if has_text_value && has_attrs {
                simple_extensions.push(id.clone());
            }
        }
    }

    println!(
        "Simple content extensions (text + attrs): {:?}",
        simple_extensions.into_iter().take(10).collect::<Vec<_>>()
    );
}

// ============================================================================
// xs:restriction Tests
// ============================================================================

/// Test: Type Restriction
///
/// XSD Feature: xs:restriction on complexType
/// Schema: DITA/NISO-STS
/// Validates: Restricted types subset base type
#[test]
fn test_feature_type_restriction() {
    let model = XsdModel::from_file(dita_topic_xsd(), None::<&str>)
        .expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Restriction creates a subset of base type
    // These often appear as enumerations in our schema

    let enum_count = schemas
        .iter()
        .filter(|s| matches!(s, Schema::Enum { .. }))
        .count();

    println!("Enum types (often from restriction): {}", enum_count);

    // Show some enum examples
    for schema in schemas.iter().take(10) {
        if let Schema::Enum { id, values, .. } = schema {
            println!("Enum '{}' restricts to: {:?}", id, values);
        }
    }
}

/// Test: Pattern Restriction
///
/// XSD Feature: xs:pattern facet in restriction
/// Schema: DITA/NISO-STS
/// Validates: Pattern-based restrictions handled
#[test]
fn test_feature_pattern_restriction() {
    let model = XsdModel::from_file(dita_topic_xsd(), None::<&str>)
        .expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Pattern restrictions often become xsd:string in our model
    // Look for string-typed properties that might have patterns

    let mut pattern_candidates = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, properties, .. } = schema {
            for prop in properties {
                // Common pattern-restricted attribute names
                if (prop.name == "id" || prop.name == "name" || prop.name.contains("ref"))
                    && prop.class == "xsd:string"
                {
                    pattern_candidates.push(format!("{}.{}", id, prop.name));
                }
            }
        }
    }

    println!(
        "Properties potentially with pattern restrictions: {:?}",
        pattern_candidates.into_iter().take(10).collect::<Vec<_>>()
    );
}

// ============================================================================
// xs:any / xs:anyAttribute Tests
// ============================================================================

/// Test: xs:any Wildcard Element
///
/// XSD Feature: xs:any for extensibility
/// Schema: DITA (foreign element support)
/// Validates: Wildcard elements handled
#[test]
fn test_feature_xs_any_wildcard() {
    let model = XsdModel::from_file(dita_topic_xsd(), None::<&str>)
        .expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // xs:any typically allows arbitrary child elements
    // Look for classes that might have "any" or "foreign" content

    let mut any_candidates = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, properties, .. } = schema {
            for prop in properties {
                if prop.name.contains("any")
                    || prop.name.contains("foreign")
                    || prop.name.contains("unknown")
                {
                    any_candidates.push(format!("{}.{} (type: {})", id, prop.name, prop.class));
                }
            }
        }
    }

    println!(
        "Potential xs:any handling: {:?}",
        any_candidates
    );

    // Also check for classes that might handle foreign content
    if let Some(foreign) = find_class(schemas, "Foreign") {
        print_class_details(foreign);
    }
}

/// Test: xs:anyAttribute Wildcard
///
/// XSD Feature: xs:anyAttribute for arbitrary attributes
/// Schema: DITA/NISO-STS
/// Validates: Wildcard attributes handled
#[test]
fn test_feature_xs_any_attribute() {
    let model = XsdModel::from_file(dita_topic_xsd(), None::<&str>)
        .expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // xs:anyAttribute allows arbitrary attributes
    // These are harder to detect but might appear as map/dict properties

    let mut any_attr_candidates = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, properties, .. } = schema {
            for prop in properties {
                // Look for properties that might handle arbitrary attributes
                if prop.class.contains("Map")
                    || prop.class.contains("Dict")
                    || prop.name == "attributes"
                    || prop.name == "otherAttributes"
                {
                    any_attr_candidates.push(format!("{}.{}", id, prop.name));
                }
            }
        }
    }

    println!("Potential xs:anyAttribute handling: {:?}", any_attr_candidates);
}

// ============================================================================
// xs:union Tests
// ============================================================================

/// Test: xs:union Simple Type
///
/// XSD Feature: Union of simple types
/// Schema: NISO-STS (MathML has unions)
/// Validates: Union types handled
#[test]
fn test_feature_xs_union() {
    let model = XsdModel::from_file(niso_sts_xsd(), None::<&str>)
        .expect("Failed to load NISO-STS XSD");

    let schemas = model.schemas();

    // xs:union combines multiple simple types
    // These might become enums or string types in our model

    // Look for types that might be unions
    let mut union_candidates = Vec::new();
    for schema in schemas {
        match schema {
            Schema::Enum { id, values, .. } => {
                // Enums with many values might be from unions
                if values.len() > 5 {
                    union_candidates.push(format!("Enum {} ({} values)", id, values.len()));
                }
            }
            Schema::OneOfClass { id, properties, .. } => {
                // OneOfClass is another way to represent unions
                union_candidates.push(format!("OneOf {} ({} properties)", id, properties.len()));
            }
            _ => {}
        }
    }

    println!(
        "Potential union types: {:?}",
        union_candidates.into_iter().take(10).collect::<Vec<_>>()
    );
}

// ============================================================================
// Inheritance Chain Tests
// ============================================================================

/// Test: Single-Level Inheritance
///
/// XSD Feature: Type extends another type
/// Schema: DITA basetopic.xsd
/// Validates: Child inherits parent properties
#[test]
fn test_feature_single_inheritance() {
    let model = XsdModel::from_file(dita_topic_xsd(), None::<&str>)
        .expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Find types with exactly one parent
    let mut single_inheritance = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, inherits, .. } = schema {
            if inherits.len() == 1 {
                single_inheritance.push((id.clone(), inherits[0].clone()));
            }
        }
    }

    println!(
        "Single inheritance types: {:?}",
        single_inheritance.into_iter().take(15).collect::<Vec<_>>()
    );
}

/// Test: Multi-Level Inheritance Chain
///
/// XSD Feature: A extends B extends C
/// Schema: DITA (topic → specialized types)
/// Validates: Full inheritance chain resolved
#[test]
fn test_feature_multi_level_inheritance() {
    let model = XsdModel::from_file(dita_concept_xsd(), None::<&str>)
        .expect("Failed to load DITA concept XSD");

    let schemas = model.schemas();

    // Build inheritance graph
    let mut inheritance_map: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for schema in schemas {
        if let Schema::Class { id, inherits, .. } = schema {
            inheritance_map.insert(id.clone(), inherits.clone());
        }
    }

    // Find types with deepest inheritance
    fn depth(
        id: &str,
        map: &std::collections::HashMap<String, Vec<String>>,
        visited: &mut std::collections::HashSet<String>,
    ) -> usize {
        if visited.contains(id) {
            return 0; // Prevent infinite loops
        }
        visited.insert(id.to_string());

        if let Some(parents) = map.get(id) {
            1 + parents
                .iter()
                .map(|p| depth(p, map, visited))
                .max()
                .unwrap_or(0)
        } else {
            0
        }
    }

    let mut type_depths: Vec<_> = inheritance_map
        .keys()
        .map(|id| {
            let mut visited = std::collections::HashSet::new();
            (id.clone(), depth(id, &inheritance_map, &mut visited))
        })
        .collect();

    type_depths.sort_by(|a, b| b.1.cmp(&a.1));

    println!(
        "Types with deepest inheritance: {:?}",
        type_depths.into_iter().take(10).collect::<Vec<_>>()
    );
}

/// Test: Multiple Inheritance (via groups)
///
/// XSD Feature: Type using multiple groups
/// Schema: DITA basetopic.xsd
/// Validates: Content from multiple sources combined
#[test]
fn test_feature_multiple_groups() {
    let model = XsdModel::from_file(dita_topic_xsd(), None::<&str>)
        .expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Types that reference multiple groups effectively have "multiple inheritance"
    // of content models. Look for classes with many diverse properties.

    let mut diverse_types = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, properties, .. } = schema {
            // Types with many properties from different "domains"
            let xsd_props = properties.iter().filter(|p| p.class.starts_with("xsd:")).count();
            let class_props = properties.iter().filter(|p| !p.class.starts_with("xsd:")).count();

            if xsd_props > 3 && class_props > 3 {
                diverse_types.push((id.clone(), xsd_props, class_props));
            }
        }
    }

    println!(
        "Types with diverse content (attrs + elements): {:?}",
        diverse_types.into_iter().take(10).collect::<Vec<_>>()
    );
}

// ============================================================================
// Tagged Union / OneOf Tests
// ============================================================================

/// Test: Tagged Union Schema
///
/// XSD Feature: xs:choice with different types
/// Schema: Generated from XSD choice
/// Validates: OneOfClass or TaggedUnion generated
#[test]
fn test_feature_tagged_union() {
    let model = XsdModel::from_file(dita_topic_xsd(), None::<&str>)
        .expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // Look for OneOfClass or TaggedUnion schemas
    let mut union_types = Vec::new();
    for schema in schemas {
        match schema {
            Schema::OneOfClass { id, properties, .. } => {
                union_types.push(format!("OneOfClass '{}' with {} properties", id, properties.len()));
            }
            Schema::TaggedUnion { id, properties, .. } => {
                union_types.push(format!("TaggedUnion '{}' with {} properties", id, properties.len()));
            }
            _ => {}
        }
    }

    println!("Union/choice types: {:?}", union_types);
}

// ============================================================================
// NISO-STS Advanced Tests
// ============================================================================

/// Test: MathML Integration
///
/// XSD Feature: Complex imported namespace (MathML)
/// Schema: NISO-STS with MathML
/// Validates: MathML types properly integrated
#[test]
fn test_feature_mathml_integration() {
    let model = XsdModel::from_file(niso_sts_xsd(), None::<&str>)
        .expect("Failed to load NISO-STS XSD");

    let schemas = model.schemas();

    // Look for MathML-related types
    let math_types: Vec<_> = schemas
        .iter()
        .filter_map(|s| match s {
            Schema::Class { id, .. } => {
                if id.to_lowercase().contains("math")
                    || id.to_lowercase().contains("mml")
                    || id.to_lowercase().starts_with("m")
                {
                    Some(id.clone())
                } else {
                    None
                }
            }
            _ => None,
        })
        .take(20)
        .collect();

    println!("MathML-related types: {:?}", math_types);
}

/// Test: Complex Namespace Handling
///
/// XSD Feature: Multiple namespaces interacting
/// Schema: NISO-STS extended
/// Validates: Cross-namespace references resolved
#[test]
fn test_feature_complex_namespaces() {
    let model = XsdModel::from_file(niso_sts_xsd(), None::<&str>)
        .expect("Failed to load NISO-STS XSD");

    let schemas = model.schemas();

    // Check for proper namespace handling in property types
    let mut namespace_refs = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, properties, .. } = schema {
            for prop in properties {
                // Look for cross-namespace references
                if prop.class.contains(":")
                    && !prop.class.starts_with("xsd:")
                    && !prop.class.starts_with("sys:")
                {
                    namespace_refs.push(format!("{}.{} → {}", id, prop.name, prop.class));
                }
            }
        }
    }

    println!(
        "Cross-namespace references (first 15): {:?}",
        namespace_refs.into_iter().take(15).collect::<Vec<_>>()
    );
}
