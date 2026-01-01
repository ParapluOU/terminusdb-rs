#![cfg(feature = "generic-derive")]

use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use terminusdb_schema::{TerminusDBField, ToTDBInstance, ToTDBSchema};
use terminusdb_schema_derive::TerminusDBModel;

// Define concrete types that are TerminusDBModel models
#[derive(Debug, Clone, TerminusDBModel)]
struct Product {
    id: String,
    name: String,
    price: f64,
}

#[derive(Debug, Clone, TerminusDBModel)]
struct Customer {
    id: String,
    name: String,
    email: String,
}

// Generic container that works with TerminusDBModel types
// Note: When T is used directly as a field (not wrapped in Option, Vec, etc.),
// it needs all the TerminusDBField traits which are added by the derive macro
#[derive(Debug, Clone, TerminusDBModel)]
struct GenericContainer<T>
where
    T: TerminusDBField<GenericContainer<T>>,
{
    id: String,
    value: T,
    metadata: String,
}

#[test]
fn test_generic_container_with_models() {
    // Create a container holding a Product
    let product_container = GenericContainer::<Product> {
        id: "container-1".to_string(),
        value: Product {
            id: "prod-1".to_string(),
            name: "Widget".to_string(),
            price: 19.99,
        },
        metadata: "Product container".to_string(),
    };

    // Verify we can get the schema
    let schema = <GenericContainer<Product> as ToTDBSchema>::to_schema();
    assert_eq!(schema.class_name(), "GenericContainer<Product>");

    // Create a container holding a Customer
    let customer_container = GenericContainer::<Customer> {
        id: "container-2".to_string(),
        value: Customer {
            id: "cust-1".to_string(),
            name: "John Doe".to_string(),
            email: "john@example.com".to_string(),
        },
        metadata: "Customer container".to_string(),
    };

    let customer_schema = <GenericContainer<Customer> as ToTDBSchema>::to_schema();
    assert_eq!(customer_schema.class_name(), "GenericContainer<Customer>");

    // Verify we can convert to instances
    let _product_instance = product_container.to_instance(None);
    let _customer_instance = customer_container.to_instance(None);

    println!("✅ Generic container successfully works with TerminusDBModel types");
}
