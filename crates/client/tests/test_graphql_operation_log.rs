//! Unit test to verify GraphQL queries are tracked in the operation log

use terminusdb_client::debug::OperationType;

#[test]
fn test_graphql_operation_type_display() {
    // Test that the GraphQL operation type displays correctly
    let op_type = OperationType::GraphQL;
    assert_eq!(op_type.to_string(), "graphql");
}

#[test]
fn test_graphql_operation_type_serialization() {
    // Test that GraphQL operation type serializes correctly
    let op_type = OperationType::GraphQL;
    let json = serde_json::to_value(&op_type).unwrap();
    assert_eq!(json, serde_json::json!("graphql"));
}

#[test]
fn test_graphql_operation_type_deserialization() {
    // Test that GraphQL operation type deserializes correctly
    let json = serde_json::json!("graphql");
    let op_type: OperationType = serde_json::from_value(json).unwrap();
    assert_eq!(op_type, OperationType::GraphQL);
}
