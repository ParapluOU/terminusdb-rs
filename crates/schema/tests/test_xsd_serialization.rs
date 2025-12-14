use terminusdb_schema::XSDAnySimpleType;
use decimal_rs::Decimal;

#[test]
fn test_xsd_any_simple_type_serialization_fixed() {
    // Test that the serialization has been fixed for TerminusDB compatibility
    use terminusdb_schema::{ToInstanceProperty, InstanceProperty, PrimitiveValue, Schema};
    
    // Test Decimal (used for i32, i64)
    let decimal_value = XSDAnySimpleType::Decimal(Decimal::from(50));
    let schema = Schema::empty_class("TestClass");
    let decimal_prop: InstanceProperty = <XSDAnySimpleType as ToInstanceProperty<()>>::to_property(decimal_value, "test", &schema);
    
    match decimal_prop {
        InstanceProperty::Primitive(PrimitiveValue::Object(json)) => {
            println!("Decimal serialization: {}", serde_json::to_string_pretty(&json).unwrap());
            assert_eq!(json["@type"], "xsd:decimal");
            assert_eq!(json["@value"], "50");
        }
        _ => panic!("Expected Primitive Object for Decimal")
    }
    
    // Test UnsignedInt (used for u32, usize)
    let uint_value = XSDAnySimpleType::UnsignedInt(50);
    let uint_prop: InstanceProperty = <XSDAnySimpleType as ToInstanceProperty<()>>::to_property(uint_value, "test", &schema);
    
    match uint_prop {
        InstanceProperty::Primitive(PrimitiveValue::Number(n)) => {
            println!("UnsignedInt serialization: {}", n);
            assert_eq!(n.as_u64(), Some(50));
        }
        _ => panic!("Expected Primitive Number for UnsignedInt")
    }
    
    println!("\n✅ XSDAnySimpleType serialization has been fixed!");
    println!("- Decimal now produces: {{\"@type\": \"xsd:decimal\", \"@value\": \"50\"}}");
    println!("- UnsignedInt produces: 50 (as JSON number)");
}

#[test]
fn test_all_xsd_types_serialization() {
    // Test various XSD types to show the pattern
    let test_cases = vec![
        (XSDAnySimpleType::String("hello".to_string()), "String", "hello"),
        (XSDAnySimpleType::Decimal(Decimal::from(42)), "Decimal", "42"),
        (XSDAnySimpleType::Float(3.14), "Float", "3.14"),
        (XSDAnySimpleType::Boolean(true), "Boolean", "true"),
    ];
    
    for (value, expected_key, _expected_value) in test_cases {
        let json = serde_json::to_value(&value).unwrap();
        let obj = json.as_object().unwrap();
        
        // Current behavior: enum variant as key
        assert!(obj.contains_key(expected_key), 
            "Expected key '{}' not found in {:?}", expected_key, obj);
        
        // What TerminusDB needs: {"@type": "xsd:TYPE", "@value": VALUE}
        // This would require custom serialization implementation
    }
}

// Note: The WOQL context test would show how this issue affects DataValue serialization,
// but that requires the terminusdb-woql2 crate dependency which isn't available here.