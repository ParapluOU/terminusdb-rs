use serde::{Deserialize, Serialize};
use terminusdb_schema::{EntityIDFor, Key, ServerIDFor, ToTDBInstance, ToTDBSchema};
use terminusdb_schema_derive::TerminusDBModel;

// Test model with ServerIDFor and hash key (required for non-random keys)
#[derive(Debug, Clone, Eq, PartialEq, TerminusDBModel)]
#[tdb(key = "hash", key_fields = "email", id_field = "id")]
pub struct UserWithEntityID {
    pub id: ServerIDFor<Self>,
    pub email: String,
    pub name: String,
}

// Test model with EntityIDFor and random key
#[derive(Debug, Clone, Eq, PartialEq, TerminusDBModel)]
#[tdb(key = "random", id_field = "id")]
pub struct RandomUserWithEntityID {
    pub id: EntityIDFor<Self>,
    pub name: String,
}

#[test]
fn test_entity_id_with_hash_key() {
    assert_eq!(UserWithEntityID::id(), Some("UserWithEntityID".to_string()));

    match UserWithEntityID::key() {
        Key::Hash(fields) => {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0], "email");
        }
        _ => panic!("Expected Hash key"),
    }

    // Create instance with new ServerIDFor (for non-random keys)
    let user = UserWithEntityID {
        id: ServerIDFor::new(),
        email: "test@example.com".to_string(),
        name: "Test User".to_string(),
    };

    let instance = user.to_instance(None);
    // For ServerIDFor with non-random keys, the ID is not set until server assigns it
    assert!(instance.id.is_none());
}

#[test]
fn test_entity_id_with_random_key() {
    assert_eq!(
        RandomUserWithEntityID::id(),
        Some("RandomUserWithEntityID".to_string())
    );
    assert_eq!(RandomUserWithEntityID::key(), Key::Random);

    // Create instance with a specific EntityIDFor value
    let user = RandomUserWithEntityID {
        id: EntityIDFor::new("custom-user-id").unwrap(),
        name: "Random User".to_string(),
    };

    let instance = user.to_instance(None);
    // For random keys, the ID should include the class prefix
    assert_eq!(
        instance.id,
        Some("RandomUserWithEntityID/custom-user-id".to_string())
    );
}

#[test]
fn test_entity_id_serialization() {
    // Test that ServerIDFor properly converts via ToInstanceProperty
    let user = UserWithEntityID {
        id: ServerIDFor::new(),
        email: "test@example.com".to_string(),
        name: "Test".to_string(),
    };

    let instance = user.to_instance(None);

    // Check that properties are correctly set
    assert_eq!(
        instance.properties.get("email").unwrap(),
        &terminusdb_schema::InstanceProperty::Primitive(terminusdb_schema::PrimitiveValue::String(
            "test@example.com".to_string()
        ))
    );
    assert_eq!(
        instance.properties.get("name").unwrap(),
        &terminusdb_schema::InstanceProperty::Primitive(terminusdb_schema::PrimitiveValue::String(
            "Test".to_string()
        ))
    );
}
