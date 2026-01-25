use terminusdb_schema::{EntityIDFor, PrimaryKey, ToTDBInstance};
use terminusdb_schema_derive::TerminusDBModel;

// Test 1: Auto-detection with EntityIDFor<Self>
#[derive(Debug, Clone, TerminusDBModel)]
struct AutoDetectedEntityId {
    id: EntityIDFor<Self>,
    name: String,
}

#[test]
fn test_auto_detect_entity_id_for_self() {
    let instance = AutoDetectedEntityId {
        id: EntityIDFor::random(),
        name: "Test".to_string(),
    };
    let tdb_instance = instance.to_instance(None);
    // ID should be extracted from the id field (auto-detected)
    assert!(tdb_instance.id.is_some());
}

// Test 2: Auto-detection with PrimaryKey!() macro
#[derive(Debug, Clone, TerminusDBModel)]
struct AutoDetectedPrimaryKey {
    id: PrimaryKey!(),
    name: String,
}

#[test]
fn test_auto_detect_primary_key_macro() {
    let instance = AutoDetectedPrimaryKey {
        id: EntityIDFor::random(), // PrimaryKey!() expands to EntityIDFor<Self>
        name: "Test".to_string(),
    };
    let tdb_instance = instance.to_instance(None);
    // ID should be extracted from the id field (auto-detected)
    assert!(tdb_instance.id.is_some());
}

// Test 3: Explicit id_field takes precedence over auto-detection
#[derive(Debug, Clone, TerminusDBModel)]
#[tdb(id_field = "custom_id")]
struct ExplicitIdField {
    id: EntityIDFor<Self>,
    custom_id: String,
    name: String,
}

#[test]
fn test_explicit_id_field_precedence() {
    let instance = ExplicitIdField {
        id: EntityIDFor::random(),
        custom_id: "explicit-custom-id".to_string(),
        name: "Test".to_string(),
    };
    let tdb_instance = instance.to_instance(None);
    // Should use custom_id, not the id field
    assert!(tdb_instance.id.is_some());
    assert!(tdb_instance.id.unwrap().contains("explicit-custom-id"));
}

// Test 4: No auto-detection for wrong field name
#[derive(Debug, Clone, TerminusDBModel)]
struct NoAutoDetectWrongName {
    entity_id: EntityIDFor<Self>, // NOT named "id"
    name: String,
}

#[test]
fn test_no_auto_detect_wrong_name() {
    let instance = NoAutoDetectWrongName {
        entity_id: EntityIDFor::random(),
        name: "Test".to_string(),
    };
    let tdb_instance = instance.to_instance(None);
    // ID should be None since entity_id is not auto-detected (wrong name)
    assert!(tdb_instance.id.is_none());
}

// Test 5: No auto-detection for wrong type (plain String)
#[derive(Debug, Clone, TerminusDBModel)]
struct NoAutoDetectWrongType {
    id: String, // Not EntityIDFor<Self> or PrimaryKey!()
    name: String,
}

#[test]
fn test_no_auto_detect_wrong_type() {
    let instance = NoAutoDetectWrongType {
        id: "my-id".to_string(),
        name: "Test".to_string(),
    };
    let tdb_instance = instance.to_instance(None);
    // ID should be None since String is not auto-detected
    assert!(tdb_instance.id.is_none());
}

// Test 6: EntityIDFor with different type parameter (not Self) - should NOT be auto-detected
#[derive(Debug, Clone, TerminusDBModel)]
struct OtherModel {
    name: String,
}

#[derive(Debug, Clone, TerminusDBModel)]
struct NoAutoDetectOtherType {
    id: EntityIDFor<OtherModel>, // Not EntityIDFor<Self>
    name: String,
}

#[test]
fn test_no_auto_detect_other_type() {
    let instance = NoAutoDetectOtherType {
        id: EntityIDFor::random(),
        name: "Test".to_string(),
    };
    let tdb_instance = instance.to_instance(None);
    // EntityIDFor<OtherModel> should NOT trigger auto-detection
    assert!(tdb_instance.id.is_none());
}

// Test 7: PrimaryKey!() produces EntityIDFor with random UUID
#[derive(Debug, Clone, TerminusDBModel)]
struct TestPrimaryKeyRandom {
    id: PrimaryKey!(),
    name: String,
}

#[test]
fn test_primary_key_macro_random() {
    let id: EntityIDFor<TestPrimaryKeyRandom> = EntityIDFor::random();
    // UUID is 36 characters with dashes
    assert!(id.id().len() == 36);
}

// Test 8: PrimaryKey!() with specific ID
#[derive(Debug, Clone, TerminusDBModel)]
struct TestPrimaryKeyWithId {
    id: PrimaryKey!(),
    name: String,
}

#[test]
fn test_primary_key_macro_with_id() {
    let id: EntityIDFor<TestPrimaryKeyWithId> = EntityIDFor::new_unchecked("custom-123").unwrap();
    assert_eq!(id.id(), "custom-123");
}
