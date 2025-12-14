use terminusdb_woql2::prelude::*;

struct User;
struct Post;
struct Comment;
struct Like;
struct Tag;

#[test]
fn test_fields_at_start() {
    // Fields at the start work
    let query = from_path!(User.posts > Post);
    println!("Start field: {}", query.to_dsl());
    assert!(query.to_dsl().contains("posts"));
}

#[test]
fn test_fields_in_middle() {
    // Test: User > Post.comments > Comment
    let query = from_path!(User > Post.comments > Comment);
    println!("Middle field: {}", query.to_dsl());
    
    let dsl = query.to_dsl();
    assert!(dsl.contains("User"));
    assert!(dsl.contains("Post"));
    assert!(dsl.contains("Comment"));
    assert!(dsl.contains("comments")); // Should contain the field name
}

#[test]
fn test_multiple_fields_in_chain() {
    // Multiple fields throughout the chain
    let query = from_path!(User.posts > Post.comments > Comment.likes > Like);
    println!("Multiple fields: {}", query.to_dsl());
    
    let dsl = query.to_dsl();
    assert!(dsl.contains("posts"));
    assert!(dsl.contains("comments"));
    assert!(dsl.contains("likes"));
}

#[test]
fn test_fields_with_variables() {
    // Fields with custom variables
    let query = from_path!(u:User > p:Post.comments > c:Comment.likes > Like);
    println!("Fields with variables: {}", query.to_dsl());
    
    let dsl = query.to_dsl();
    assert!(dsl.contains("u"));
    assert!(dsl.contains("p"));
    assert!(dsl.contains("c"));
    assert!(dsl.contains("comments"));
    assert!(dsl.contains("likes"));
}

#[test]
fn test_fields_with_backward_relations() {
    // Fields with backward relations
    let query = from_path!(Comment < Post.author > User.profile > Profile);
    println!("Fields with backward: {}", query.to_dsl());
    
    struct Profile;
    let dsl = query.to_dsl();
    assert!(dsl.contains("author"));
    assert!(dsl.contains("profile"));
}

#[test]
fn test_complex_field_patterns() {
    // Mix of everything
    let query = from_path!(
        u:User.posts > Post < c:Comment.author > User > Like.tags > t:Tag
    );
    println!("Complex pattern: {}", query.to_dsl());
    
    let dsl = query.to_dsl();
    assert!(dsl.contains("posts"));
    assert!(dsl.contains("author"));
    assert!(dsl.contains("tags"));
    assert!(dsl.contains("u"));
    assert!(dsl.contains("c"));
    assert!(dsl.contains("t"));
}

#[test]
fn test_field_after_backward() {
    // Field after backward relation: A < B.field > C
    let query = from_path!(Comment < Post.tags > Tag);
    println!("Field after backward: {}", query.to_dsl());
    
    let dsl = query.to_dsl();
    assert!(dsl.contains("tags"));
}

#[test]
fn test_long_chain_with_fields() {
    // Long chain with fields throughout
    struct Forum;
    struct Category;
    struct Thread;
    struct Author;
    
    let query = from_path!(
        Forum.categories > Category.threads > Thread.posts > Post.comments > 
        Comment.author > Author.likes > Like.tags > Tag
    );
    
    println!("Long chain with fields: {}", query.to_dsl());
    
    let dsl = query.to_dsl();
    let field_count = ["categories", "threads", "posts", "comments", "author", "likes", "tags"]
        .iter()
        .filter(|f| dsl.contains(**f))
        .count();
    
    println!("Found {} fields out of 7", field_count);
    assert_eq!(field_count, 7);
}

#[test]
fn test_original_limitation_resolved() {
    // This was the original limitation - now it should work!
    struct Category;
    
    // All of these should now work:
    let q1 = from_path!(User > Post.comments > Comment);
    let q2 = from_path!(User.posts > Post.comments > Comment.likes > Like);
    let q3 = from_path!(u:User > p:Post.comments > c:Comment < Like.users > User);
    
    println!("Q1: {}", q1.to_dsl());
    println!("Q2: {}", q2.to_dsl());
    println!("Q3: {}", q3.to_dsl());
    
    // All queries should contain their respective field names
    assert!(q1.to_dsl().contains("comments"));
    assert!(q2.to_dsl().contains("posts") && q2.to_dsl().contains("comments") && q2.to_dsl().contains("likes"));
    assert!(q3.to_dsl().contains("comments") && q3.to_dsl().contains("users"));
}