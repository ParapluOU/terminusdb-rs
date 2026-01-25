use terminusdb_woql2::path_builder::PathDirection;
use terminusdb_woql2::prelude::*;
use terminusdb_woql2::{parse_direction, parse_node_pattern};

struct User;
struct Post;
struct Comment;

#[test]
fn test_parse_direction() {
    // Test direction parsing
    let forward = parse_direction!(>);
    assert_eq!(forward, PathDirection::Forward);

    let backward = parse_direction!(<);
    assert_eq!(backward, PathDirection::Backward);
}

#[test]
fn test_combined_parsing() {
    // Example of how these macros can work together

    // Parse first node
    let (builder1, field1): (_, Option<&str>) = parse_node_pattern!(User);
    assert!(field1.is_none());

    // Parse direction
    let dir1 = parse_direction!(>);
    assert_eq!(dir1, PathDirection::Forward);

    // Parse second node
    let (builder2, field2): (_, Option<&str>) = parse_node_pattern!(Post.comments);
    assert_eq!(field2, Some("comments"));

    // Parse another direction
    let dir2 = parse_direction!(<);
    assert_eq!(dir2, PathDirection::Backward);

    // Parse third node
    let (builder3, field3): (_, Option<&str>) = parse_node_pattern!(c:Comment);
    assert!(field3.is_none());

    println!("Successfully parsed: User > Post.comments < c:Comment");
}

/// Example of a simplified from_path! macro using the helper macros
macro_rules! simplified_path {
    // Simple nodes
    ($first:ident $dir:tt $second:ident) => {{
        let (mut builder, field1): (_, Option<&str>) = parse_node_pattern!($first);
        let direction = parse_direction!($dir);
        let (_builder2, field2): (_, Option<&str>) = parse_node_pattern!($second);

        println!("Simple: {:?} direction, fields: {:?}, {:?}", direction, field1, field2);
    }};

    // First node with field
    ($first:ident . $field:ident $dir:tt $second:ident) => {{
        let (mut builder, field1): (_, Option<&str>) = parse_node_pattern!($first.$field);
        let direction = parse_direction!($dir);
        let (_builder2, field2): (_, Option<&str>) = parse_node_pattern!($second);

        println!("Field first: {:?} direction, fields: {:?}, {:?}", direction, field1, field2);
    }};

    // First node with variable
    ($var1:ident : $first:ident $dir:tt $var2:ident : $second:ident) => {{
        let (mut builder, field1): (_, Option<&str>) = parse_node_pattern!($var1:$first);
        let direction = parse_direction!($dir);
        let (_builder2, field2): (_, Option<&str>) = parse_node_pattern!($var2:$second);

        println!("Variables: {:?} direction, fields: {:?}, {:?}", direction, field1, field2);
    }};

    // Mixed patterns
    ($var1:ident : $first:ident . $field:ident $dir:tt $second:ident) => {{
        let (mut builder, field1): (_, Option<&str>) = parse_node_pattern!($var1:$first.$field);
        let direction = parse_direction!($dir);
        let (_builder2, field2): (_, Option<&str>) = parse_node_pattern!($second);

        println!("Mixed: {:?} direction, fields: {:?}, {:?}", direction, field1, field2);
    }};
}

#[test]
fn test_simplified_path() {
    // Test the simplified macro
    simplified_path!(User > Post);
    simplified_path!(User.posts > Post);
    simplified_path!(u:User < p:Post);
    simplified_path!(u:User.posts > Comment);
}

/// Even simpler approach using token trees
macro_rules! ultra_simple_path {
    // Match any sequence of tokens
    ($($tokens:tt)+) => {{
        // In a real implementation, we'd parse the token stream
        // This just demonstrates the concept
        println!("Tokens: {}", stringify!($($tokens)+));

        // We could iterate through tokens and dispatch to parse_node_pattern!
        // and parse_direction! as needed
    }};
}

#[test]
fn test_ultra_simple_path() {
    ultra_simple_path!(User > Post < Comment);
    ultra_simple_path!(u:User.posts > p:Post.comments < c:Comment);
}
