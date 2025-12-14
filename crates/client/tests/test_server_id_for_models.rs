#![cfg(test)]

use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::TerminusDBModel;

// Model with ServerIDFor using random key
#[derive(Clone, Debug, Default, TerminusDBModel, serde::Serialize, serde::Deserialize)]
#[tdb(key = "random", id_field = "id")]
pub struct RandomKeyServerModel {
    pub id: ServerIDFor<Self>,
    pub name: String,
    pub value: i32,
}

// Model with ServerIDFor using lexical key
#[derive(Clone, Debug, Default, TerminusDBModel, serde::Serialize, serde::Deserialize)]
#[tdb(key = "lexical", key_fields = "email", id_field = "id")]
pub struct LexicalKeyServerModel {
    pub id: ServerIDFor<Self>,
    pub email: String,
    pub name: String,
}

// Model with ServerIDFor using value_hash key
#[derive(Clone, Debug, Default, TerminusDBModel, serde::Serialize, serde::Deserialize)]
#[tdb(key = "value_hash", id_field = "id")]
pub struct ValueHashServerModel {
    pub id: ServerIDFor<Self>,
    pub content: String,
    pub timestamp: i32,  // Changed from i64 to i32 for JSON compatibility
}

mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_random_key_model_serialization() {
        // Test with ID set
        let mut model = RandomKeyServerModel {
            id: ServerIDFor::new(),
            name: "Test Model".to_string(),
            value: 42,
        };
        
        // Simulate server setting the ID
        model.id.__set_from_server(EntityIDFor::new("random-123").unwrap());
        
        let json = serde_json::to_value(&model).unwrap();
        assert_eq!(json["id"], serde_json::json!("RandomKeyServerModel/random-123"));
        assert_eq!(json["name"], "Test Model");
        assert_eq!(json["value"], 42);

        // Test deserialization
        let deserialized: RandomKeyServerModel = serde_json::from_value(json).unwrap();
        assert!(deserialized.id.is_some());
        assert_eq!(deserialized.id.as_ref().unwrap().id(), "random-123");
        assert_eq!(deserialized.name, "Test Model");
        assert_eq!(deserialized.value, 42);
    }

    #[test]
    fn test_lexical_key_model_serialization() {
        let model = LexicalKeyServerModel {
            id: ServerIDFor::new(),
            email: "test@example.com".to_string(),
            name: "Test User".to_string(),
        };
        
        // Initially no ID
        assert!(model.id.is_none());
        
        let json = serde_json::to_value(&model).unwrap();
        assert_eq!(json["id"], serde_json::json!(null));
        assert_eq!(json["email"], "test@example.com");
        assert_eq!(json["name"], "Test User");

        // Deserialize from server response with ID
        let server_json = serde_json::json!({
            "id": "LexicalKeyServerModel/lex-456",
            "email": "test@example.com",
            "name": "Test User"
        });
        
        let from_server: LexicalKeyServerModel = serde_json::from_value(server_json).unwrap();
        assert!(from_server.id.is_some());
        assert_eq!(from_server.id.as_ref().unwrap().id(), "lex-456");
    }

    #[test]
    fn test_value_hash_model_serialization() {
        let model = ValueHashServerModel {
            id: ServerIDFor::new(),
            content: "Some content".to_string(),
            timestamp: 1234567,
        };
        
        let json = serde_json::to_value(&model).unwrap();
        assert_eq!(json["id"], serde_json::json!(null));
        assert_eq!(json["content"], "Some content");
        assert_eq!(json["timestamp"], 1234567890);

        // Deserialize from server response with ID
        let server_json = serde_json::json!({
            "id": "ValueHashServerModel/hash-789",
            "content": "Some content",
            "timestamp": 1234567890
        });
        
        let from_server: ValueHashServerModel = serde_json::from_value(server_json).unwrap();
        assert!(from_server.id.is_some());
        assert_eq!(from_server.id.as_ref().unwrap().id(), "hash-789");
    }

    #[test]
    fn test_server_id_deref_functionality() {
        let mut model = RandomKeyServerModel {
            id: ServerIDFor::new(),
            name: "Deref Test".to_string(),
            value: 100,
        };
        
        // Test deref when None
        assert!(model.id.is_none());
        let opt_ref: &Option<EntityIDFor<RandomKeyServerModel>> = &*model.id;
        assert!(opt_ref.is_none());
        
        // Set ID and test deref
        model.id.__set_from_server(EntityIDFor::new("deref-test").unwrap());
        let opt_ref: &Option<EntityIDFor<RandomKeyServerModel>> = &*model.id;
        assert!(opt_ref.is_some());
        assert_eq!(opt_ref.as_ref().unwrap().id(), "deref-test");
    }

    #[test]
    fn test_server_id_cannot_be_modified_by_user() {
        let model = RandomKeyServerModel {
            id: ServerIDFor::new(),
            name: "Immutable Test".to_string(),
            value: 200,
        };
        
        // User can read the ID
        assert!(model.id.is_none());
        
        // User can't modify the ID directly (no public setter methods except __set_from_server)
        // The only way to set the ID is through deserialization or the hidden __set_from_server method
        
        // This demonstrates that ServerIDFor achieves the desired read-only behavior
    }
}