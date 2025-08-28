// This file contains code that should fail to compile
// Uncomment to test the compile-time validation

/*
use terminusdb_schema::{ToTDBSchema, ToTDBInstance, Key};
use terminusdb_schema_derive::TerminusDBModel;
use serde::{Deserialize, Serialize};

// This should fail: String id field with lexical key
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq, TerminusDBModel)]
#[tdb(key = "lexical", key_fields = "name", id_field = "id")]
pub struct InvalidLexicalWithString {
    pub id: String, // ERROR: This should be Option<String>
    pub name: String,
}
*/

#[test]
fn compile_fail_tests_are_commented_out() {
    // This test exists to ensure the file compiles when the invalid code is commented out
    assert!(true);
}