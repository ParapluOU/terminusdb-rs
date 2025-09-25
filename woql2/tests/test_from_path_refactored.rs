use terminusdb_woql2::prelude::*;
use terminusdb_woql2::{parse_node_pattern, parse_direction, from_path_refactored, process_node};

struct User;
struct Post; 
struct Comment;
struct Like;

#[test]
fn test_refactored_single_nodes() {
    // Test all single node patterns
    let q1 = from_path_refactored!(User);
    println!("User: {}", q1.to_dsl());
    assert!(q1.to_dsl().contains("User"));
    
    let q2 = from_path_refactored!(u:User);
    println!("u:User: {}", q2.to_dsl());
    assert!(q2.to_dsl().contains("u"));
    
    let q3 = from_path_refactored!(User.posts);
    println!("User.posts: {}", q3.to_dsl());
    assert!(q3.to_dsl().contains("User"));
    
    let q4 = from_path_refactored!(u:User.posts);
    println!("u:User.posts: {}", q4.to_dsl());
    assert!(q4.to_dsl().contains("u"));
}

#[test]
fn test_refactored_simple_chains() {
    // Test two-node chains
    let q1 = from_path_refactored!(User > Post);
    println!("User > Post: {}", q1.to_dsl());
    assert!(q1.to_dsl().contains("User"));
    assert!(q1.to_dsl().contains("Post"));
    
    let q2 = from_path_refactored!(User < Post);
    println!("User < Post: {}", q2.to_dsl());
    
    let q3 = from_path_refactored!(u:User > p:Post);
    println!("u:User > p:Post: {}", q3.to_dsl());
    assert!(q3.to_dsl().contains("u"));
    assert!(q3.to_dsl().contains("p"));
    
    let q4 = from_path_refactored!(User.posts > Post);
    println!("User.posts > Post: {}", q4.to_dsl());
}

#[test]
fn test_refactored_complex_chains() {
    // Test longer chains
    let q1 = from_path_refactored!(User > Post > Comment);
    println!("User > Post > Comment: {}", q1.to_dsl());
    assert!(q1.to_dsl().contains("User"));
    assert!(q1.to_dsl().contains("Post"));
    assert!(q1.to_dsl().contains("Comment"));
    
    let q2 = from_path_refactored!(User > Post < Comment);
    println!("User > Post < Comment: {}", q2.to_dsl());
    
    let q3 = from_path_refactored!(u:User > p:Post < c:Comment);
    println!("u:User > p:Post < c:Comment: {}", q3.to_dsl());
    
    let q4 = from_path_refactored!(User > Post > Comment > Like);
    println!("4-node chain: {}", q4.to_dsl());
}

#[test]
fn test_refactored_with_fields() {
    // Test chains with fields
    let q1 = from_path_refactored!(User.posts > Post.comments > Comment);
    println!("Fields chain: {}", q1.to_dsl());
    
    let q2 = from_path_refactored!(u:User.posts > Post < c:Comment);
    println!("Mixed fields: {}", q2.to_dsl());
}

#[test]
fn test_refactored_vs_original() {
    // Compare refactored output with original
    let ref_q = from_path_refactored!(User > Post > Comment);
    let orig_q = from_path!(User > Post > Comment);
    
    println!("Refactored: {}", ref_q.to_dsl());
    println!("Original: {}", orig_q.to_dsl());
    
    // They should produce similar WOQL structure
    assert_eq!(ref_q.to_dsl().contains("User"), orig_q.to_dsl().contains("User"));
    assert_eq!(ref_q.to_dsl().contains("Post"), orig_q.to_dsl().contains("Post"));
    assert_eq!(ref_q.to_dsl().contains("Comment"), orig_q.to_dsl().contains("Comment"));
}