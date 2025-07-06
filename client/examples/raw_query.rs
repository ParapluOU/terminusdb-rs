//! Example demonstrating the use of RawQueryable for custom WOQL queries
//! 
//! This example shows how to write WOQL queries that return custom result types
//! instead of full TerminusDB model instances.

use terminusdb_client::*;
use terminusdb_woql_builder::prelude::{vars, WoqlBuilder};
use terminusdb_woql2::prelude::Query;
use serde::Deserialize;
use std::collections::HashMap;

/// Custom result type for a join query
#[derive(Debug, Deserialize)]
struct PersonWithAddress {
    name: String,
    age: i32,
    street: String,
    city: String,
}

/// Query that joins Person and Address data
struct PersonAddressQuery;

impl RawQueryable for PersonAddressQuery {
    type Result = PersonWithAddress;
    
    fn query(&self) -> Query {
        WoqlBuilder::new()
            // Find all persons
            .triple(vars!("Person"), "rdf:type", "@schema:Person")
            .triple(vars!("Person"), "@schema:name", vars!("Name"))
            .triple(vars!("Person"), "@schema:age", vars!("Age"))
            // Find their addresses
            .triple(vars!("Person"), "@schema:address", vars!("Address"))
            .triple(vars!("Address"), "@schema:street", vars!("Street"))
            .triple(vars!("Address"), "@schema:city", vars!("City"))
            // Select the fields we want
            .select(vec![vars!("Name"), vars!("Age"), vars!("Street"), vars!("City")])
            .finalize()
    }
    
    fn extract_result(&self, mut binding: HashMap<String, serde_json::Value>) -> anyhow::Result<Self::Result> {
        // Extract each field from the binding
        let name = extract_string(&mut binding, "Name")?;
        let age = extract_number(&mut binding, "Age")? as i32;
        let street = extract_string(&mut binding, "Street")?;
        let city = extract_string(&mut binding, "City")?;
        
        Ok(PersonWithAddress {
            name,
            age,
            street,
            city,
        })
    }
}

/// Helper to extract string values from bindings
fn extract_string(binding: &mut HashMap<String, serde_json::Value>, key: &str) -> anyhow::Result<String> {
    binding.remove(key)
        .and_then(|v| v.get("@value").cloned())
        .and_then(|v| serde_json::from_value::<String>(v).ok())
        .ok_or_else(|| anyhow::anyhow!("Missing {} field", key))
}

/// Helper to extract number values from bindings
fn extract_number(binding: &mut HashMap<String, serde_json::Value>, key: &str) -> anyhow::Result<f64> {
    binding.remove(key)
        .and_then(|v| v.get("@value").cloned())
        .and_then(|v| serde_json::from_value::<f64>(v).ok())
        .ok_or_else(|| anyhow::anyhow!("Missing {} field", key))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create client
    let client = TerminusDBHttpClient::local_node().await?;
    let spec = BranchSpec::from("mydb");
    
    // Execute the custom query
    let results = client.execute_raw_query(&spec, PersonAddressQuery).await?;
    
    // Print results
    println!("People with addresses:");
    for person in results {
        println!("  {} (age {}) lives at {} in {}", 
                 person.name, person.age, person.street, person.city);
    }
    
    // You can also use the RawWoqlQuery builder for simpler queries
    #[derive(Debug, Deserialize)]
    struct CountResult {
        #[serde(rename = "Count")]
        count: i32,
    }
    
    let count_query = RawWoqlQuery::<CountResult>::new()
        .builder()
        .triple(vars!("Person"), "rdf:type", "@schema:Person")
        .count(vars!("Count"))
        .select(vec![vars!("Count")])
        .finalize();
    
    // This would need a custom implementation since RawWoqlQuery doesn't have query() returning the finalized query
    // For now, this is just an example of the intended API
    
    Ok(())
}