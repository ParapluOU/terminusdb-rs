#![cfg(feature = "generic-derive")]

use std::fmt::Debug;
use terminusdb_schema::{
    EntityIDFor, FromTDBInstance, InstanceFromJson, ToJson, ToSchemaClass, ToTDBInstance,
    ToTDBSchema,
};
use terminusdb_schema_derive::TerminusDBModel;

// First, create a simple model that implements all required traits
#[derive(Debug, Clone, TerminusDBModel)]
struct User {
    id: String,
    name: String,
    email: String,
}

#[derive(Debug, Clone, TerminusDBModel)]
struct Product {
    id: String,
    name: String,
    price: f64,
}

// Now create a generic model that references another model
#[derive(Debug, Clone, TerminusDBModel)]
struct Reference<T>
where
    T: ToTDBSchema
        + ToSchemaClass
        + Debug
        + Clone
        + FromTDBInstance
        + InstanceFromJson
        + Send
        + Sync,
{
    id: String,
    referenced_id: EntityIDFor<T>,
    description: String,
}

// A more complex example with multiple generic uses
#[derive(Debug, Clone, TerminusDBModel)]
struct Relation<T>
where
    T: ToTDBSchema
        + ToSchemaClass
        + Debug
        + Clone
        + FromTDBInstance
        + InstanceFromJson
        + Send
        + Sync,
{
    id: String,
    source: EntityIDFor<T>,
    target: EntityIDFor<T>,
    relation_type: String,
}

// Container that holds actual instances
#[derive(Debug, Clone, TerminusDBModel)]
struct Container<T>
where
    T: ToTDBSchema
        + ToSchemaClass
        + Debug
        + Clone
        + FromTDBInstance
        + InstanceFromJson
        + Send
        + Sync,
{
    id: String,
    name: String,
    items: Vec<EntityIDFor<T>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reference_with_user() {
        // Create a reference to a User
        let user_ref = Reference::<User> {
            id: "ref-1".to_string(),
            referenced_id: EntityIDFor::new("user-123").unwrap(),
            description: "Reference to user 123".to_string(),
        };

        // Test schema generation
        let schema = Reference::<User>::to_schema();
        println!("Schema class name: {:?}", schema.class_name());
        println!(
            "ToSchemaClass::to_class() = {:?}",
            Reference::<User>::to_class()
        );
        assert_eq!(schema.class_name(), "Reference<User>");

        // Test instance conversion
        let instance = user_ref.to_instance(None);
        assert!(instance.has_property("id"));
        assert!(instance.has_property("referenced_id"));
        assert!(instance.has_property("description"));

        // Test JSON round-trip
        let json = instance.to_json();
        println!(
            "Instance JSON: {}",
            serde_json::to_string_pretty(&json).unwrap()
        );
        let recovered_instance = Reference::<User>::instance_from_json(json.clone()).unwrap();
        println!(
            "Recovered instance schema: {:?}",
            recovered_instance.schema.class_name()
        );

        // Use from_instance instead of from_json for proper type checking
        let recovered = Reference::<User>::from_instance(&recovered_instance).unwrap();
        assert_eq!(recovered.id, user_ref.id);
        assert_eq!(recovered.referenced_id, user_ref.referenced_id);
    }

    #[test]
    fn test_reference_with_product() {
        // Same structure works with Product type
        let product_ref = Reference::<Product> {
            id: "ref-2".to_string(),
            referenced_id: EntityIDFor::new("product-456").unwrap(),
            description: "Reference to product 456".to_string(),
        };

        let instance = product_ref.to_instance(None);
        assert!(instance.has_property("referenced_id"));
    }

    #[test]
    fn test_relation_between_users() {
        let relation = Relation::<User> {
            id: "rel-1".to_string(),
            source: EntityIDFor::new("user-1").unwrap(),
            target: EntityIDFor::new("user-2").unwrap(),
            relation_type: "follows".to_string(),
        };

        let schema = Relation::<User>::to_schema();
        assert_eq!(schema.class_name(), "Relation<User>");

        let instance = relation.to_instance(None);
        assert!(instance.has_property("source"));
        assert!(instance.has_property("target"));
    }

    #[test]
    fn test_container_with_users() {
        let container = Container::<User> {
            id: "container-1".to_string(),
            name: "User Group".to_string(),
            items: vec![
                EntityIDFor::new("user-1").unwrap(),
                EntityIDFor::new("user-2").unwrap(),
                EntityIDFor::new("user-3").unwrap(),
            ],
        };

        let instance = container.to_instance(None);
        assert!(instance.has_property("items"));

        // Verify the schema includes the container
        let schemas = Container::<User>::to_schema_tree();
        assert!(schemas.iter().any(|s| s.class_name() == "Container<User>"));
    }

    #[test]
    fn test_schema_generation_includes_referenced_types() {
        // For Reference<User>, the schema tree should include both Reference and User schemas
        let schemas = Reference::<User>::to_schema_tree();

        // Should contain at least the Reference schema
        assert!(schemas.iter().any(|s| s.class_name() == "Reference<User>"));

        // Note: User schema might not be included if EntityIDFor doesn't trigger schema collection
        // This depends on the ToMaybeTDBSchema implementation for EntityIDFor
    }

    #[test]
    fn test_different_generic_instantiations() {
        // Verify that Reference<User> and Reference<Product> work independently
        let user_ref = Reference::<User> {
            id: "ref-user".to_string(),
            referenced_id: EntityIDFor::new("user-1").unwrap(),
            description: "User reference".to_string(),
        };

        let product_ref = Reference::<Product> {
            id: "ref-product".to_string(),
            referenced_id: EntityIDFor::new("product-1").unwrap(),
            description: "Product reference".to_string(),
        };

        // Both should generate valid instances
        let user_instance = user_ref.to_instance(None);
        let product_instance = product_ref.to_instance(None);

        // Both use the same schema class name (this is a limitation)
        assert_eq!(
            Reference::<User>::to_schema().class_name(),
            "Reference<User>"
        );
        assert_eq!(
            Reference::<Product>::to_schema().class_name(),
            "Reference<Product>"
        );
    }
}
