use terminusdb_client::{BranchSpec, TerminusDBHttpClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = TerminusDBHttpClient::local_node().await;
    let spec = BranchSpec::with_branch("manager_config", "main");
    let count = client.commits_count(&spec).await?;
    println!("Database has {} commits", count);
    Ok(())
}
