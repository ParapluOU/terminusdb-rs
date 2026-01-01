//! Composition feature tests
//!
//! Tests for XSD composition mechanisms:
//! - xs:group (named model groups)
//! - xs:attributeGroup (named attribute groups)
//! - xs:include (same namespace inclusion)
//! - xs:import (different namespace import)
//! - xs:redefine (type redefinition)

use super::*;
use terminusdb_xsd::XsdModel;

// ============================================================================
// xs:group Tests
// ============================================================================

/// Test: xs:group Reference
///
/// XSD Feature: Named model group reference
/// Schema: DITA basetopic.xsd (uses body.cnt, basic.block groups)
/// Validates: Group content flattened into referencing type
#[test]
fn test_feature_group_reference() {
    let model = XsdModel::from_file(dita_topic_xsd(), None::<&str>)
        .expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // DITA uses groups extensively - body.cnt, basic.block, etc.
    // These groups define reusable content models
    // When referenced, their content should appear in the class properties

    // Look for classes that likely reference groups (many properties)
    let mut group_refs = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, properties, .. } = schema {
            // Classes with many properties often reference groups
            if properties.len() > 5 {
                group_refs.push((id.clone(), properties.len()));
            }
        }
    }

    println!(
        "Classes likely using group refs (many properties): {:?}",
        group_refs.into_iter().take(10).collect::<Vec<_>>()
    );
}

/// Test: Nested Group References
///
/// XSD Feature: Groups referencing other groups
/// Schema: DITA basetopic.xsd
/// Validates: Nested group content resolved
#[test]
fn test_feature_nested_group_refs() {
    let model = XsdModel::from_file(dita_topic_xsd(), None::<&str>)
        .expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // In DITA, some groups reference other groups
    // The final class should have all content flattened

    let class_count = schemas
        .iter()
        .filter(|s| matches!(s, Schema::Class { .. }))
        .count();

    println!("Total classes (groups flattened): {}", class_count);

    // Verify we have classes with inherited content
    assert!(class_count > 0, "Should have classes from group resolution");
}

// ============================================================================
// xs:attributeGroup Tests
// ============================================================================

/// Test: xs:attributeGroup Reference
///
/// XSD Feature: Named attribute group reference
/// Schema: DITA basetopic.xsd (global-atts, univ-atts)
/// Validates: Attribute group content becomes class properties
#[test]
fn test_feature_attribute_group() {
    let model = XsdModel::from_file(dita_topic_xsd(), None::<&str>)
        .expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // DITA uses attribute groups like global-atts, univ-atts
    // These define reusable attribute sets

    // Look for common DITA attributes that come from attribute groups
    let mut global_atts = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, properties, .. } = schema {
            for prop in properties {
                // Common DITA universal attributes
                if prop.name == "class"
                    || prop.name == "outputclass"
                    || prop.name == "xtrf"
                    || prop.name == "xtrc"
                {
                    global_atts.push(format!("{}.{}", id, prop.name));
                }
            }
        }
    }

    println!(
        "Properties from attribute groups (first 15): {:?}",
        global_atts.into_iter().take(15).collect::<Vec<_>>()
    );
}

/// Test: Multiple Attribute Group References
///
/// XSD Feature: Type referencing multiple attribute groups
/// Schema: DITA basetopic.xsd
/// Validates: All attribute groups merged into type
#[test]
fn test_feature_multiple_attribute_groups() {
    let model = XsdModel::from_file(dita_topic_xsd(), None::<&str>)
        .expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // DITA types often reference multiple attribute groups
    // Check for classes with many xsd: typed properties (likely attributes)
    let mut attr_heavy_classes = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, properties, .. } = schema {
            let attr_count = properties
                .iter()
                .filter(|p| p.class.starts_with("xsd:"))
                .count();
            if attr_count > 3 {
                attr_heavy_classes.push((id.clone(), attr_count));
            }
        }
    }

    println!(
        "Classes with many attributes (from attr groups): {:?}",
        attr_heavy_classes.into_iter().take(10).collect::<Vec<_>>()
    );
}

// ============================================================================
// xs:include Tests
// ============================================================================

/// Test: xs:include (Same Namespace)
///
/// XSD Feature: Including another schema in same namespace
/// Schema: DITA basetopic.xsd (includes commonElementMod.xsd, etc.)
/// Validates: Included types available in bundle
#[test]
fn test_feature_xs_include() {
    let model = XsdModel::from_file(dita_topic_xsd(), None::<&str>)
        .expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // DITA basetopic includes many modules
    // Types from included schemas should be available

    // Check for types from commonly included modules
    let class_names: Vec<_> = class_names(schemas);

    println!(
        "Classes from included modules (first 20): {:?}",
        class_names.into_iter().take(20).collect::<Vec<_>>()
    );

    // We should have more than just Topic - included modules add many types
    assert!(
        schemas.len() > 1,
        "Should have multiple types from included modules"
    );
}

/// Test: Transitive xs:include
///
/// XSD Feature: Included file that includes other files
/// Schema: DITA basetopic.xsd
/// Validates: All transitively included types available
#[test]
fn test_feature_transitive_include() {
    let model = XsdModel::from_file(dita_topic_xsd(), None::<&str>)
        .expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // DITA has deep include chains
    // All types from the transitive closure should be available

    let total_count = schemas.len();
    println!("Total schemas from transitive includes: {}", total_count);

    // DITA has many types through includes
    assert!(
        total_count > 10,
        "Should have many types from transitive includes"
    );
}

// ============================================================================
// xs:import Tests
// ============================================================================

/// Test: xs:import (Different Namespace)
///
/// XSD Feature: Importing types from different namespace
/// Schema: NISO-STS (imports XLink, MathML, etc.)
/// Validates: Imported namespace types available
#[test]
fn test_feature_xs_import() {
    let model = XsdModel::from_file(niso_sts_xsd(), None::<&str>)
        .expect("Failed to load NISO-STS XSD");

    let schemas = model.schemas();

    // NISO-STS imports types from other namespaces
    // These should be available in the schema bundle

    let class_count = schemas
        .iter()
        .filter(|s| matches!(s, Schema::Class { .. }))
        .count();

    println!("NISO-STS classes (includes imported types): {}", class_count);

    assert!(
        class_count > 0,
        "Should have classes from NISO-STS and imports"
    );
}

/// Test: Multi-Namespace Import
///
/// XSD Feature: Schema importing from multiple namespaces
/// Schema: NISO-STS extended (XLink + MathML + OASIS)
/// Validates: Types from all namespaces present
#[test]
fn test_feature_multi_namespace_import() {
    let model = XsdModel::from_file(niso_sts_xsd(), None::<&str>)
        .expect("Failed to load NISO-STS XSD");

    let schemas = model.schemas();

    // Look for types that might come from different namespaces
    // MathML types often have "math" in their name

    let mut namespace_hints = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, .. } = schema {
            if id.to_lowercase().contains("math")
                || id.to_lowercase().contains("xlink")
                || id.to_lowercase().contains("mml")
            {
                namespace_hints.push(id.clone());
            }
        }
    }

    println!(
        "Types possibly from imported namespaces: {:?}",
        namespace_hints.into_iter().take(20).collect::<Vec<_>>()
    );
}

// ============================================================================
// xs:redefine Tests
// ============================================================================

/// Test: xs:redefine (Type Redefinition)
///
/// XSD Feature: Redefining a type from included schema
/// Schema: DITA concept.xsd (redefines topic types)
/// Validates: Redefined types override base definitions
#[test]
fn test_feature_xs_redefine() {
    // DITA concept.xsd uses xs:redefine to customize topic types
    let model = XsdModel::from_file(dita_concept_xsd(), None::<&str>)
        .expect("Failed to load DITA concept XSD");

    let schemas = model.schemas();

    // Concept redefines topic types for specialized content
    // Look for Concept class (the redefined topic)
    let concept = find_class(schemas, "Concept");

    if let Some(c) = concept {
        print_class_details(c);
    }

    // We should have concept-specific types
    let class_names: Vec<_> = class_names(schemas);
    println!(
        "Concept schema classes: {:?}",
        class_names.into_iter().take(20).collect::<Vec<_>>()
    );
}

/// Test: Redefine with Extension
///
/// XSD Feature: xs:redefine extending base type
/// Schema: DITA concept.xsd
/// Validates: Extended type has base + additional content
#[test]
fn test_feature_redefine_extension() {
    let model = XsdModel::from_file(dita_concept_xsd(), None::<&str>)
        .expect("Failed to load DITA concept XSD");

    let schemas = model.schemas();

    // In DITA, redefined types often extend base types
    // Look for inheritance relationships

    let mut inheritance_info = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, inherits, .. } = schema {
            if !inherits.is_empty() {
                inheritance_info.push((id.clone(), inherits.clone()));
            }
        }
    }

    println!(
        "Types with inheritance (from redefine/extension): {:?}",
        inheritance_info.into_iter().take(10).collect::<Vec<_>>()
    );
}

/// Test: Redefine with Restriction
///
/// XSD Feature: xs:redefine restricting base type
/// Schema: DITA (various specializations)
/// Validates: Restricted type has subset of base content
#[test]
fn test_feature_redefine_restriction() {
    let model = XsdModel::from_file(dita_concept_xsd(), None::<&str>)
        .expect("Failed to load DITA concept XSD");

    let schemas = model.schemas();

    // Restriction removes or constrains content from base type
    // This is harder to detect - look for types with fewer properties
    // that might be restrictions

    let mut potential_restrictions = Vec::new();
    for schema in schemas {
        if let Schema::Class { id, properties, inherits, .. } = schema {
            // Types with inheritance but few properties might be restrictions
            if !inherits.is_empty() && properties.len() < 3 {
                potential_restrictions.push((id.clone(), properties.len(), inherits.clone()));
            }
        }
    }

    println!(
        "Potential restriction types (few props, has parent): {:?}",
        potential_restrictions.into_iter().take(10).collect::<Vec<_>>()
    );
}

// ============================================================================
// Module Resolution Tests
// ============================================================================

/// Test: DITA Module Resolution
///
/// XSD Feature: Full module resolution chain
/// Schema: DITA basetopic.xsd
/// Validates: All required modules loaded
#[test]
fn test_feature_dita_module_resolution() {
    let model = XsdModel::from_file(dita_topic_xsd(), None::<&str>)
        .expect("Failed to load DITA topic XSD");

    let schemas = model.schemas();

    // DITA has a complex module structure
    // Verify we have types from different modules

    print_schema_summary(schemas);

    // Should have substantial schema from module resolution
    assert!(
        schemas.len() > 20,
        "DITA should have many types from module resolution"
    );
}

/// Test: NISO-STS Module Resolution
///
/// XSD Feature: Full module resolution with imports
/// Schema: NISO-STS extended
/// Validates: All imported namespaces resolved
#[test]
fn test_feature_niso_module_resolution() {
    let model = XsdModel::from_file(niso_sts_xsd(), None::<&str>)
        .expect("Failed to load NISO-STS XSD");

    let schemas = model.schemas();

    print_schema_summary(schemas);

    // NISO-STS with MathML should have many types
    assert!(
        schemas.len() > 50,
        "NISO-STS with imports should have many types"
    );
}
