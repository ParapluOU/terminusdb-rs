#![cfg(test)]

use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::TerminusDBModel;
use serde::{Deserialize, Serialize};

// This should compile successfully - ServerIDFor with lexical key
#[derive(Clone, Debug, Default, TerminusDBModel, Serialize, Deserialize)]
#[tdb(key = "lexical", key_fields = "email", id_field = "id")]
pub struct ValidLexicalModel {
    pub id: ServerIDFor<Self>,
    pub email: String,
    pub name: String,
}

// This should compile successfully - ServerIDFor with value_hash key
#[derive(Clone, Debug, Default, TerminusDBModel, Serialize, Deserialize)]
#[tdb(key = "value_hash", id_field = "id")]
pub struct ValidValueHashModel {
    pub id: ServerIDFor<Self>,
    pub content: String,
}

// This should compile successfully - Random key can use regular String
#[derive(Clone, Debug, Default, TerminusDBModel, Serialize, Deserialize)]
#[tdb(key = "random", id_field = "id")]
pub struct ValidRandomModel {
    pub id: String,
    pub data: String,
}

// The following would fail at compile time (commented out to allow tests to run):
// 
// #[derive(Clone, Debug, Default, TerminusDBModel, Serialize, Deserialize)]
// #[tdb(key = "lexical", key_fields = "email", id_field = "id")]
// pub struct InvalidLexicalModel {
//     pub id: String, // Error: must be ServerIDFor<Self> for lexical key
//     pub email: String,
// }

#[cfg(feature = "integration-tests")]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    #[ignore]
    async fn test_server_id_for_reinsert() {
        let client = TerminusDBHttpClient::local_node();
        let db = "test_compile_validation";
        
        // Setup database
        if client.check_db_exists(db).await.unwrap_or(false) {
            client.delete_database(db).await.unwrap();
        }
        client.create_database(db).await.unwrap();
        
        let args = DocumentInsertArgs {
            spec: DatabaseSpec::new(db),
            author: Some("test".to_string()),
            message: Some("test insert".to_string()),
            ..Default::default()
        };
        
        // Insert schema
        client.insert_schema(&ValidLexicalModel::to_schema(), args.clone()).await.unwrap();
        
        // Create and insert a model
        let model = ValidLexicalModel {
            id: ServerIDFor::new(),
            email: "test@example.com".to_string(),
            name: "Test User".to_string(),
        };
        
        // Insert and retrieve to populate the ID
        let (saved_model, _) = client.insert_instance_and_retrieve(&model, args.clone()).await.unwrap();
        assert!(saved_model.id.is_some());
        
        // Now test re-inserting the model with populated ServerIDFor
        // This should NOT panic thanks to our compile-time validation
        let wrapper_model = ModelWrapper {
            name: "wrapper".to_string(),
            embedded_model: saved_model,
        };
        
        // This would have panicked before, but now works fine
        let result = client.insert_instance(&wrapper_model, args.clone()).await;
        assert!(result.is_ok());
        
        // Clean up
        client.delete_database(db).await.unwrap();
    }
}

// Helper struct for testing embedded models
#[derive(Clone, Debug, Default, TerminusDBModel, Serialize, Deserialize)]
pub struct ModelWrapper {
    pub name: String,
    pub embedded_model: ValidLexicalModel,
}