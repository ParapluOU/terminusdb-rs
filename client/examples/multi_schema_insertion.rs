//! Example demonstrating multiple schema insertion approaches
//! 
//! This example shows two convenient ways to insert multiple schemas at once:
//! 1. Using the `schemas!` macro for flexible syntax
//! 2. Using tuple types with `insert_schemas()` for type safety

use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Person {
    id: EntityIDFor<Self>,
    name: String,
    age: i32,
    email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Company {
    id: EntityIDFor<Self>,
    name: String,
    founded: i32,
    employees: Vec<Person>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Product {
    id: EntityIDFor<Self>,
    name: String,
    price: f64,
    manufacturer: Company,
}

#[derive(Debug, Clone, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Order {
    id: EntityIDFor<Self>,
    customer: Person,
    products: Vec<Product>,
    total: f64,
    order_date: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create client
    let client = TerminusDBHttpClient::local_node().await;
    let spec = BranchSpec::from("multi_schema_example");
    let args = DocumentInsertArgs::from(spec.clone());
    
    println!("=== Multi-Schema Insertion Examples ===\n");
    
    // Example 1: Using the schemas! macro (flexible, works with any number of types)
    println!("1. Using schemas! macro:");
    println!("   - Flexible syntax, supports any number of types");
    println!("   - Works well with dynamic type lists");
    println!("   - Generated code: vec![Type1::to_schema(), Type2::to_schema(), ...]");
    
    let schema_vec = schemas!(Person, Company, Product, Order);
    println!("   Generated {} schemas using macro", schema_vec.len());
    
    client.insert_documents(schema_vec.iter().collect(), args.clone().as_schema()).await?;
    println!("   ✓ Successfully inserted schemas using macro\n");
    
    // Example 2: Using tuple types with insert_schemas() (type-safe, up to 8 types)
    println!("2. Using insert_schemas() with tuple types:");
    println!("   - Type-safe at compile time");
    println!("   - More concise syntax");
    println!("   - Limited to 8 types max (can be extended)");
    
    client.insert_schemas::<(Person, Company, Product, Order)>(args.clone()).await?;
    println!("   ✓ Successfully inserted schemas using tuple approach\n");
    
    // Example 3: Smaller tuple example
    println!("3. Smaller tuple example (just 2 types):");
    client.insert_schemas::<(Person, Company)>(args.clone()).await?;
    println!("   ✓ Successfully inserted 2 schemas using tuple\n");
    
    // Example 4: Single type tuple
    println!("4. Single type tuple:");
    client.insert_schemas::<(Product,)>(args.clone()).await?;
    println!("   ✓ Successfully inserted 1 schema using tuple\n");
    
    // Example 5: Using macro with trailing comma
    println!("5. Macro with trailing comma (valid syntax):");
    let schemas_with_comma = schemas!(Person, Company, Product,);
    println!("   Generated {} schemas with trailing comma", schemas_with_comma.len());
    
    // Example 6: Empty schemas (edge case)
    println!("\n6. Empty schemas (edge case):");
    let empty_schemas: Vec<Schema> = schemas!();
    println!("   Empty macro returned {} schemas", empty_schemas.len());
    
    println!("\n=== Summary ===");
    println!("Both approaches provide convenient ways to insert multiple schemas:");
    println!("• schemas!(Type1, Type2, ...) - Flexible macro approach");
    println!("• insert_schemas::<(Type1, Type2, ...)>() - Type-safe tuple approach");
    println!("Choose based on your needs: flexibility vs type safety!");
    
    Ok(())
}