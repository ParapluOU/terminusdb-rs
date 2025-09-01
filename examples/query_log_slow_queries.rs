//! Example showing how to use the query logger to find slow queries

use anyhow::Result;
use std::time::Duration;
use terminusdb_client::{TerminusDBHttpClient, debug::OperationFilter};

#[tokio::main]
async fn main() -> Result<()> {
    // Create client
    let client = TerminusDBHttpClient::local_node();
    
    // Enable query logging
    let log_path = "/tmp/terminusdb_queries.log";
    client.enable_query_log(log_path).await?;
    println!("Query logging enabled at: {}", log_path);
    
    // ... run your application and accumulate query logs ...
    
    // Retrieve slow queries with default threshold (1 second)
    println!("\n=== Slow queries (>= 1 second) ===");
    let slow_queries = client.get_slow_queries(None, None, None).await?;
    
    for (i, entry) in slow_queries.iter().enumerate() {
        println!("\n{}. [{}ms] {} - {}", 
            i + 1, 
            entry.duration_ms,
            entry.operation_type,
            entry.endpoint
        );
        
        if let Some(db) = &entry.database {
            println!("   Database: {}", db);
        }
        
        if !entry.success {
            println!("   Status: FAILED");
            if let Some(err) = &entry.error {
                println!("   Error: {}", err);
            }
        }
    }
    
    // Get slow queries with custom threshold (500ms)
    println!("\n=== Queries slower than 500ms ===");
    let slow_queries = client.get_slow_queries(
        Some(Duration::from_millis(500)),
        None,
        None,
    ).await?;
    println!("Found {} slow queries", slow_queries.len());
    
    // Get only slow WOQL queries (not inserts/updates)
    println!("\n=== Slow WOQL queries only ===");
    let slow_woql = client.get_slow_queries(
        Some(Duration::from_millis(500)),
        Some(OperationFilter::QueriesOnly),
        None,
    ).await?;
    println!("Found {} slow WOQL queries", slow_woql.len());
    
    // Get top 5 slowest operations
    println!("\n=== Top 5 slowest operations ===");
    let top_5 = client.get_slow_queries(
        Some(Duration::from_millis(1)), // Very low threshold to get all
        Some(OperationFilter::All),
        Some(5),
    ).await?;
    
    for (i, entry) in top_5.iter().enumerate() {
        println!("{}. {}ms - {}", i + 1, entry.duration_ms, entry.operation_type);
    }
    
    Ok(())
}