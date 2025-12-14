use terminusdb_woql_builder::prelude::*;
use terminusdb_schema::{ToJson, ToTDBInstance};

fn main() {
    // Create a simple triple query using the builder
    let query = WoqlBuilder::new()
        .triple(
            vars!("Subject"),
            "v:Predicate",
            vars!("Object")
        )
        .finalize();

    let json = query.to_json();
    println!("Rust-generated JSON-LD:");
    println!("{}", serde_json::to_string_pretty(&json).unwrap());
}
