use anyhow::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use terminusdb_schema::{
    json::InstanceFromJson, Instance, InstanceProperty, ToTDBInstance, ToTDBInstances, ToTDBSchema,
};
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

#[derive(Debug, Clone, TerminusDBModel)]
#[tdb(class_name = "Person")]
struct Person {
    name: String,
    age: u32,
    is_active: bool,
    tags: Vec<String>,
}

#[derive(Debug, Clone, TerminusDBModel)]
#[tdb(class_name = "SimpleAddress")]
struct SimpleAddress {
    street: String,
    city: String,
}

// Simple enum for testing
#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance, Serialize, Deserialize)]
enum Color {
    Red,
    Green,
    Blue,
    Yellow,
}

// Simple struct for testing
#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance, Serialize, Deserialize)]
struct SimpleStruct {
    name: String,
    color: Color,
}

// Complex struct for tagged union value
#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance, Serialize, Deserialize)]
struct TaggedValueComplex {
    x: f64,
    y: f64,
}

// Tagged union enum for testing
#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance, Serialize, Deserialize)]
enum TaggedValue {
    Simple(String),
    Number(i32),
    Point { x: f64, y: f64 },
    Complex(TaggedValueComplex),
}

#[test]
fn test_basic_deserialization() {
    // Create a simple JSON object
    let json = json!({
        "@id": "Person/123",
        "@type": "Person",
        "name": "John Doe",
        "age": 30,
        "is_active": true,
        "tags": ["developer", "rust"]
    });

    // Deserialize the JSON into an Instance
    let instance = Person::instance_from_json(json).expect("Failed to deserialize JSON");

    // Verify the instance properties
    assert_eq!(instance.id, Some("Person/123".to_string()));
    assert!(instance.properties.contains_key("name"));
    assert!(instance.properties.contains_key("age"));
    assert!(instance.properties.contains_key("is_active"));
    assert!(instance.properties.contains_key("tags"));
}

#[test]
fn test_simple_enum_deserialization() {
    // Test Red variant
    let json_red = json!({
        "@id": "Color/1",
        "@type": "Color",
        "Red": null
    });

    let instance_red =
        Color::instance_from_json(json_red).expect("Failed to deserialize Red enum variant");

    // Verify the instance properly captures the Red variant
    assert_eq!(instance_red.id, Some("Color/1".to_string()));
    assert!(instance_red.properties.contains_key("Red"));
    assert!(!instance_red.properties.contains_key("Green"));
    assert!(!instance_red.properties.contains_key("Blue"));

    // Test Green variant
    let json_green = json!({
        "@id": "Color/2",
        "@type": "Color",
        "Green": null
    });

    let instance_green =
        Color::instance_from_json(json_green).expect("Failed to deserialize Green enum variant");

    // Verify the instance properly captures the Green variant
    assert_eq!(instance_green.id, Some("Color/2".to_string()));
    assert!(!instance_green.properties.contains_key("Red"));
    assert!(instance_green.properties.contains_key("Green"));
    assert!(!instance_green.properties.contains_key("Blue"));
}

#[test]
fn test_tagged_union_enum_deserialization() {
    // Test Simple variant
    let json_simple = json!({
        "@id": "TaggedValue/1",
        "@type": "TaggedValue",
        "simple": "Hello, world!"
    });

    let instance_simple =
        TaggedValue::instance_from_json(json_simple).expect("Failed to deserialize Simple variant");

    // Verify the instance properly captures the Simple variant
    assert_eq!(instance_simple.id, Some("TaggedValue/1".to_string()));
    assert!(instance_simple.properties.contains_key("simple"));

    // Test Number variant
    let json_number = json!({
        "@id": "TaggedValue/2",
        "@type": "TaggedValue",
        "number": 42
    });

    let instance_number =
        TaggedValue::instance_from_json(json_number).expect("Failed to deserialize Number variant");

    // Verify the instance properly captures the Number variant
    assert_eq!(instance_number.id, Some("TaggedValue/2".to_string()));
    assert!(instance_number.properties.contains_key("number"));

    // Test Point variant (struct with named fields)
    let json_point = json!({
        "@id": "TaggedValue/4",
        "@type": "TaggedValue",
        "point": {
            "x": 3.14,
            "y": 2.71
        }
    });

    let instance_point =
        TaggedValue::instance_from_json(json_point).expect("Failed to deserialize Point variant");

    // Verify the instance properly captures the Point variant
    assert_eq!(instance_point.id, Some("TaggedValue/4".to_string()));
    assert!(instance_point.properties.contains_key("point"));

    // Test Complex variant (named fields)
    let json_complex = json!({
        "@id": "TaggedValue/5",
        "@type": "TaggedValue",
        "complex": {
            "@type": "TaggedValueComplex",
            "x": 10.5,
            "y": 20.3
        }
    });

    let instance_complex = TaggedValue::instance_from_json(json_complex)
        .expect("Failed to deserialize Complex variant");

    // Verify the instance properly captures the Complex variant
    assert_eq!(instance_complex.id, Some("TaggedValue/5".to_string()));
    assert!(instance_complex.properties.contains_key("complex"));
}

#[test]
fn test_enum_deserialization_errors() {
    // Test missing @id field
    let json_missing_id = json!({
        "@type": "Color",
        "Red": null
    });

    let result = Color::instance_from_json(json_missing_id);
    // assert!(result.is_err(), "Expected error for missing @id field");

    // Test missing @type field
    let json_missing_type = json!({
        "@id": "Color/1"
    });

    let result = Color::instance_from_json(json_missing_type);
    assert!(result.is_err(), "Expected error for missing @type field");

    // Test wrong @type field
    let json_wrong_type = json!({
        "@id": "Color/1",
        "@type": "WrongType",
        "Red": null
    });

    let result = Color::instance_from_json(json_wrong_type);
    assert!(result.is_err(), "Expected error for wrong @type field");

    // Test missing variant
    let json_missing_variant = json!({
        "@id": "Color/1",
        "@type": "Color"
    });

    let result = Color::instance_from_json(json_missing_variant);
    assert!(result.is_err(), "Expected error for missing variant");
}

#[test]
fn test_error_cases() {
    // Test missing required field
    let json_missing_name = json!({
        "@id": "Person/123",
        "@type": "Person",
        "age": 30,
        "is_active": true,
        "tags": ["developer", "rust"]
    });

    let result = Person::instance_from_json(json_missing_name);
    assert!(result.is_err(), "Expected error for missing required field");

    // Test incorrect type
    let json_wrong_type = json!({
        "@id": "Person/123",
        "@type": "NotAPerson",  // Wrong type
        "name": "John Doe",
        "age": 30,
        "is_active": true,
        "tags": ["developer", "rust"]
    });

    let result = Person::instance_from_json(json_wrong_type);
    assert!(result.is_err(), "Expected error for incorrect type");

    // Test incorrect field type
    let json_wrong_field_type = json!({
        "@id": "Person/123",
        "@type": "Person",
        "name": "John Doe",
        "age": "thirty",  // String instead of number
        "is_active": true,
        "tags": ["developer", "rust"]
    });

    let result = Person::instance_from_json(json_wrong_field_type);
    assert!(result.is_err(), "Expected error for incorrect field type");
}
