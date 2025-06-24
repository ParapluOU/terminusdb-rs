use terminusdb_schema::{Schema, ToTDBInstance, ToTDBSchema};
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
#[derive(TerminusDBModel, Debug)]
#[tdb(
    class_name = "Status",
    doc = "Status represents the current state of an entity"
)]
pub enum Status {
    Active,
    Inactive,
    Pending,
    Expired,
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
