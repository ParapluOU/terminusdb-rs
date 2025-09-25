use terminusdb_woql2::prelude::*;

struct A;
struct B;
struct C;

#[test]
fn test_debug_simple() {
    // This should match: ($node:ident $dir:tt $($rest:tt)+)
    // Where $node = A, $dir = >, $rest = B
    let query = from_path!(A > B);
    println!("A > B worked!");
    
    // Test backward
    let query2 = from_path!(B < A);
    println!("B < A worked!");
    
    // Test 3-chain
    let query3 = from_path!(A > B > C);
    println!("A > B > C worked!");
    
    // Test mixed
    let query4 = from_path!(A > B < C);
    println!("A > B < C worked!");
}

#[test]
fn test_debug_custom_var() {
    let query = from_path!(a:A > B);
    println!("a:A > B worked!");
    
    let query2 = from_path!(A > b:B);
    println!("A > b:B worked!");
    
    let query3 = from_path!(a:A > b:B);
    println!("a:A > b:B worked!");
}

#[test]
fn test_debug_field() {
    let query = from_path!(A.field > B);
    println!("A.field > B worked!");
    
    let query2 = from_path!(a:A.field > B);
    println!("a:A.field > B worked!");
}