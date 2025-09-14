#![cfg(feature = "generic-derive")]

// This test demonstrates that generic derive works with EntityIDFor<T>
// as requested by the user: "ideally i want Model<T> {other: EntityIDFor<T>}"

use terminusdb_schema::{EntityIDFor, ToTDBSchema};
use terminusdb_schema_derive::TerminusDBModel;

// Define concrete types that implement all required traits
#[derive(Debug, Clone, TerminusDBModel)]
struct Person {
    id: String,
    name: String,
}

#[derive(Debug, Clone, TerminusDBModel)]
struct Company {
    id: String,
    name: String,
}

// Generic model with EntityIDFor<T> field
#[derive(Debug, Clone, TerminusDBModel)]
struct Employment<T> {
    id: String,
    employee_ref: EntityIDFor<T>,  // This is what the user wanted
    position: String,
}

#[test]
fn test_generic_model_with_entity_id_for() {
    // Create an employment relationship that references a Person
    let person_employment = Employment::<Person> {
        id: "emp-001".to_string(),
        employee_ref: EntityIDFor::new("person-123").unwrap(),
        position: "Software Engineer".to_string(),
    };
    
    // Verify we can get the schema - this proves the derive macro works
    let schema = <Employment<Person> as ToTDBSchema>::to_schema();
    assert_eq!(schema.class_name(), "Employment");
    
    // The same generic struct works with Company type
    let company_employment = Employment::<Company> {
        id: "emp-002".to_string(),
        employee_ref: EntityIDFor::new("company-456").unwrap(),
        position: "Contractor".to_string(),
    };
    
    let company_schema = <Employment<Company> as ToTDBSchema>::to_schema();
    assert_eq!(company_schema.class_name(), "Employment");
    
    println!("✅ Generic derive macro successfully generates code for Model<T> {{ EntityIDFor<T> }}");
}