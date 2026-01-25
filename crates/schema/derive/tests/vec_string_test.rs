use serde::{Deserialize, Serialize};
use terminusdb_schema::{Schema, ToTDBInstance, ToTDBSchema};
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

/// Simple test struct to check if Vec<String> works
#[derive(TerminusDBModel, FromTDBInstance, Debug, Clone, PartialEq)]
#[tdb(doc = "Test model for Vec<String>")]
pub struct VecStringTest {
    /// Unique identifier
    pub id: String,

    /// Name
    pub name: String,

    /// List of string tags
    pub tags: Vec<String>,

    /// Optional list of keywords
    pub keywords: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec_string_schema() {
        let schema = <VecStringTest as ToTDBSchema>::to_schema();

        if let Schema::Class { properties, .. } = schema {
            // Check that we have all properties
            assert_eq!(properties.len(), 4);

            // Check tags property
            let tags_prop = properties.iter().find(|p| p.name == "tags").unwrap();
            assert_eq!(tags_prop.class, "xsd:string");
            assert_eq!(tags_prop.r#type, Some(terminusdb_schema::TypeFamily::List));

            // Check keywords property
            let keywords_prop = properties.iter().find(|p| p.name == "keywords").unwrap();
            assert_eq!(keywords_prop.class, "xsd:string");
            assert_eq!(
                keywords_prop.r#type,
                Some(terminusdb_schema::TypeFamily::Array(1))
            );
        } else {
            panic!("Expected Schema::Class");
        }
    }

    #[test]
    fn test_vec_string_instance() {
        // Create an instance for testing
        let model = VecStringTest {
            id: "test1".to_string(),
            name: "Test Model".to_string(),
            tags: vec!["tag1".to_string(), "tag2".to_string(), "tag3".to_string()],
            keywords: Some(vec!["keyword1".to_string(), "keyword2".to_string()]),
        };

        // Verify we can generate an instance without errors
        let instance = model.to_instance(None);

        // Instance should contain all properties
        assert_eq!(instance.properties.len(), 4);
    }

    #[test]
    fn test_vec_string_instance_with_none() {
        // Test with None keywords
        let model = VecStringTest {
            id: "test2".to_string(),
            name: "Test Model 2".to_string(),
            tags: vec!["only".to_string(), "tags".to_string()],
            keywords: None,
        };

        // Verify we can generate an instance without errors
        let instance = model.to_instance(None);

        // Instance should contain all properties
        assert_eq!(instance.properties.len(), 4);
    }
}
