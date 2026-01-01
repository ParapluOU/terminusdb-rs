use serde::{Deserialize, Serialize};
use terminusdb_schema::FromTDBInstance;
use terminusdb_schema::{Schema, TDBEnum, ToTDBInstance, ToTDBSchema};
use terminusdb_schema_derive::TerminusDBModel;
/// Color enum demonstrates a basic enum model for TerminusDB
#[derive(TerminusDBModel, Debug)]
#[tdb(class_name = "Color")]
pub enum Color {
    Red,
    Green,
    Blue,
}

/// Status enum demonstrates another enum model with documentation
#[derive(TerminusDBModel, Debug, Clone)]
#[tdb(
    class_name = "Status",
    doc = "Status represents the current state of an entity",
    subdocument = true
)]
pub enum Status {
    Active,
    Inactive,
    Pending,
    Expired,
}

#[derive(TerminusDBModel, Debug, Clone)]
pub struct Comment {
    status: Status
}

// #[cfg(test)]
// mod tests {
//     use terminusdb_schema::FromTDBInstance;

//     use super::*;

#[test]
fn test_enum_deserde() {
    let instance = Comment {
        status: Status::Active,
    };
    let json = instance.to_json();

    dbg!(&json);

    let instance = Comment::from_json(json).unwrap();

    let json = Status::Active.to_json();

    dbg!(&json);

    Status::from_json(json).unwrap();
}

#[test]
fn test_simple_enum() {
    let schema = <Color as terminusdb_schema::ToTDBSchema>::to_schema();

    if let Schema::Enum { id, values, .. } = schema {
        assert_eq!(id, "Color");
        assert_eq!(values.len(), 3);
        assert!(values.contains(&"red".to_string()));
        assert!(values.contains(&"green".to_string()));
        assert!(values.contains(&"blue".to_string()));
    } else {
        panic!("Expected Enum schema");
    }
}

#[test]
fn test_documented_enum() {
    let schema = <Status as terminusdb_schema::ToTDBSchema>::to_schema();

    if let Schema::Enum {
        id,
        values,
        documentation,
        ..
    } = schema
    {
        assert_eq!(id, "Status");
        assert_eq!(values.len(), 4);
        assert!(values.contains(&"active".to_string()));
        assert!(values.contains(&"inactive".to_string()));
        assert!(values.contains(&"pending".to_string()));
        assert!(values.contains(&"expired".to_string()));

        // Just check that documentation exists
        assert!(documentation.is_some());
    } else {
        panic!("Expected Enum schema");
    }
}

/// Test for multi-word PascalCase variants (the original bug case)
/// This verifies that TDBEnum correctly handles variants like FullyImported
#[derive(TerminusDBModel, Debug, Clone, PartialEq)]
#[tdb(subdocument = true)]
pub enum ImportStatus {
    LocallyCreated,
    MetadataOnly,
    ImportInProgress,
    FullyImported,
    XmlNotFound,
    ImportFailed,
}

#[test]
fn test_tdbenum_multiword_variants() {
    // Test variants() returns all variants
    let variants = ImportStatus::variants();
    assert_eq!(variants.len(), 6);

    // Test to_tdb_value() produces lowercase
    assert_eq!(ImportStatus::LocallyCreated.to_tdb_value(), "locallycreated");
    assert_eq!(ImportStatus::MetadataOnly.to_tdb_value(), "metadataonly");
    assert_eq!(ImportStatus::ImportInProgress.to_tdb_value(), "importinprogress");
    assert_eq!(ImportStatus::FullyImported.to_tdb_value(), "fullyimported");
    assert_eq!(ImportStatus::XmlNotFound.to_tdb_value(), "xmlnotfound");
    assert_eq!(ImportStatus::ImportFailed.to_tdb_value(), "importfailed");

    // Test from_tdb_value() correctly parses lowercase back to variants
    // This was the original bug - "fullyimported" -> "Fullyimported" (wrong) instead of "FullyImported"
    assert_eq!(ImportStatus::from_tdb_value("locallycreated"), Some(ImportStatus::LocallyCreated));
    assert_eq!(ImportStatus::from_tdb_value("metadataonly"), Some(ImportStatus::MetadataOnly));
    assert_eq!(ImportStatus::from_tdb_value("importinprogress"), Some(ImportStatus::ImportInProgress));
    assert_eq!(ImportStatus::from_tdb_value("fullyimported"), Some(ImportStatus::FullyImported));
    assert_eq!(ImportStatus::from_tdb_value("xmlnotfound"), Some(ImportStatus::XmlNotFound));
    assert_eq!(ImportStatus::from_tdb_value("importfailed"), Some(ImportStatus::ImportFailed));

    // Test round-trip
    for variant in ImportStatus::variants() {
        let tdb_value = variant.to_tdb_value();
        let parsed = ImportStatus::from_tdb_value(&tdb_value);
        assert_eq!(parsed, Some(variant));
    }

    // Test invalid value returns None
    assert_eq!(ImportStatus::from_tdb_value("invalid"), None);
    assert_eq!(ImportStatus::from_tdb_value("FullyImported"), None); // Case sensitive
}

#[test]
fn test_multiword_enum_json_deserialization() {
    use terminusdb_schema::json::InstanceFromJson;

    // This simulates what TerminusDB returns - lowercase enum value
    let json = serde_json::json!("fullyimported");

    // This should work now (previously failed with serde error)
    let instance = ImportStatus::instance_from_json(json).expect("Should deserialize multiword enum");

    // Verify the enum value is correct by checking the instance properties
    assert!(instance.is_enum());
    assert_eq!(instance.enum_value(), Some("fullyimported".to_string()));
}
// }
