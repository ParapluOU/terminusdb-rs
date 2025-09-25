use terminusdb_woql2::prelude::*;

// Test structures
struct User;
struct Post;
struct Comment;
struct Like;

#[test]
fn test_simple_forward() {
    // This should work
    let q1 = from_path!(User => Post);
    println!("User => Post: {:#?}", q1);
    
    // Test with field
    let q2 = from_path!(User.posts => Post);
    println!("User.posts => Post: {:#?}", q2);
}

#[test]
fn test_longer_chain() {
    // Test 3-node chain
    let q3 = from_path!(User => Post => Comment);
    println!("User => Post => Comment: {:#?}", q3);
    
    // Test 4-node chain
    let q4 = from_path!(User => Post => Comment => Like);
    println!("User => Post => Comment => Like: {:#?}", q4);
}

#[test]
fn test_mixed_direction() {
    // Test mixed
    let q5 = from_path!(User => Post <= Comment);
    println!("User => Post <= Comment: {:#?}", q5);
}

#[test]
fn test_custom_vars() {
    // Test custom variables
    let q6 = from_path!(u:User => p:Post);
    println!("u:User => p:Post: {:#?}", q6);
    
    // Longer with custom vars
    let q7 = from_path!(u:User => p:Post => c:Comment);
    println!("u:User => p:Post => c:Comment: {:#?}", q7);
}

#[test]
fn test_unlimited_chain() {
    // Test very long chain (5+ nodes)
    struct A;
    struct B;
    struct C;
    struct D;
    struct E;
    struct F;
    struct G;
    
    let q8 = from_path!(A => B => C => D => E);
    println!("5-node chain: {:#?}", q8);
    
    let q9 = from_path!(A => B => C => D => E => F);
    println!("6-node chain: {:#?}", q9);
    
    let q10 = from_path!(A => B => C => D => E => F => G);
    println!("7-node chain: {:#?}", q10);
}