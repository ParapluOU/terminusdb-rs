use terminusdb_woql2::prelude::*;

struct D;
struct E;
struct F;

#[test]
fn test_specific_pattern() {
    // Test the problematic pattern in isolation
    let query = from_path!(D > E.field > F);
    println!("D > E.field > F worked!");
    
    let dsl = query.to_dsl();
    println!("DSL: {}", dsl);
}