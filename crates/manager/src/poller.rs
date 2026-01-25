use anyhow::{Context, Result};
use std::time::Duration;
use terminusdb_types::DatabasePath;

use crate::models::{DatabaseInfo, NodeConfig, NodeStatus, RemoteInfo};
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
                let status = poll_node(&state, &node).await;
                state.update_status(status);
            }
        }
    })
}

/// Spawn a per-node background poller with parallel database querying
/// Each node gets its own independent polling loop
pub fn spawn_node_poller(state: AppState, node_id: String) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));

        loop {
            interval.tick().await;

            // Get the node config
            let node = match state.get_nodes().into_iter().find(|n| n.id == node_id) {
                Some(n) => n,
                None => {
                    tracing::warn!("Node {} not found, stopping poller", node_id);
                    break;
                }
            };

            // Poll this specific node with parallel database queries
            let status = poll_node_parallel(&state, &node).await;
            state.update_status(status);
        }
    })
}

/// Poll a single node for its status
async fn poll_node(state: &AppState, node: &NodeConfig) -> NodeStatus {
    match try_poll_node(state, node).await {
        Ok(status) => status,
        Err(e) => {
            tracing::debug!("Failed to poll node {}: {}", node.id, e);
            NodeStatus::offline(node.id.clone(), e.to_string())
        }
    }
}

/// Poll a single node with parallel database queries for efficiency
async fn poll_node_parallel(state: &AppState, node: &NodeConfig) -> NodeStatus {
    match try_poll_node_parallel(state, node).await {
        Ok(status) => status,
        Err(e) => {
            tracing::debug!("Failed to poll node {}: {}", node.id, e);
            NodeStatus::offline(node.id.clone(), e.to_string())
        }
    }
}

/// Attempt to poll a node, returning an error if unreachable
async fn try_poll_node(state: &AppState, node: &NodeConfig) -> Result<NodeStatus> {
    // Get or create cached client for this node (reuses connections)
    let client = state.get_or_create_client(&node.id).await?;

    // Check if online by calling info endpoint
    match client.info().await {
        Ok(_) => {
            // Node is reachable, now try to list databases
            match client.list_databases_simple().await {
                Ok(databases) => {
                    let database_count = databases.len();

                    // Query remote configurations for each database
                    let mut remotes: Vec<RemoteInfo> = Vec::new();
                    let mut db_infos: Vec<DatabaseInfo> = Vec::new();

                    for db in &databases {
                        // Get the database path string, preferring path over name
                        let db_path_str = db
                            .path
                            .as_ref()
                            .or(db.name.as_ref())
                            .map(|s| s.as_str())
                            .unwrap_or("");

                        if db_path_str.is_empty() {
                            continue; // Skip databases without a name
                        }

                        // Parse into DatabasePath for type-safe handling
                        let db_path = match DatabasePath::parse(db_path_str) {
                            Ok(path) => path,
                            Err(e) => {
                                tracing::debug!(
                                    "Skipping invalid/system database path '{}': {}",
                                    db_path_str,
                                    e
                                );
                                continue;
                            }
                        };

                        // Skip system databases (those starting with underscore)
                        if db_path.is_system_database() {
                            continue;
                        }

                        // Get just the database name (without org prefix) for API calls
                        let db_name_only = db_path.database_name().to_string();
                        let path = db_name_only.clone();

                        // Create basic DatabaseInfo (detailed metadata will be collected in Phase 3)
                        db_infos.push(DatabaseInfo {
                            name: db_name_only.clone(),
                            commit_count: 0, // Will be populated by per-node poller in Phase 3
                            last_modified: "Unknown".to_string(), // Will be populated in Phase 3
                            remote_count: 0, // Will be populated in Phase 3
                        });

                        // Try to list remotes for this database
                        if let Ok(db_remotes) = client.list_remotes(&path).await {
                            for remote in db_remotes {
                                // Try to match remote URL to a known node
                                let target_node_id = state
                                    .get_nodes()
                                    .iter()
                                    .find(|n| remote.remote_url.contains(&n.host))
                                    .map(|n| n.id.clone());

                                remotes.push(RemoteInfo {
                                    database: db_name_only.clone(),
                                    remote_name: remote.name.clone(),
                                    remote_url: remote.remote_url.clone(),
                                    target_node_id,
                                });
                            }
                        }
                    }

                    Ok(NodeStatus::online(
                        node.id.clone(),
                        database_count,
                        db_infos,
                        remotes,
                    ))
                }
                Err(_) => {
                    // Can reach node but cannot list databases (auth issue?)
                    Ok(NodeStatus::reachable(node.id.clone()))
                }
            }
        }
        Err(e) => {
            // Node is unreachable
            Err(e).context("Failed to get instance info")
        }
    }
}

/// Attempt to poll a node with parallel database metadata queries
async fn try_poll_node_parallel(state: &AppState, node: &NodeConfig) -> Result<NodeStatus> {
    use futures::stream::{self, StreamExt};

    // Get or create cached client for this node (reuses connections)
    let client = state.get_or_create_client(&node.id).await?;

    // Check if online by calling info endpoint
    match client.info().await {
        Ok(_) => {
            // Node is reachable, now try to list databases
            match client.list_databases_simple().await {
                Ok(databases) => {
                    let database_count = databases.len();

                    // Process all databases in parallel
                    let db_results: Vec<_> = stream::iter(databases.into_iter())
                        .map(|db| {
                            let client = client.clone();
                            let state = state.clone();

                            async move {
                                // Get the database path string, preferring path over name
                                let db_path_str = db
                                    .path
                                    .as_ref()
                                    .or(db.name.as_ref())
                                    .map(|s| s.as_str())
                                    .unwrap_or("");

                                if db_path_str.is_empty() {
                                    return None;
                                }

                                // Parse into DatabasePath for type-safe handling
                                let db_path = match DatabasePath::parse(db_path_str) {
                                    Ok(path) => path,
                                    Err(e) => {
                                        tracing::debug!(
                                            "Skipping invalid/system database path '{}': {}",
                                            db_path_str,
                                            e
                                        );
                                        return None;
                                    }
                                };

                                // Skip system databases (those starting with underscore)
                                if db_path.is_system_database() {
                                    return None;
                                }

                                // Get just the database name (without org prefix) for API calls
                                let db_name_only = db_path.database_name().to_string();

                                // Create BranchSpec for queries
                                let spec = terminusdb_client::BranchSpec {
                                    db: db_name_only.clone(),
                                    branch: None,
                                    ref_commit: None,
                                };

                                // Query metadata in parallel: commit count and remotes
                                let (commit_count_result, remotes_result) = tokio::join!(
                                    client.commits_count(&spec),
                                    client.list_remotes(&db_name_only)
                                );

                                // Process commit count (more efficient than fetching all commits)
                                let commit_count = commit_count_result.unwrap_or(0);

                                // For last_modified, we still need to fetch the most recent commit
                                // Use log with count=1 to get just the latest commit
                                let log_opts = terminusdb_client::LogOpts {
                                    offset: None,
                                    count: Some(1), // Only fetch the most recent commit
                                    verbose: false,
                                };
                                let last_modified = match client.log(&spec, log_opts).await {
                                    Ok(commits) => commits
                                        .first()
                                        .map(|c| c.timestamp.to_string())
                                        .unwrap_or_else(|| "Unknown".to_string()),
                                    Err(_) => "Unknown".to_string(),
                                };

                                // Process remotes
                                let mut db_remotes = Vec::new();
                                let remote_count = if let Ok(remotes) = remotes_result {
                                    let count = remotes.len();
                                    for remote in remotes {
                                        // Try to match remote URL to a known node
                                        let target_node_id = state
                                            .get_nodes()
                                            .iter()
                                            .find(|n| remote.remote_url.contains(&n.host))
                                            .map(|n| n.id.clone());

                                        db_remotes.push(RemoteInfo {
                                            database: db_name_only.clone(),
                                            remote_name: remote.name.clone(),
                                            remote_url: remote.remote_url.clone(),
                                            target_node_id,
                                        });
                                    }
                                    count
                                } else {
                                    0
                                };

                                Some((
                                    DatabaseInfo {
                                        name: db_name_only,
                                        commit_count,
                                        last_modified,
                                        remote_count,
                                    },
                                    db_remotes,
                                ))
                            }
                        })
                        .buffer_unordered(10) // Process up to 10 databases in parallel
                        .collect()
                        .await;

                    // Separate databases and remotes
                    let mut db_infos = Vec::new();
                    let mut all_remotes = Vec::new();

                    for result in db_results.into_iter().flatten() {
                        db_infos.push(result.0);
                        all_remotes.extend(result.1);
                    }

                    Ok(NodeStatus::online(
                        node.id.clone(),
                        database_count,
                        db_infos,
                        all_remotes,
                    ))
                }
                Err(_) => {
                    // Can reach node but cannot list databases (auth issue?)
                    Ok(NodeStatus::reachable(node.id.clone()))
                }
            }
        }
        Err(e) => {
            // Node is unreachable
            Err(e).context("Failed to get instance info")
        }
    }
}
