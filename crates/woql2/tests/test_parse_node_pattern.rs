use terminusdb_woql2::prelude::*;
use terminusdb_woql2::parse_node_pattern;

struct User;
struct Post;

#[test]
fn test_parse_node_pattern_simple() {
    // Test: M
    let (builder, field): (_, Option<&str>) = parse_node_pattern!(User);
    assert!(field.is_none());
    let query = builder.finalize();
    let dsl = query.to_dsl();
    assert!(dsl.contains("User"));
    println!("Simple node pattern works: {}", dsl);
}

#[test]
fn test_parse_node_pattern_with_var() {
    // Test: m:M
    let (builder, field): (_, Option<&str>) = parse_node_pattern!(u:User);
    assert!(field.is_none());
    let query = builder.finalize();
    let dsl = query.to_dsl();
    assert!(dsl.contains("u"));
    assert!(dsl.contains("User"));
    println!("Variable node pattern works: {}", dsl);
}

#[test]
fn test_parse_node_pattern_with_field() {
    // Test: M.field
    let (builder, field): (_, Option<&str>) = parse_node_pattern!(User.posts);
    assert_eq!(field, Some("posts"));
    let query = builder.finalize();
    let dsl = query.to_dsl();
    assert!(dsl.contains("User"));
    println!("Field node pattern works: {}", dsl);
}

#[test]
fn test_parse_node_pattern_with_var_and_field() {
    // Test: m:M.field
    let (builder, field): (_, Option<&str>) = parse_node_pattern!(u:User.posts);
    assert_eq!(field, Some("posts"));
    let query = builder.finalize();
    let dsl = query.to_dsl();
    assert!(dsl.contains("u"));
    assert!(dsl.contains("User"));
    println!("Variable and field node pattern works: {}", dsl);
}

#[test]
fn test_parse_node_pattern_in_context() {
    // Test using the pattern in a path-like context
    let (builder1, field1): (_, Option<&str>) = parse_node_pattern!(u:User.posts);
    let (builder2, field2): (_, Option<&str>) = parse_node_pattern!(Post);
    
    // This demonstrates how the macro could be used to build paths
    println!("Node 1 field: {:?}", field1);
    println!("Node 2 field: {:?}", field2);
    
    // Both builders work independently
    let q1 = builder1.finalize();
    let q2 = builder2.finalize();
    
    println!("Query 1: {}", q1.to_dsl());
    println!("Query 2: {}", q2.to_dsl());
}