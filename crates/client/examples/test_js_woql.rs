use terminusdb_client::{TerminusDBHttpClient, BranchSpec};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = TerminusDBHttpClient::local_node().await;
    let spec = BranchSpec::with_branch("manager_config", "main");

    // Test JavaScript WOQL syntax - simple count query
    let js_query = r#"count("v:Count", triple("v:Commit", "rdf:type", "@schema:ValidCommit"))"#;

    println!("Testing JavaScript WOQL syntax: {}", js_query);

    let result = client.query_string::<serde_json::Value>(Some(spec), js_query, None).await?;

    println!("Query succeeded!");
    println!("Result: {}", serde_json::to_string_pretty(&result)?);

    Ok(())
}
