use terminusdb_woql2::prelude::*;
use terminusdb_woql2::{parse_node_pattern, parse_direction, from_path_simplified};
use terminusdb_woql2::path_builder::PathDirection;

struct User;
struct Post;
struct Comment;

#[test]
fn test_simplified_single_nodes() {
    // Test single node patterns
    let q1 = from_path_simplified!(User);
    println!("User: {}", q1.to_dsl());
    
    let q2 = from_path_simplified!(u:User);
    println!("u:User: {}", q2.to_dsl());
    
    let q3 = from_path_simplified!(User.posts);
    println!("User.posts: {}", q3.to_dsl());
    
    let q4 = from_path_simplified!(u:User.posts);
    println!("u:User.posts: {}", q4.to_dsl());
}

#[test]
fn test_simplified_two_nodes() {
    // Test two-node patterns
    let q1 = from_path_simplified!(User > Post);
    println!("User > Post: {}", q1.to_dsl());
    assert!(q1.to_dsl().contains("User"));
    assert!(q1.to_dsl().contains("Post"));
    
    let q2 = from_path_simplified!(User < Post);
    println!("User < Post: {}", q2.to_dsl());
    
    let q3 = from_path_simplified!(u:User > p:Post);
    println!("u:User > p:Post: {}", q3.to_dsl());
    
    let q4 = from_path_simplified!(User.posts > Post);
    println!("User.posts > Post: {}", q4.to_dsl());
}

#[test]
fn test_simplified_three_nodes() {
    // Test three-node chains
    let q1 = from_path_simplified!(User > Post > Comment);
    println!("User > Post > Comment: {}", q1.to_dsl());
    assert!(q1.to_dsl().contains("User"));
    assert!(q1.to_dsl().contains("Post"));
    assert!(q1.to_dsl().contains("Comment"));
    
    let q2 = from_path_simplified!(User > Post < Comment);
    println!("User > Post < Comment: {}", q2.to_dsl());
    
    let q3 = from_path_simplified!(User < Post > Comment);
    println!("User < Post > Comment: {}", q3.to_dsl());
}

#[test]
fn test_helper_macros_directly() {
    // Test the helper macros work correctly
    let dir_forward = parse_direction!(>);
    assert_eq!(dir_forward, PathDirection::Forward);
    
    let dir_backward = parse_direction!(<);
    assert_eq!(dir_backward, PathDirection::Backward);
    
    let (builder, field): (_, Option<&str>) = parse_node_pattern!(User);
    assert!(field.is_none());
    
    let (builder, field): (_, Option<&str>) = parse_node_pattern!(User.posts);
    assert_eq!(field, Some("posts"));
}

#[test]
fn test_comparison_with_original() {
    // Compare simplified version with original
    let simplified = from_path_simplified!(User > Post > Comment);
    let original = from_path!(User > Post > Comment);
    
    println!("Simplified: {}", simplified.to_dsl());
    println!("Original: {}", original.to_dsl());
    
    // Both should contain the same types
    assert!(simplified.to_dsl().contains("User"));
    assert!(simplified.to_dsl().contains("Post"));
    assert!(simplified.to_dsl().contains("Comment"));
    assert!(original.to_dsl().contains("User"));
    assert!(original.to_dsl().contains("Post")); 
    assert!(original.to_dsl().contains("Comment"));
}