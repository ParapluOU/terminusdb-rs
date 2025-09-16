//! Example demonstrating the squash operation

use terminusdb_client::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create a client connected to the local TerminusDB instance
    let client = TerminusDBHttpClient::local_node().await;

    // Example of squashing commits on a branch
    println!("Squashing commits on main branch...");
    let response = client
        .squash(
            "admin/mydb/local/branch/main",
            "admin",
            "Squash all commits into one",
        )
        .await?;

    println!("Squash successful!");
    println!("New commit ID: {}", response.commit);
    println!("Old commit ID: {}", response.old_commit);
    println!("Status: {:?}", response.status);

    // You can also squash from a specific commit
    println!("\nSquashing from a specific commit...");
    let commit_path = format!("admin/mydb/local/commit/{}", response.commit);
    let response2 = client
        .squash(&commit_path, "admin", "Another squash from a commit")
        .await?;

    println!("Second squash successful!");
    println!("New commit ID: {}", response2.commit);

    Ok(())
}