use terminusdb_schema::*;
use terminusdb_schema_derive::{TerminusDBModel, FromTDBInstance};

#[test]
fn test_enum_variant_schema_class_naming() {
    // This test verifies that the fix for enum variant schema class IDs is working correctly
    #[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
    enum TestEnum {
        TextRangeSelector {
            start_offset: u32,
            end_offset: u32,
        },
        ObjectSelector {
            query: String,
        },
        SimpleVariant,
    }

    // Test that the schema generates the correct class names for variants
    let schema = <TestEnum as ToTDBSchema>::to_schema();
    if let Schema::TaggedUnion { properties, .. } = schema {
        assert_eq!(properties.len(), 3);
        
        // Check that class names use original casing (not renamed)
        // This is what the fix ensures - class names should be TestEnumTextRangeSelector
        // not TestEnum_textrangeselector
        let text_range_prop = properties
            .iter()
            .find(|p| p.name == "textrangeselector")
            .unwrap();
        assert_eq!(text_range_prop.class, "TestEnumTextRangeSelector");
        
        let object_prop = properties
            .iter()
            .find(|p| p.name == "objectselector")
            .unwrap();
        assert_eq!(object_prop.class, "TestEnumObjectSelector");
        
        let simple_prop = properties
            .iter()
            .find(|p| p.name == "simplevariant")
            .unwrap();
        assert_eq!(simple_prop.class, "sys:Unit");
    } else {
        panic!("Expected TaggedUnion schema");
    }
}

#[test]
fn test_enum_variant_instance_schema_class_id() {
    // This test verifies the fix: variant schema class IDs should use original casing
    #[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
    enum TestEnum {
        TextRangeSelector {
            start_offset: u32,
            end_offset: u32,
        },
    }

    let variant = TestEnum::TextRangeSelector {
        start_offset: 10,
        end_offset: 20,
    };
    
    let instance = variant.to_instance(None);
    
    // Find the variant property (it will be lowercase by default)
    if let Some(InstanceProperty::Relation(RelationValue::One(inner))) = 
        instance.properties.get("textrangeselector") {
        
        match &inner.schema {
            Schema::Class { id, .. } => {
                // The fix ensures this is TestEnumTextRangeSelector, not TestEnum_textrangeselector
                assert_eq!(id, "TestEnumTextRangeSelector");
            }
            _ => panic!("Expected Class schema for variant instance"),
        }
    } else {
        panic!("Expected Relation property for textrangeselector");
    }
}

