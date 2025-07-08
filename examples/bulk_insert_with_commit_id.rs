//! Example demonstrating bulk insert with commit ID retrieval

use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{TerminusDBModel, FromTDBInstance};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
struct Product {
    name: String,
    price: f64,
    in_stock: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Connect to local TerminusDB instance
    let client = TerminusDBHttpClient::local_node();
    let spec = BranchSpec::from("admin/mydb/main");
    
    // Insert product schema
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<Product>(args.clone()).await?;
    
    // Create multiple products
    let products = vec![
        Product {
            name: "Laptop".to_string(),
            price: 999.99,
            in_stock: true,
        },
        Product {
            name: "Mouse".to_string(),
            price: 29.99,
            in_stock: true,
        },
        Product {
            name: "Keyboard".to_string(),
            price: 79.99,
            in_stock: false,
        },
    ];
    
    // Insert all products and get the commit ID
    let (result, commit_id) = client.insert_instances_with_commit_id(products, args).await?;
    
    println!("Successfully inserted {} products in commit {}", result.len(), commit_id);
    
    // Print individual results
    for (id, insert_result) in result.iter() {
        match insert_result {
            TDBInsertInstanceResult::Inserted(instance_id) => {
                println!("  ✓ Inserted: {}", instance_id);
            }
            TDBInsertInstanceResult::AlreadyExists(instance_id) => {
                println!("  - Already exists: {}", instance_id);
            }
        }
    }
    
    // The commit_id can be used for:
    // 1. Auditing - knowing exactly when these instances were created
    // 2. Time travel - retrieving the database state at this specific commit
    // 3. Rollback - reverting to a state before this commit if needed
    
    println!("\nYou can use this commit ID for time travel queries:");
    println!("  let commit_spec = BranchSpec::from(\"admin/mydb/local/commit/{}\");", commit_id);
    
    Ok(())
}