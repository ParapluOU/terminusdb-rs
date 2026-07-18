//! Example demonstrating the debug logging functionality

use terminusdb_client::*;
use terminusdb_schema::ToTDBInstance;
use terminusdb_schema_derive::TerminusDBModel;

#[derive(Debug, Clone, TerminusDBModel)]
struct Person {
    name: String,
    age: i32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize the client
    let client = TerminusDBHttpClient::local_node().await;

    // Enable query logging to a file
    client
        .enable_query_log("/tmp/terminusdb_queries.log")
        .await?;
    println!("Query logging enabled to /tmp/terminusdb_queries.log");

    // Create a database
    let spec = BranchSpec::from("debug_example");
    // Ensure the database exists
    let client = match client.ensure_database("debug_example").await {
        Ok(client) => {
            println!("Using database: debug_example");
            client
        }
        Err(_) => {
            println!("Note: Database may already exist or creation failed");
            client
        }
    };

    // Insert some data
    let person = Person {
        name: "Alice".to_string(),
        age: 30,
    };

    let args = DocumentInsertArgs {
        spec: spec.clone(),
        ..Default::default()
    };

    let result = client.save_instance(&person, args).await?;
    println!("Inserted person with ID: {:?}", result);

    // Execute a query
    use terminusdb_woql2::prelude::*;
    let query = select!(
        [Name, Age],
        and!(
            triple!(var!(Person), "name", var!(Name)),
            triple!(var!(Person), "age", var!(Age)),
        )
    );

    let results: WOQLResult<serde_json::Value> = client.query(Some(spec), query).await?;
    println!("Query returned {} results", results.bindings.len());

    // Display the operation log
    println!("\n=== Recent Operations ===");
    let recent_ops = client.get_recent_operations(10);
    for op in recent_ops {
        println!(
            "{}: {} - {} ({}ms) {}",
            op.timestamp.format("%H:%M:%S"),
            op.operation_type,
            op.endpoint,
            op.duration_ms,
            if op.success { "✓" } else { "✗" }
        );
        if let Some(count) = op.result_count {
            println!("  Results: {}", count);
        }
        if let Some(error) = &op.error {
            println!("  Error: {}", error);
        }
    }

    // Rotate the query log
    client.rotate_query_log().await?;
    println!("\nQuery log rotated");

    // Disable query logging
    client.disable_query_log().await;
    println!("Query logging disabled");

    println!("\nCheck /tmp/terminusdb_queries.log for the query audit trail");

    Ok(())
}
