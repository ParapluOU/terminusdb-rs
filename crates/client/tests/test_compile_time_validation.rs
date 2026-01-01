#![cfg(test)]

use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::TerminusDBModel;
use serde::{Deserialize, Serialize};

// This should compile successfully - ServerIDFor with lexical key
#[derive(Clone, Debug, Default, TerminusDBModel)]
#[tdb(key = "lexical", key_fields = "email", id_field = "id")]
pub struct ValidLexicalModel {
    pub id: ServerIDFor<Self>,
    pub email: String,
    pub name: String,
}

// This should compile successfully - ServerIDFor with value_hash key
#[derive(Clone, Debug, Default, TerminusDBModel)]
#[tdb(key = "value_hash", id_field = "id")]
pub struct ValidValueHashModel {
    pub id: ServerIDFor<Self>,
    pub content: String,
}

// This should compile successfully - Random key can use regular String
#[derive(Clone, Debug, Default, TerminusDBModel)]
#[tdb(key = "random", id_field = "id")]
pub struct ValidRandomModel {
    pub id: String,
    pub data: String,
}

// The following would fail at compile time (commented out to allow tests to run):
// 
// #[derive(Clone, Debug, Default, TerminusDBModel)]
// #[tdb(key = "lexical", key_fields = "email", id_field = "id")]
// pub struct InvalidLexicalModel {
//     pub id: String, // Error: must be ServerIDFor<Self> for lexical key
//     pub email: String,
// }

// NOTE: Integration tests disabled - they use deprecated APIs (create_database, check_db_exists,
// insert_schema, insert_instance_and_retrieve) that no longer exist in the client.
// These tests would need to be rewritten to use the new API (ensure_database, insert_entity_schema, etc.)
// when the ServerIDFor functionality is implemented.

// Helper struct for testing embedded models
#[derive(Clone, Debug, Default, TerminusDBModel)]
pub struct ModelWrapper {
    pub name: String,
    pub embedded_model: ValidLexicalModel,
}