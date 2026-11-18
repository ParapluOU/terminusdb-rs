use serde_json;
use terminusdb_schema::XSDAnySimpleType;
use terminusdb_woql2::string::Concatenate;
use terminusdb_woql2::value::{DataValue, ListOrVariable};

#[test]
fn test_datavalue_list_serialization() {
    // A DataValue::List is a tagged object carrying a `list` array — the enum
    // keeps its `@type` discriminator rather than collapsing to a bare array.
    //
    // NB: this pins the *serde* shape only. The JSON-LD actually sent to the
    // server comes from `ToJson`/`to_woql_json`, not from serde.
    let list = DataValue::List(vec![
        DataValue::Data(XSDAnySimpleType::String("Hello".to_string())),
        DataValue::Variable("World".to_string()),
    ]);

    let json = serde_json::to_value(&list).unwrap();

    assert!(json.is_object());
    assert_eq!(json["@type"], "DataValue");

    let items = json["list"].as_array().expect("list should be an array");
    assert_eq!(items.len(), 2);

    // A data element carries its literal directly under `data`.
    assert_eq!(items[0]["@type"], "DataValue");
    assert_eq!(items[0]["data"], "Hello");

    // A variable element carries its name under `variable`.
    assert_eq!(items[1]["@type"], "DataValue");
    assert_eq!(items[1]["variable"], "World");
}

#[test]
fn test_concatenate_with_list() {
    // Create a Concatenate query with a list
    let concat = Concatenate {
        list: ListOrVariable::List(vec![
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
    // `result_string` is `#[tdb(name = "result")]`, so it is `result` on the wire.
    assert_eq!(json["result"]["@type"], "DataValue");
    assert_eq!(json["result"]["variable"], "PubIRIStr");
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
    // The string literal sits directly under `data`, not wrapped in a
    // variant-named object.
    assert_eq!(json["data"], "TestData");
}
