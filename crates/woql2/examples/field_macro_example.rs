//! Example demonstrating the field! macro for type-checked property names

use terminusdb_woql2::prelude::*;

// Example model
#[allow(dead_code)]
#[derive(Clone, Debug)]
struct Person {
    name: String,
    age: i32,
    email: Option<String>,
}

fn main() {
    // Using field! macro provides compile-time verification
    let query = and!(
        triple!(var!(p), "rdf:type", "Person"),
        triple!(var!(p), field!(Person:name), var!(name)),
        data_triple!(var!(p), field!(Person:age), var!(age)),
        optional!(triple!(var!(p), field!(Person:email), var!(email))),
        greater!(var!(age), data!(18))
    );
    
    println!("Generated query: {:?}", query);
    
    // The following would fail to compile:
    // let bad_query = triple!(var!(p), field!(Person:nam), var!(n));
    // Error: no field `nam` on type `Person`
    
    // Compare with traditional approach (no compile-time checking):
    let traditional = and!(
        triple!(var!(p), "rdf:type", "Person"),
        triple!(var!(p), "name", var!(name)),      // Typo here wouldn't be caught
        triple!(var!(p), "age", var!(age)),        // until runtime
        triple!(var!(p), "email", var!(email))
    );
    
    println!("Traditional query: {:?}", traditional);
}