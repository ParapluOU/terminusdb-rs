use serde::{Deserialize, Serialize};
use terminusdb_schema::{ToTDBInstance, ToTDBSchema};
use terminusdb_schema_derive::TerminusDBModel;

#[derive(TerminusDBModel, Debug, Clone, Serialize, Deserialize)]
#[tdb(rename_all = "lowercase")]
pub enum TestOrder {
    Asc,
    Desc,
}

#[derive(TerminusDBModel, Debug, Clone, Serialize, Deserialize)]
#[tdb(rename_all = "UPPERCASE")]
pub enum TestStatus {
    Active,
    Inactive,
}

#[derive(TerminusDBModel, Debug, Clone, Serialize, Deserialize)]
#[tdb(rename_all = "snake_case")]
pub enum TestMode {
    ReadOnly,
    WriteOnly,
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminusdb_schema::{Schema, ToTDBInstance, ToTDBSchema};

    #[test]
    fn test_lowercase_rename() {
        let schema = TestOrder::to_schema();

        if let Schema::Enum { values, .. } = schema {
            assert_eq!(values, vec!["asc".to_string(), "desc".to_string()]);
            println!("✅ Lowercase rename test passed: {:?}", values);
        } else {
            panic!("Expected Schema::Enum, got {:?}", schema);
        }

        // Test instance generation too
        let order_asc = TestOrder::Asc;
        let instance = order_asc.to_instance(None);
        assert!(
            instance.properties.contains_key("asc"),
            "Instance should contain 'asc' property"
        );
        println!("✅ Instance serialization test passed: contains 'asc' property");
    }

    #[test]
    fn test_uppercase_rename() {
        let schema = TestStatus::to_schema();

        if let Schema::Enum { values, .. } = schema {
            assert_eq!(values, vec!["ACTIVE".to_string(), "INACTIVE".to_string()]);
            println!("✅ Uppercase rename test passed: {:?}", values);
        } else {
            panic!("Expected Schema::Enum, got {:?}", schema);
        }

        // Test instance generation
        let status = TestStatus::Active;
        let instance = status.to_instance(None);
        assert!(
            instance.properties.contains_key("ACTIVE"),
            "Instance should contain 'ACTIVE' property"
        );
        println!("✅ Uppercase instance test passed");
    }

    #[test]
    fn test_snake_case_rename() {
        let schema = TestMode::to_schema();

        if let Schema::Enum { values, .. } = schema {
            assert_eq!(
                values,
                vec!["read_only".to_string(), "write_only".to_string()]
            );
            println!("✅ Snake case rename test passed: {:?}", values);
        } else {
            panic!("Expected Schema::Enum, got {:?}", schema);
        }

        // Test instance generation
        let mode = TestMode::ReadOnly;
        let instance = mode.to_instance(None);
        assert!(
            instance.properties.contains_key("read_only"),
            "Instance should contain 'read_only' property"
        );
        println!("✅ Snake case instance test passed");
    }
}
