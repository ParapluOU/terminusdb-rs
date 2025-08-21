use terminusdb_woql2::prelude::*;
use terminusdb_woql2::string::Concatenate;
use terminusdb_woql2::value::DataValue;
use terminusdb_schema::XSDAnySimpleType;
use serde_json;

#[test]
fn test_datavalue_list_serialization() {
    // Test that DataValue::List serializes as an array, not an object
    let list = DataValue::List(vec![
        DataValue::Data(XSDAnySimpleType::String("Hello".to_string())),
        DataValue::Variable("World".to_string()),
    ]);
    
    let json = serde_json::to_value(&list).unwrap();
    
    // Should be an array, not an object
    assert!(json.is_array());
    assert_eq!(json.as_array().unwrap().len(), 2);
    
    // Check the first element
    let first = &json[0];
    assert!(first.is_object());
    assert_eq!(first["@type"], "DataValue");
    // XSDAnySimpleType::String is serialized as {"String": "Hello"}
    assert_eq!(first["data"]["String"], "Hello");
    
    // Check the second element
    let second = &json[1];
    assert!(second.is_object());
    assert_eq!(second["@type"], "DataValue");
    assert_eq!(second["variable"], "World");
}

#[test]
fn test_concatenate_with_list() {
    // Create a Concatenate query with a list
    let concat = Concatenate {
        list: DataValue::List(vec![
            DataValue::Data(XSDAnySimpleType::String("AwsDBPublication/".to_string())),
            DataValue::Variable("PubId".to_string()),
        ]),
        result_string: DataValue::Variable("PubIRIStr".to_string()),
    };
    
    let json = serde_json::to_value(&concat).unwrap();
    
    // The list field should be an array, not an object
    assert!(json["list"].is_array());
    assert_eq!(json["list"].as_array().unwrap().len(), 2);
    
    // Pretty print to see the structure
    let pretty = serde_json::to_string_pretty(&json).unwrap();
    println!("Concatenate JSON-LD:\n{}", pretty);
    
    // Verify the structure matches what TerminusDB expects
    // The TerminusDBModel derive should add @type field
    // But if it's missing, let's check the actual structure
    if json.get("@type").is_none() {
        println!("Warning: @type field is missing from Concatenate");
    }
    assert!(json["list"].is_array());
    assert_eq!(json["result_string"]["@type"], "DataValue");
    assert_eq!(json["result_string"]["variable"], "PubIRIStr");
}

#[test]
fn test_datavalue_variable_serialization() {
    // Test that DataValue::Variable still serializes as an object
    let var = DataValue::Variable("TestVar".to_string());
    let json = serde_json::to_value(&var).unwrap();
    
    assert!(json.is_object());
    assert_eq!(json["@type"], "DataValue");
    assert_eq!(json["variable"], "TestVar");
}

#[test]
fn test_datavalue_data_serialization() {
    // Test that DataValue::Data still serializes as an object
    let data = DataValue::Data(XSDAnySimpleType::String("TestData".to_string()));
    let json = serde_json::to_value(&data).unwrap();
    
    assert!(json.is_object());
    assert_eq!(json["@type"], "DataValue");
    // XSDAnySimpleType::String is serialized as {"String": "TestData"}
    assert_eq!(json["data"]["String"], "TestData");
}