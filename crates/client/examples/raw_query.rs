//! Example demonstrating the use of RawQueryable for custom WOQL queries
//!
//! This example shows how to write WOQL queries that return custom result types
//! instead of full TerminusDB model instances.

use serde::Deserialize;
use std::collections::HashMap;
use terminusdb_client::*;
use terminusdb_woql2::prelude::*;

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
        select!(
            [Name, Age, Street, City],
            and!(
                // Find all persons
                triple!(var!(Person), "rdf:type", "@schema:Person"),
                triple!(var!(Person), "@schema:name", var!(Name)),
                triple!(var!(Person), "@schema:age", var!(Age)),
                // Find their addresses
                triple!(var!(Person), "@schema:address", var!(Address)),
                triple!(var!(Address), "@schema:street", var!(Street)),
                triple!(var!(Address), "@schema:city", var!(City)),
            )
        )
    }

    fn extract_result(
        &self,
        mut binding: HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<Self::Result> {
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
fn extract_string(
    binding: &mut HashMap<String, serde_json::Value>,
    key: &str,
) -> anyhow::Result<String> {
    binding
        .remove(key)
        .and_then(|v| v.get("@value").cloned())
        .and_then(|v| serde_json::from_value::<String>(v).ok())
        .ok_or_else(|| anyhow::anyhow!("Missing {} field", key))
}

/// Helper to extract number values from bindings
fn extract_number(
    binding: &mut HashMap<String, serde_json::Value>,
    key: &str,
) -> anyhow::Result<f64> {
    binding
        .remove(key)
        .and_then(|v| v.get("@value").cloned())
        .and_then(|v| serde_json::from_value::<f64>(v).ok())
        .ok_or_else(|| anyhow::anyhow!("Missing {} field", key))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create client
    let client = TerminusDBHttpClient::local_node().await;
    let spec = BranchSpec::from("mydb");

    // Execute the custom query
    let results = client.execute_raw_query(&spec, PersonAddressQuery).await?;

    // Print results
    println!("People with addresses:");
    for person in results {
        println!(
            "  {} (age {}) lives at {} in {}",
            person.name, person.age, person.street, person.city
        );
    }

    // You can also use the RawWoqlQuery builder for simpler queries
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)] // deserialization target for an illustrative-only query
    struct CountResult {
        #[serde(rename = "Count")]
        count: i32,
    }

    let _count_query = RawWoqlQuery::<CountResult>::new(select!(
        [Count],
        count_into!(
            triple!(var!(Person), "rdf:type", "@schema:Person"),
            var!(Count)
        )
    ));

    // `_count_query` implements `RawQueryable`, so it could be executed with
    // `.count(&client, &spec)` / `.apply(&client, &spec)` against a live server.

    Ok(())
}
