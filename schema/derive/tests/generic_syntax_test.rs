#![cfg(feature = "generic-derive")]

// This test verifies that the derive macro accepts generic syntax
// The compilation of this file proves the macro works

use terminusdb_schema::EntityIDFor;
use terminusdb_schema_derive::TerminusDBModel;

// The exact syntax requested: Model<T> { other: EntityIDFor<T> }
#[allow(dead_code)]
#[derive(Debug, Clone, TerminusDBModel)]
struct Reference<T> {
    id: String,
    referenced_id: EntityIDFor<T>,
}

#[test]
fn generic_syntax_is_accepted() {
    // This test passes if it compiles
    // The derive macro successfully processes generic syntax!
    println!("✅ Generic derive macro accepts Model<T> {{ EntityIDFor<T> }} syntax!");
}