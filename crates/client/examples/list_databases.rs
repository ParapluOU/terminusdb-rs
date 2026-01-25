//! Example demonstrating how to list databases in TerminusDB

use terminusdb_client::TerminusDBHttpClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Connect to local TerminusDB instance
    let client = TerminusDBHttpClient::local_node().await;

    println!("Listing databases (simple):");
    println!("==========================");

    // List databases with default options
    let databases = client.list_databases_simple().await?;

    for db in &databases {
        if let Some(path) = &db.path {
            println!("- {}", path);

            // Use the helper methods to extract parts
            if let Some(db_name) = db.database_name() {
                println!("  Database: {}", db_name);
            }
            if let Some(org) = db.organization() {
                println!("  Organization: {}", org);
            }
        }
    }

    println!("\nListing databases with branches:");
    println!("================================");

    // List databases with branch information
    let databases_with_branches = client.list_databases(true, false).await?;

    for db in &databases_with_branches {
        if let Some(path) = &db.path {
            print!("{}", path);

            if let Some(branches) = &db.branches {
                println!(" - branches: {}", branches.join(", "));
            } else {
                println!();
            }
        }
    }

    println!("\nListing databases (verbose):");
    println!("============================");

    // List databases with all available information
    let verbose_databases = client.list_databases(false, true).await?;

    for db in &verbose_databases {
        if let Some(path) = &db.path {
            println!("\n{}", path);

            if let Some(id) = &db.id {
                println!("  ID: {}", id);
            }
            if let Some(db_type) = &db.database_type {
                println!("  Type: {}", db_type);
            }
            if let Some(label) = &db.label {
                println!("  Label: {}", label);
            }
            if let Some(comment) = &db.comment {
                println!("  Comment: {}", comment);
            }
            if let Some(creation_date) = &db.creation_date {
                println!("  Created: {}", creation_date);
            }
            if let Some(state) = &db.state {
                println!("  State: {}", state);
            }
        }
    }

    Ok(())
}
