use chrono::{DateTime, Utc};
use terminusdb_schema::{Schema, ToTDBInstance, ToTDBSchema};
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use anyhow::*;

/// Test struct with UUID, DateTime and HashMap fields to demonstrate
/// their handling with TerminusDBModel
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, Serialize, Deserialize)]
#[tdb(doc = "A test model for special types")]
pub struct SpecialTypesModel {
    /// Unique identifier
    pub id: Uuid,

    /// Name
    pub name: String,

    /// Optional description
    pub description: Option<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Update timestamp
    pub updated_at: DateTime<Utc>,

    /// Additional metadata as a JSON object
    pub metadata: HashMap<String, serde_json::Value>,

    /// Tags
    pub tags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_special_types_schema() {
        let schema = <SpecialTypesModel as ToTDBSchema>::to_schema();

        if let Schema::Class { properties, .. } = schema {
            // Check UUID field
            let id_prop = properties.iter().find(|p| p.name == "id").unwrap();
            // UUID should be treated as a string in TerminusDB
            assert_eq!(id_prop.class, "xsd:string");

            // Check DateTime fields
            let created_at_prop = properties.iter().find(|p| p.name == "created_at").unwrap();
            assert_eq!(created_at_prop.class, "xsd:dateTime");

            let updated_at_prop = properties.iter().find(|p| p.name == "updated_at").unwrap();
            assert_eq!(updated_at_prop.class, "xsd:dateTime");

            // Check HashMap field
            let metadata_prop = properties.iter().find(|p| p.name == "metadata").unwrap();
            // HashMap should be converted to a JSON value
            assert_eq!(metadata_prop.class, "sys:JSON");

            // Check Vec field
            let tags_prop = properties.iter().find(|p| p.name == "tags").unwrap();
            assert_eq!(tags_prop.class, "xsd:string");
            assert_eq!(
                tags_prop.r#type,
                Some(terminusdb_schema::TypeFamily::List)
            );
        } else {
            panic!("Expected Schema::Class");
        }
    }

    #[test]
    fn test_special_types_instance() {
        // Create an instance for testing
        let model = SpecialTypesModel {
            id: Uuid::new_v4(),
            name: "Test Model".to_string(),
            description: Some("A test model".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("key1".to_string(), serde_json::json!("value1"));
                map.insert("key2".to_string(), serde_json::json!(42));
                map
            },
            tags: vec!["tag1".to_string(), "tag2".to_string()],
        };

        // Verify we can generate an instance without errors
        let instance = model.to_instance(None);

        // Instance should contain all properties
        assert_eq!(instance.properties.len(), 7);
    }
}
