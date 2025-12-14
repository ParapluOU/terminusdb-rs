use terminusdb_woql_builder::prelude::*;
use terminusdb_woql_builder::builder::WoqlBuilder;
use terminusdb_schema::ToTDBInstance;

#[test]
fn test_concat_generates_correct_json_ld() {
    // Create variables
    let (pub_id_var, pub_iri_str_var) = vars!("PubId", "PubIRIStr");
    
    // Build a simple query that uses concat
    let query = WoqlBuilder::new()
        .concat(
            list(vec![
                string_literal("AwsDBPublication/"),
                pub_id_var.clone().into(),
            ]),
            pub_iri_str_var.clone(),
        )
        .finalize();
    
    // Convert to JSON-LD
    let json_ld = query.to_json();
    
    println!("Generated WOQL JSON-LD:");
    println!("{}", serde_json::to_string_pretty(&json_ld).unwrap());
    
    // Verify the structure
    let json_obj = json_ld.as_object().unwrap();
    assert_eq!(json_obj.get("@type").unwrap().as_str().unwrap(), "Concatenate");
    
    // The list should be a direct array, not wrapped in a DataValue
    let list_value = json_obj.get("list").unwrap();
    assert!(list_value.is_array(), "list should be a direct array, not wrapped in DataValue");
    
    let list_array = list_value.as_array().unwrap();
    assert_eq!(list_array.len(), 2);
    
    // Check the first element - it should be {"@type": "DataValue", "data": "AwsDBPublication/"}
    let first_elem = list_array[0].as_object().unwrap();
    assert_eq!(first_elem.get("@type").unwrap().as_str().unwrap(), "DataValue");
    assert_eq!(first_elem.get("data").unwrap().as_str().unwrap(), "AwsDBPublication/");
    
    // Check the second element - it should be {"@type": "DataValue", "variable": "PubId"}
    let second_elem = list_array[1].as_object().unwrap();
    assert_eq!(second_elem.get("@type").unwrap().as_str().unwrap(), "DataValue");
    assert_eq!(second_elem.get("variable").unwrap().as_str().unwrap(), "PubId");
    
    println!("Test passed! The list is correctly serialized as a direct array.");
}