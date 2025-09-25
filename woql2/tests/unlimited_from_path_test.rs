use terminusdb_woql2::prelude::*;

// Test structures
struct User;
struct Post;
struct Comment;
struct Like;
struct Tag;
struct Category;
struct Forum;
struct Thread;

#[test]
fn test_simple_forward_new_syntax() {
    // Test: User > Post
    let query = from_path!(User > Post);
    println!("User > Post: {:#?}", query);
    
    // Verify structure
    match query {
        Query::And(_) => println!("✅ Two types generate And constraint"),
        _ => panic!("Expected And query"),
    }
}

#[test]
fn test_simple_backward_new_syntax() {
    // Test: Comment < Post (Comment belongs to Post)
    let query = from_path!(Comment < Post);
    println!("Comment < Post: {:#?}", query);
    
    match query {
        Query::And(_) => println!("✅ Backward relation works"),
        _ => panic!("Expected And query"),
    }
}

#[test]
fn test_three_node_chain() {
    // Test: User > Post > Comment
    let query = from_path!(User > Post > Comment);
    println!("User > Post > Comment: {:#?}", query);
    
    // Verify DSL output
    let dsl = query.to_dsl();
    println!("DSL: {}", dsl);
    assert!(dsl.contains("User"));
    assert!(dsl.contains("Post"));
    assert!(dsl.contains("Comment"));
}

#[test]
fn test_four_node_chain() {
    // Test: User > Post > Comment > Like
    let query = from_path!(User > Post > Comment > Like);
    println!("4-node chain: {:#?}", query);
}

#[test]
fn test_five_node_chain() {
    // Test: User > Post > Comment > Like > Tag
    let query = from_path!(User > Post > Comment > Like > Tag);
    println!("5-node chain: {:#?}", query);
}

#[test]
fn test_seven_node_chain() {
    // Test: Forum > Category > Thread > Post > Comment > Like > User
    let query = from_path!(Forum > Category > Thread > Post > Comment > Like > User);
    println!("7-node chain: {:#?}", query);
}

#[test]
fn test_mixed_directions() {
    // Test: User > Post < Comment > Like
    let query = from_path!(User > Post < Comment > Like);
    println!("Mixed directions: {:#?}", query);
    
    // Test: Comment < Post > User
    let query2 = from_path!(Comment < Post > User);
    println!("Mixed directions 2: {:#?}", query2);
}

#[test]
fn test_custom_variables() {
    // Test: u:User > p:Post
    let query = from_path!(u:User > p:Post);
    println!("Custom variables: {:#?}", query);
    
    // Long chain with custom vars
    let query2 = from_path!(u:User > p:Post > c:Comment > l:Like);
    println!("Long chain with vars: {:#?}", query2);
}

#[test]
fn test_field_access() {
    // Test: User.posts > Post
    let query = from_path!(User.posts > Post);
    println!("Field access: {:#?}", query);
    
    // Chain with fields
    let query2 = from_path!(User.posts > Post.comments > Comment);
    println!("Chain with fields: {:#?}", query2);
}

#[test]
fn test_complex_unlimited_chain() {
    // Very long mixed chain
    struct A;
    struct B;
    struct C;
    struct D;
    struct E;
    struct F;
    struct G;
    struct H;
    struct I;
    struct J;
    
    // 10-node chain with mixed patterns
    // Note: fields can only appear at the start of a relation or after a type name
    // The pattern "Type > Type.field > Type" is not supported - use "Type > Type" and "Type.field > Type" separately
    let query = from_path!(
        a:A > B > c:C < D > E < f:F < G > h:H < j:J
    );
    println!("10-node complex chain: {:#?}", query);
    
    let dsl = query.to_dsl();
    println!("Complex DSL: {}", dsl);
    
    // Verify all types are present
    assert!(dsl.contains("@schema:A"));
    assert!(dsl.contains("@schema:B"));
    assert!(dsl.contains("@schema:C"));
    assert!(dsl.contains("@schema:D"));
    assert!(dsl.contains("@schema:E"));
    assert!(dsl.contains("@schema:F"));
    assert!(dsl.contains("@schema:G"));
    assert!(dsl.contains("@schema:H"));
    assert!(dsl.contains("@schema:I"));
    assert!(dsl.contains("@schema:J"));
}