use terminusdb_woql_builder::prelude::*;
use terminusdb_woql_builder::builder::WoqlBuilder;
use terminusdb_schema::ToTDBInstance;

#[test]
fn test_join_generates_correct_json_ld() {
    // Create variables
    let (parts_var, result_var) = vars!("Parts", "Result");
    
    // Build a simple query that uses join
    let query = WoqlBuilder::new()
        .join(
            list(vec![
                string_literal("Hello"),
                string_literal("World"),
            ]),
            string_literal(", "),
            result_var.clone(),
        )
        .finalize();
    
    // Convert to JSON-LD
    let json_ld = query.to_json();
    
    println!("Generated Join WOQL JSON-LD:");
    println!("{}", serde_json::to_string_pretty(&json_ld).unwrap());
    
    // Verify the structure
    let json_obj = json_ld.as_object().unwrap();
    assert_eq!(json_obj.get("@type").unwrap().as_str().unwrap(), "Join");
    
    // The list should be a direct array, not wrapped in a DataValue
    let list_value = json_obj.get("list").unwrap();
    assert!(list_value.is_array(), "list should be a direct array, not wrapped in DataValue");
    
    let list_array = list_value.as_array().unwrap();
    assert_eq!(list_array.len(), 2);
    
    // Check the separator
    let separator = json_obj.get("separator").unwrap();
    let separator_obj = separator.as_object().unwrap();
    assert_eq!(separator_obj.get("@type").unwrap().as_str().unwrap(), "DataValue");
    assert_eq!(separator_obj.get("data").unwrap().as_str().unwrap(), ", ");
    
    println!("Test passed! The join list is correctly serialized as a direct array.");
}

#[test]
fn test_join_with_variable_list() {
    // Create variables
    let (parts_var, result_var) = vars!("Parts", "Result");
    
    // Build a query where the list is a variable
    let query = WoqlBuilder::new()
        .join(
            parts_var.clone(),
            string_literal(" - "),
            result_var.clone(),
        )
        .finalize();
    
    // Convert to JSON-LD
    let json_ld = query.to_json();
    
    println!("Generated Join with Variable WOQL JSON-LD:");
    println!("{}", serde_json::to_string_pretty(&json_ld).unwrap());
    
    // Verify the structure
    let json_obj = json_ld.as_object().unwrap();
    assert_eq!(json_obj.get("@type").unwrap().as_str().unwrap(), "Join");
    
    // The list should be a variable object
    let list_value = json_obj.get("list").unwrap();
    let list_obj = list_value.as_object().unwrap();
    assert_eq!(list_obj.get("Variable").unwrap().as_str().unwrap(), "Parts");
    
    println!("Test passed! The join with variable list is correctly serialized.");
}