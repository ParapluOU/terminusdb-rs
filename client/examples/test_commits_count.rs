use terminusdb_client::{TerminusDBHttpClient, BranchSpec};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = TerminusDBHttpClient::local_node().await;
    let spec = BranchSpec::with_branch("manager_config", "main");
    let count = client.commits_count(&spec).await?;
    println!("Database has {} commits", count);
    Ok(())
}
