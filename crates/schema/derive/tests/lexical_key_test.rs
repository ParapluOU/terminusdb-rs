use serde::{Deserialize, Serialize};
use terminusdb_schema::{Key, ToTDBInstance, ToTDBSchema};
use terminusdb_schema_derive::TerminusDBModel;

#[derive(Debug, Clone, Eq, PartialEq, TerminusDBModel)]
#[tdb(key = "lexical", key_fields = "first_name,last_name")]
pub struct PersonWithLexicalKey {
    pub first_name: String,
    pub last_name: String,
    pub age: i32,
}

#[derive(Debug, Clone, Eq, PartialEq, TerminusDBModel)]
#[tdb(key = "hash", key_fields = "email,phone")]
pub struct ContactWithHashKey {
    pub email: String,
    pub phone: String,
    pub address: String,
}

#[test]
fn test_lexical_key_with_multiple_fields() {
    assert_eq!(
        PersonWithLexicalKey::id(),
        Some("PersonWithLexicalKey".to_string())
    );

    match PersonWithLexicalKey::key() {
        Key::Lexical(fields) => {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0], "first_name");
            assert_eq!(fields[1], "last_name");
        }
        _ => panic!("Expected Lexical key"),
    }
}

#[test]
fn test_hash_key_with_multiple_fields() {
    assert_eq!(
        ContactWithHashKey::id(),
        Some("ContactWithHashKey".to_string())
    );

    match ContactWithHashKey::key() {
        Key::Hash(fields) => {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0], "email");
            assert_eq!(fields[1], "phone");
        }
        _ => panic!("Expected Hash key"),
    }
}

#[test]
fn test_lexical_key_fallback_to_id() {
    #[derive(Debug, Clone, Eq, PartialEq, TerminusDBModel)]
    #[tdb(key = "lexical")]
    pub struct DefaultLexicalKey {
        pub id: String,
        pub name: String,
    }

    match DefaultLexicalKey::key() {
        Key::Lexical(fields) => {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0], "id");
        }
        _ => panic!("Expected Lexical key with default id field"),
    }
}
