#![cfg(feature = "generic-derive")]

use std::fmt::Debug;
use terminusdb_schema::{
    EntityIDFor, FromTDBInstance, InstanceFromJson, Schema, ToJson, ToSchemaClass, ToTDBInstance,
    ToTDBSchema,
};
use terminusdb_schema_derive::TerminusDBModel;

// Trait alias for all required bounds - makes the code cleaner
trait Model:
    ToTDBSchema + ToSchemaClass + Debug + Clone + FromTDBInstance + InstanceFromJson + Send + Sync
{
}
impl<T> Model for T where
    T: ToTDBSchema
        + ToSchemaClass
        + Debug
        + Clone
        + FromTDBInstance
        + InstanceFromJson
        + Send
        + Sync
{
}

#[derive(Debug, Clone, TerminusDBModel)]
struct Person {
    id: String,
    name: String,
}

#[derive(Debug, Clone, TerminusDBModel)]
struct Product {
    id: String,
    title: String,
    price: f64,
}

#[derive(Debug, Clone, TerminusDBModel)]
struct Order {
    id: String,
    order_number: String,
}

// Test struct with two generic parameters
#[derive(Debug, Clone, TerminusDBModel)]
struct Pair<T1: Model, T2: Model> {
    id: String,
    first: EntityIDFor<T1>,
    second: EntityIDFor<T2>,
}

// Test struct with multiple fields of different generic types
#[derive(Debug, Clone, TerminusDBModel)]
struct Mapping<K: Model, V: Model> {
    id: String,
    key: EntityIDFor<K>,
    value: EntityIDFor<V>,
    description: String,
}

// Test struct with three generic parameters
#[derive(Debug, Clone, TerminusDBModel)]
struct Triple<A: Model, B: Model, C: Model> {
    id: String,
    a_ref: EntityIDFor<A>,
    b_ref: EntityIDFor<B>,
    c_ref: EntityIDFor<C>,
}

// Test mixed generic usage
#[derive(Debug, Clone, TerminusDBModel)]
struct MixedContainer<T1: Model, T2: Model> {
    id: String,
    single_ref: EntityIDFor<T1>,
    multiple_refs: Vec<EntityIDFor<T2>>,
    metadata: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pair_with_person_and_product() {
        let pair = Pair::<Person, Product> {
            id: "pair-1".to_string(),
            first: EntityIDFor::new("Person/person-123").unwrap(),
            second: EntityIDFor::new("Product/product-456").unwrap(),
        };

        // Check schema class name includes both type parameters
        assert_eq!(Pair::<Person, Product>::to_class(), "Pair<Person, Product>");

        let instance = pair.to_instance(None);

        // Use to_json() instead of serde_json::to_value()
        let json = instance.to_json();
        assert_eq!(json["@type"], "Pair<Person, Product>");
        assert_eq!(json["first"], "Person/person-123");
        assert_eq!(json["second"], "Product/product-456");
    }

    #[test]
    fn test_mapping_with_product_and_order() {
        let mapping = Mapping::<Product, Order> {
            id: "mapping-1".to_string(),
            key: EntityIDFor::new("Product/prod-789").unwrap(),
            value: EntityIDFor::new("Order/order-123").unwrap(),
            description: "Maps product to order".to_string(),
        };

        assert_eq!(
            Mapping::<Product, Order>::to_class(),
            "Mapping<Product, Order>"
        );

        let instance = mapping.to_instance(None);
        // Verify instance type via JSON
        let json = instance.to_json();
        assert_eq!(json["@type"], "Mapping<Product, Order>");
    }

    #[test]
    fn test_triple_with_three_types() {
        let triple = Triple::<Person, Product, Order> {
            id: "triple-1".to_string(),
            a_ref: EntityIDFor::new("Person/person-1").unwrap(),
            b_ref: EntityIDFor::new("Product/product-2").unwrap(),
            c_ref: EntityIDFor::new("Order/order-3").unwrap(),
        };

        assert_eq!(
            Triple::<Person, Product, Order>::to_class(),
            "Triple<Person, Product, Order>"
        );

        let instance = triple.to_instance(None);
        // Verify instance type via JSON
        let json = instance.to_json();
        assert_eq!(json["@type"], "Triple<Person, Product, Order>");
    }

    #[test]
    fn test_mixed_container() {
        let container = MixedContainer::<Person, Product> {
            id: "mixed-1".to_string(),
            single_ref: EntityIDFor::new("Person/person-abc").unwrap(),
            multiple_refs: vec![
                EntityIDFor::new("Product/prod-1").unwrap(),
                EntityIDFor::new("Product/prod-2").unwrap(),
            ],
            metadata: "Mixed references".to_string(),
        };

        assert_eq!(
            MixedContainer::<Person, Product>::to_class(),
            "MixedContainer<Person, Product>"
        );

        let instance = container.to_instance(None);
        let json = instance.to_json();
        assert_eq!(json["@type"], "MixedContainer<Person, Product>");
        assert_eq!(json["single_ref"], "Person/person-abc");
        assert_eq!(json["multiple_refs"][0], "Product/prod-1");
        assert_eq!(json["multiple_refs"][1], "Product/prod-2");
    }

    #[test]
    fn test_different_parameter_order() {
        // Verify that Pair<Person, Product> is different from Pair<Product, Person>
        assert_ne!(
            Pair::<Person, Product>::to_class(),
            Pair::<Product, Person>::to_class()
        );

        assert_eq!(Pair::<Person, Product>::to_class(), "Pair<Person, Product>");
        assert_eq!(Pair::<Product, Person>::to_class(), "Pair<Product, Person>");
    }

    #[test]
    fn test_schema_generation_with_multiple_params() {
        let schemas = Triple::<Person, Product, Order>::to_schema_tree();

        // Should have schemas for Triple<Person, Product, Order>, Person, Product, and Order
        assert!(schemas.len() >= 4);

        // Find the Triple schema by checking the schema type
        let triple_schema = schemas
            .iter()
            .find(|s| {
                if let Schema::Class { id, .. } = s {
                    id == "Triple<Person, Product, Order>"
                } else {
                    false
                }
            })
            .expect("Should have Triple schema");

        match triple_schema {
            Schema::Class { properties, .. } => {
                assert_eq!(properties.len(), 4); // id, a_ref, b_ref, c_ref
                assert!(properties.iter().any(|p| p.name == "a_ref"));
                assert!(properties.iter().any(|p| p.name == "b_ref"));
                assert!(properties.iter().any(|p| p.name == "c_ref"));
            }
            _ => panic!("Expected Class schema"),
        }
    }
}
