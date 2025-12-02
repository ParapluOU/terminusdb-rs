//! Simple test to verify server startup works

use terminusdb_bin::TerminusDBServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Starting test server...");

    let server = TerminusDBServer::test().await?;
    println!("Server started!");

    let client = server.client().await?;
    println!("Got client!");

    let info = client.info().await?;
    println!("Server info: {:?}", info);

    println!("Test passed!");
    Ok(())
}
