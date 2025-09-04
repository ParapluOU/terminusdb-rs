//! This example demonstrates generic type support in TerminusDBModel derive macro
//! Run with: cargo run --example generic_model_demo --features terminusdb-schema-derive/generic-derive

use terminusdb_schema::{EntityIDFor, ToTDBSchema};
use terminusdb_schema_derive::TerminusDBModel;

// Define concrete models that implement all required traits
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

// Generic model with EntityIDFor<T> - exactly what was requested!
#[derive(Debug, Clone, TerminusDBModel)]
struct Reference<T> {
    id: String,
    referenced_id: EntityIDFor<T>,  // References another entity of type T
    description: String,
}

fn main() {
    println!("=== Generic TerminusDBModel Demo ===\n");
    
    // Create a reference to a User
    let user_ref = Reference::<User> {
        id: "ref-001".to_string(),
        referenced_id: EntityIDFor::new("user-123").unwrap(),
        description: "Primary user reference".to_string(),
    };
    
    println!("Created user reference: {:?}", user_ref);
    
    // Create a reference to a Product  
    let product_ref = Reference::<Product> {
        id: "ref-002".to_string(),
        referenced_id: EntityIDFor::new("product-456").unwrap(),
        description: "Featured product reference".to_string(),
    };
    
    println!("Created product reference: {:?}", product_ref);
    
    // Get schema for Reference<User>
    let user_ref_schema = <Reference<User> as ToTDBSchema>::to_schema();
    println!("\nSchema for Reference<User>:");
    println!("  Class name: {}", user_ref_schema.class_name());
    
    // Get schema for Reference<Product> 
    let product_ref_schema = <Reference<Product> as ToTDBSchema>::to_schema();
    println!("\nSchema for Reference<Product>:");
    println!("  Class name: {}", product_ref_schema.class_name());
    
    println!("\n✅ Generic derive macro successfully works with Model<T> {{ EntityIDFor<T> }}!");
}