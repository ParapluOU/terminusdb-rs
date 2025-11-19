use anyhow::{Context, Result};
use std::time::Duration;
use terminusdb_client::TerminusDBHttpClient;
use url::Url;

use crate::models::{NodeConfig, NodeStatus, RemoteInfo};
use crate::state::AppState;

/// Spawn a background task that polls all nodes for their status
pub fn spawn_poller(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));

        loop {
            interval.tick().await;

            // Poll all nodes
            let nodes = state.get_nodes();
            for node in nodes {
                let status = poll_node(&node).await;
                state.update_status(status);
            }
        }
    })
}

/// Poll a single node for its status
async fn poll_node(node: &NodeConfig) -> NodeStatus {
    match try_poll_node(node).await {
        Ok(status) => status,
        Err(e) => {
            tracing::debug!("Failed to poll node {}: {}", node.id, e);
            NodeStatus::offline(node.id.clone(), e.to_string())
        }
    }
}

/// Attempt to poll a node, returning an error if unreachable
async fn try_poll_node(node: &NodeConfig) -> Result<NodeStatus> {
    // Create client for this node
    let url = Url::parse(&node.base_url())
        .context("Invalid node URL")?;

    let client = TerminusDBHttpClient::new(
        url,
        &node.username,
        &node.password,
        "admin",
    ).await?;

    // Check if online by calling info endpoint
    client.info().await
        .context("Failed to get instance info")?;

    // Get database list
    let databases = client.list_databases_simple().await
        .context("Failed to list databases")?;

    let database_count = databases.len();

    // TODO: Query remote configurations for each database
    // For now, return empty remotes list
    let remotes: Vec<RemoteInfo> = Vec::new();

    Ok(NodeStatus::online(node.id.clone(), database_count, remotes))
}
