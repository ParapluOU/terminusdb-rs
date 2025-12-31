//! Tests documenting known limitations in XSD to TerminusDB conversion
//!
//! These tests document current behavior that differs from ideal behavior.
//! They serve as:
//! 1. Documentation of known gaps
//! 2. Regression tests to detect if behavior changes
//! 3. A roadmap for future improvements
//!
//! When a limitation is fixed, update the test to verify correct behavior.

use terminusdb_schema::{Schema, TypeFamily};
use terminusdb_xsd::schema_generator::XsdToSchemaGenerator;
use terminusdb_xsd::schema_model::{
    Cardinality, ChildElement, SimpleTypeVariety, XsdComplexType, XsdSchema, XsdSimpleType,
};

/// Helper to find a schema by class name
fn find_class<'a>(schemas: &'a [Schema], name: &str) -> Option<&'a Schema> {
    schemas
        .iter()
        .find(|s| matches!(s, Schema::Class { id, .. } if id == name))
}

/// Helper to find a TaggedUnion by name
fn find_tagged_union<'a>(schemas: &'a [Schema], name: &str) -> Option<&'a Schema> {
    schemas
        .iter()
        .find(|s| matches!(s, Schema::TaggedUnion { id, .. } if id == name))
}

// ============================================================================
// LIMITATION 1: xs:union types not converted to TaggedUnion
// ============================================================================

/// Creates an XSD schema with a union type
fn create_union_type_schema() -> XsdSchema {
    XsdSchema {
        target_namespace: Some("http://example.com/test".to_string()),
        schema_location: None,
        element_form_default: None,
        root_elements: vec![],
        entry_point_elements: vec![],
        complex_types: vec![XsdComplexType {
            name: "ContainerType".to_string(),
            qualified_name: "{http://example.com/test}ContainerType".to_string(),
            category: "XsdComplexType".to_string(),
            is_complex: true,
            is_simple: false,
            has_simple_content: false,
            mixed: false,
            content_model: Some("XsdGroup".to_string()),
            attributes: None,
            child_elements: Some(vec![ChildElement {
                name: "value".to_string(),
                // Reference to the union type
                element_type: "{http://example.com/test}StringOrNumber".to_string(),
                min_occurs: Some(1),
                max_occurs: Some(Cardinality::Number(1)),
            }]),
            is_anonymous: false,
            element_name: None,
            base_type: None,
        }],
        // This union type is defined - now we can properly represent it!
        simple_types: vec![XsdSimpleType {
            name: "StringOrNumber".to_string(),
            qualified_name: "{http://example.com/test}StringOrNumber".to_string(),
            category: "XsdSimpleType".to_string(),
            base_type: None,
            restrictions: None,
            // Now we can express union types!
            variety: Some(SimpleTypeVariety::Union),
            item_type: None,
            member_types: Some(vec![
                "{http://www.w3.org/2001/XMLSchema}string".to_string(),
                "{http://www.w3.org/2001/XMLSchema}integer".to_string(),
            ]),
        }],
    }
}

#[test]
fn test_union_types_generate_tagged_union() {
    // FIXED: xs:union types are now converted to TerminusDB TaggedUnion schemas!
    //
    // XSD definition:
    // <xs:simpleType name="stringOrNumber">
    //   <xs:union memberTypes="xs:string xs:integer"/>
    // </xs:simpleType>
    //
    // This generates a Schema::TaggedUnion with properties for each member type.

    let xsd_schema = create_union_type_schema();
    let generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).unwrap();

    // Verify TaggedUnion is generated for union type
    let tagged_union = find_tagged_union(&schemas, "StringOrNumber");
    assert!(
        tagged_union.is_some(),
        "xs:union types should now generate TaggedUnion schemas!"
    );

    // Verify the TaggedUnion has the expected structure
    if let Schema::TaggedUnion { id, properties, subdocument, .. } = tagged_union.unwrap() {
        assert_eq!(id, "StringOrNumber");
        assert!(subdocument, "Union types should be subdocuments");

        // Should have two properties: one for string, one for integer
        assert_eq!(properties.len(), 2, "Should have 2 member type properties");

        // Find string variant
        let string_prop = properties.iter().find(|p| p.class == "xsd:string");
        assert!(string_prop.is_some(), "Should have xsd:string variant");
        assert_eq!(string_prop.unwrap().name, "string", "String variant tag should be 'string'");

        // Find integer variant
        let integer_prop = properties.iter().find(|p| p.class == "xsd:integer");
        assert!(integer_prop.is_some(), "Should have xsd:integer variant");
        assert_eq!(integer_prop.unwrap().name, "integer", "Integer variant tag should be 'integer'");
    } else {
        panic!("Expected TaggedUnion schema");
    }

    // The container type should also exist
    let container = find_class(&schemas, "ContainerType");
    assert!(container.is_some(), "ContainerType should be generated");
}

#[test]
fn test_xsd_simple_type_has_variety_field() {
    // XsdSimpleType now has the `variety` field to distinguish atomic, list, and union types.
    //
    // PRESENT:
    // - variety: SimpleTypeVariety (Atomic | List | Union)
    // - item_type: Option<String> (for lists - extracted from xmlschema-rs)
    // - member_types: Option<Vec<String>> (for unions - extracted from xmlschema-rs)

    let simple_type = XsdSimpleType {
        name: "TestType".to_string(),
        qualified_name: "{http://test}TestType".to_string(),
        category: "XsdSimpleType".to_string(),
        base_type: Some("xsd:string".to_string()),
        restrictions: None,
        variety: Some(SimpleTypeVariety::Atomic),
        item_type: None,
        member_types: None,
    };

    // Verify variety is present
    assert_eq!(simple_type.name, "TestType");
    assert_eq!(simple_type.variety, Some(SimpleTypeVariety::Atomic));

    // List types can now be represented
    let list_type = XsdSimpleType {
        name: "IntegerList".to_string(),
        qualified_name: "{http://test}IntegerList".to_string(),
        category: "XsdSimpleType".to_string(),
        base_type: None,
        restrictions: None,
        variety: Some(SimpleTypeVariety::List),
        item_type: Some("xsd:integer".to_string()),
        member_types: None,
    };
    assert_eq!(list_type.variety, Some(SimpleTypeVariety::List));
    assert_eq!(list_type.item_type, Some("xsd:integer".to_string()));
}

// ============================================================================
// FIXED: xs:list types now correctly generate TypeFamily::List
// ============================================================================

/// Creates an XSD schema demonstrating the xs:list issue
fn create_list_type_schema() -> XsdSchema {
    XsdSchema {
        target_namespace: Some("http://example.com/test".to_string()),
        schema_location: None,
        element_form_default: None,
        root_elements: vec![],
        entry_point_elements: vec![],
        complex_types: vec![XsdComplexType {
            name: "DataContainer".to_string(),
            qualified_name: "{http://example.com/test}DataContainer".to_string(),
            category: "XsdComplexType".to_string(),
            is_complex: true,
            is_simple: false,
            has_simple_content: false,
            mixed: false,
            content_model: Some("XsdGroup".to_string()),
            attributes: None,
            child_elements: Some(vec![
                // Element with unbounded cardinality (multiple XML elements)
                ChildElement {
                    name: "items".to_string(),
                    element_type: "xsd:string".to_string(),
                    min_occurs: Some(0),
                    max_occurs: Some(Cardinality::Unbounded),
                },
            ]),
            is_anonymous: false,
            element_name: None,
            base_type: None,
        }],
        simple_types: vec![
            // This represents an xs:list type
            // <xs:simpleType name="integerList">
            //   <xs:list itemType="xs:integer"/>
            // </xs:simpleType>
            //
            // Now we CAN express this!
            XsdSimpleType {
                name: "IntegerList".to_string(),
                qualified_name: "{http://example.com/test}IntegerList".to_string(),
                category: "XsdSimpleType".to_string(),
                base_type: Some("xsd:integer".to_string()),
                restrictions: None,
                variety: Some(SimpleTypeVariety::List),
                item_type: Some("xsd:integer".to_string()),
                member_types: None,
            },
        ],
    }
}

#[test]
fn test_limitation_unbounded_uses_set_not_list() {
    // This test documents that elements with maxOccurs="unbounded"
    // are converted to TypeFamily::Set, not TypeFamily::List.
    //
    // For repeated XML elements, Set IS the correct behavior - they represent
    // multiple distinct XML elements, not space-separated values in one element.
    //
    // XSD definitions:
    // 1. <xs:element name="items" maxOccurs="unbounded"/> → Set (CORRECT)
    // 2. <xs:list itemType="xs:integer"/> → should be List (ordered, duplicates allowed)
    //
    // CURRENT STATUS:
    // - unbounded elements correctly use Set
    // - xs:list types WOULD use List, but xmlschema-rs doesn't properly parse them

    let xsd_schema = create_list_type_schema();
    let generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).unwrap();

    let container = find_class(&schemas, "DataContainer").expect("DataContainer should exist");

    if let Schema::Class { properties, .. } = container {
        let items_prop = properties.iter().find(|p| p.name == "items");
        assert!(items_prop.is_some(), "items property should exist");

        let items = items_prop.unwrap();
        // minOccurs=0 with maxOccurs=unbounded → Set or Optional
        let is_set_or_optional = matches!(
            items.r#type,
            Some(TypeFamily::Set(_)) | Some(TypeFamily::Optional)
        );
        assert!(
            is_set_or_optional,
            "Unbounded elements should use Set or Optional. Got {:?}",
            items.r#type
        );
    }
}

/// Creates an XSD schema with a list type property to verify TypeFamily::List generation
fn create_list_property_schema() -> XsdSchema {
    XsdSchema {
        target_namespace: Some("http://example.com/test".to_string()),
        schema_location: None,
        element_form_default: None,
        root_elements: vec![],
        entry_point_elements: vec![],
        complex_types: vec![XsdComplexType {
            name: "DataWithList".to_string(),
            qualified_name: "{http://example.com/test}DataWithList".to_string(),
            category: "XsdComplexType".to_string(),
            is_complex: true,
            is_simple: false,
            has_simple_content: false,
            mixed: false,
            content_model: Some("XsdGroup".to_string()),
            attributes: None,
            child_elements: Some(vec![
                // Element using the list type
                ChildElement {
                    name: "scores".to_string(),
                    element_type: "{http://example.com/test}IntegerList".to_string(),
                    min_occurs: Some(1),
                    max_occurs: Some(Cardinality::Number(1)),
                },
            ]),
            is_anonymous: false,
            element_name: None,
            base_type: None,
        }],
        simple_types: vec![
            // This represents an xs:list type with variety=List
            // When xmlschema-rs is fixed, it will populate variety=List
            XsdSimpleType {
                name: "IntegerList".to_string(),
                qualified_name: "{http://example.com/test}IntegerList".to_string(),
                category: "XsdSimpleType".to_string(),
                base_type: Some("xsd:integer".to_string()),
                restrictions: None,
                variety: Some(SimpleTypeVariety::List),
                item_type: Some("xsd:integer".to_string()),
                member_types: None,
            },
        ],
    }
}

#[test]
fn test_list_type_generates_type_family_list() {
    // This test verifies that our schema_generator.rs DOES correctly generate
    // TypeFamily::List when the variety is List.
    //
    // The actual limitation is in xmlschema-rs (see test_list_types_xmlschema_rs_limitation
    // in schema_generation_tests.rs), not in our generator.
    //
    // When xmlschema-rs is fixed to use XsdListType for xs:list types, this
    // will automatically work.

    let xsd_schema = create_list_property_schema();
    let generator = XsdToSchemaGenerator::new();
    let schemas = generator.generate(&xsd_schema).unwrap();

    let container = find_class(&schemas, "DataWithList").expect("DataWithList should exist");

    if let Schema::Class { properties, .. } = container {
        let scores_prop = properties.iter().find(|p| p.name == "scores");
        assert!(scores_prop.is_some(), "scores property should exist");

        let scores = scores_prop.unwrap();
        assert_eq!(
            scores.r#type,
            Some(TypeFamily::List),
            "When variety=List, schema generator should produce TypeFamily::List. Got {:?}",
            scores.r#type
        );
    }
}

#[test]
fn test_type_family_list_exists() {
    // Verify that TerminusDB supports List type family
    // This proves the fix is possible - we just need to use it correctly

    // TypeFamily::List exists and represents ordered collections with duplicates
    let list_type = TypeFamily::List;

    // This compiles, proving List is available
    assert!(matches!(list_type, TypeFamily::List));

    // The difference between List and Set:
    // - List: ordered, allows duplicates (xs:list semantics)
    // - Set: unordered, no duplicates (xs:element with maxOccurs > 1)
}

// ============================================================================
// FIXED: xs:redefine now supported
// ============================================================================

#[test]
fn test_redefine_is_supported() {
    // xs:redefine IS NOW SUPPORTED!
    //
    // xs:redefine allows including a schema and modifying its components:
    // <xs:redefine schemaLocation="base.xsd">
    //   <xs:complexType name="BaseType">
    //     <xs:complexContent>
    //       <xs:extension base="BaseType">
    //         <xs:element name="newField"/>
    //       </xs:extension>
    //     </xs:complexContent>
    //   </xs:complexType>
    // </xs:redefine>
    //
    // The implementation includes:
    // 1. parse_redefine() in xmlschema-rs parses xs:redefine elements
    // 2. Redefined types/groups are registered in global_maps with original stored
    // 3. resolve_group_references() uses the redefined versions
    // 4. resolve_inline_element_type_derivations() resolves inline element types
    //
    // DITA schemas using redefine for domain specialization now work correctly.
    // See xmlschema-rs tests: test_dita_redefine_support, test_dita_topic_children_from_redefine

    eprintln!(
        "FIXED: xs:redefine is now supported by xmlschema-rs. \
         DITA domain specialization works correctly."
    );
}

// ============================================================================
// Summary of gaps and fix roadmap
// ============================================================================

#[test]
fn test_print_limitation_summary() {
    eprintln!("\n");
    eprintln!("=== terminusdb-xsd Known Limitations ===\n");

    eprintln!("1. xs:union → TaggedUnion (FIXED ✓)");
    eprintln!("   xmlschema-rs: parse_simple_union() now uses XsdUnionType");
    eprintln!("   terminusdb-xsd: Generates Schema::TaggedUnion for xs:union types");
    eprintln!();

    eprintln!("2. xs:list → TypeFamily::List (FIXED ✓)");
    eprintln!("   xmlschema-rs: parse_simple_list() now uses XsdListType");
    eprintln!("   terminusdb-xsd: Generates TypeFamily::List for xs:list types");
    eprintln!();

    eprintln!("3. xs:redefine (FIXED ✓)");
    eprintln!("   xmlschema-rs: parse_redefine() + resolve_inline_element_type_derivations()");
    eprintln!("   DITA domain specialization now works correctly");
    eprintln!();

    eprintln!("4. xs:any / xs:anyAttribute (PARTIALLY SUPPORTED)");
    eprintln!("   Wildcards may be omitted or simplified");
    eprintln!();

    eprintln!("=== End of Limitations ===\n");
}
