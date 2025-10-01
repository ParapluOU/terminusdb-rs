//! Example demonstrating GraphQL introspection with TerminusDB

use terminusdb_client::http::{GraphQLRequest, TerminusDBHttpClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("TerminusDB GraphQL Introspection Example");
    println!("========================================\n");

    // Create client connected to local TerminusDB
    let client = TerminusDBHttpClient::local_node().await;
    
    // Database name to introspect (adjust as needed)
    let database = "admin";  // Using the default admin database
    
    println!("Fetching GraphQL schema for database: {}", database);
    
    // Method 1: Using the convenience introspect_schema method
    match client.introspect_schema(database, None).await {
        Ok(schema) => {
            println!("\n✅ Schema introspection successful!");
            
            // Pretty print a preview of the schema
            let pretty = serde_json::to_string_pretty(&schema)?;
            let preview_len = pretty.len().min(1000);
            println!("\nSchema preview (first {} chars):", preview_len);
            println!("{}", &pretty[..preview_len]);
            if pretty.len() > preview_len {
                println!("... (truncated, full schema is {} bytes)", pretty.len());
            }
        }
        Err(e) => {
            eprintln!("\n❌ Error during introspection: {}", e);
            eprintln!("Make sure TerminusDB is running and the database exists.");
        }
    }
    
    println!("\n----------------------------------------\n");
    
    // Method 2: Using a custom GraphQL query
    let custom_query = r#"
        query {
            __schema {
                queryType {
                    name
                    fields(includeDeprecated: true) {
                        name
                        description
                    }
                }
            }
        }
    "#;
    
    println!("Running custom GraphQL query...");
    
    let request = GraphQLRequest::new(custom_query);
    match client.execute_graphql::<serde_json::Value>(database, None, request).await {
        Ok(response) => {
            if let Some(data) = response.data {
                println!("\n✅ Custom query successful!");
                println!("Query type info:");
                println!("{}", serde_json::to_string_pretty(&data)?);
            }
            
            if let Some(errors) = response.errors {
                println!("\n⚠️  GraphQL errors:");
                for error in errors {
                    println!("  - {}", error.message);
                }
            }
        }
        Err(e) => {
            eprintln!("\n❌ Error executing custom query: {}", e);
        }
    }
    
    Ok(())
}