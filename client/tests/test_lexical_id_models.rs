#![cfg(test)]

use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::TerminusDBModel;
use serde::{Serialize, Deserialize};

// Model with LexicalID using lexical key
#[derive(Clone, Debug, TerminusDBModel, Serialize, Deserialize)]
#[tdb(key = "lexical", key_fields = "email", id_field = "id")]
pub struct UserWithLexicalID {
    pub id: LexicalID<Self>,
    pub email: String,
    pub name: String,
}

// Model with HashID using hash key
#[derive(Clone, Debug, TerminusDBModel, Serialize, Deserialize)]
#[tdb(key = "hash", key_fields = "email,name", id_field = "id")]
pub struct UserWithHashID {
    pub id: HashID<Self>,
    pub email: String,
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_lexical_id_serialization() {
        let user = UserWithLexicalID {
            id: LexicalID::from_single_field("test@example.com"),
            email: "test@example.com".to_string(),
            name: "Test User".to_string(),
        };
        
        let json = serde_json::to_value(&user).unwrap();
        assert_eq!(json["id"], "UserWithLexicalID/test%40example.com");
        assert_eq!(json["email"], "test@example.com");
        assert_eq!(json["name"], "Test User");
    }
    
    #[test]
    fn test_hash_id_serialization() {
        let user = UserWithHashID {
            id: HashID::from_fields(&[
                ("email", "test@example.com"),
                ("name", "Test User"),
            ]),
            email: "test@example.com".to_string(),
            name: "Test User".to_string(),
        };
        
        let json = serde_json::to_value(&user).unwrap();
        let id_str = json["id"].as_str().unwrap();
        assert!(id_str.starts_with("UserWithHashID/"));
        assert_eq!(id_str.len(), "UserWithHashID/".len() + 64); // Hash is 64 chars
    }
}