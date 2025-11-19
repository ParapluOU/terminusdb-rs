use anyhow::{Context, Result};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use terminusdb_client::TerminusDBHttpClient;

use crate::manager::TerminusDBManager;
use crate::models::{NodeConfig, NodeStatus};

const META_DATABASE: &str = "_meta/manager_config";

/// Application state shared across Rocket handlers
#[derive(Clone)]
pub struct AppState {
    /// Local TerminusDB instance manager
    manager: TerminusDBManager,

    /// Cached node configurations
    nodes: Arc<RwLock<HashMap<String, NodeConfig>>>,

    /// Cached node statuses (updated by poller)
    statuses: Arc<RwLock<HashMap<String, NodeStatus>>>,
}

impl AppState {
    /// Create new application state and initialize
    pub async fn new() -> Result<Self> {
        let manager = TerminusDBManager::new().await
            .context("Failed to start local TerminusDB instance")?;

        let state = Self {
            manager,
            nodes: Arc::new(RwLock::new(HashMap::new())),
            statuses: Arc::new(RwLock::new(HashMap::new())),
        };

        // Initialize storage and load nodes
        state.initialize().await?;

        Ok(state)
    }

    /// Initialize the metadata database and load node configurations
    async fn initialize(&self) -> Result<()> {
        let _client = self.manager.client().await?;

        // Ensure the metadata database exists
        if !Self::database_exists(&_client, META_DATABASE).await? {
            tracing::info!("Creating metadata database: {}", META_DATABASE);
            _client.ensure_database(META_DATABASE).await
                .context("Failed to create metadata database")?;

            // Add the default localhost node
            let localhost = NodeConfig::localhost(self.manager.port());
            self.save_node(&localhost).await?;
        }

        // Load all node configurations
        self.load_nodes().await?;

        Ok(())
    }

    /// Check if a database exists
    async fn database_exists(client: &TerminusDBHttpClient, db_path: &str) -> Result<bool> {
        match client.list_databases_simple().await {
            Ok(databases) => {
                Ok(databases.iter().any(|db| {
                    db.path.as_ref().map(|p| p == db_path).unwrap_or(false)
                }))
            }
            Err(e) => {
                tracing::warn!("Failed to list databases: {}", e);
                Ok(false)
            }
        }
    }

    /// Load all node configurations from the metadata database
    async fn load_nodes(&self) -> Result<()> {
        let client = self.manager.client().await?;

        // TODO: Use client.get_all_instances::<NodeConfig>() once available
        // For now, we'll just load the localhost node

        let localhost = NodeConfig::localhost(self.manager.port());
        self.nodes.write().insert(localhost.id.clone(), localhost);

        tracing::info!("Loaded {} node configuration(s)", self.nodes.read().len());
        Ok(())
    }

    /// Save a node configuration to the metadata database
    pub async fn save_node(&self, node: &NodeConfig) -> Result<()> {
        let _client = self.manager.client().await?;

        // TODO: Use _client.insert_instance() to persist the node
        // For now, just cache it in memory

        self.nodes.write().insert(node.id.clone(), node.clone());
        tracing::info!("Saved node configuration: {}", node.id);

        Ok(())
    }

    /// Get all node configurations
    pub fn get_nodes(&self) -> Vec<NodeConfig> {
        self.nodes.read().values().cloned().collect()
    }

    /// Get a specific node configuration
    pub fn get_node(&self, id: &str) -> Option<NodeConfig> {
        self.nodes.read().get(id).cloned()
    }

    /// Delete a node configuration
    pub async fn delete_node(&self, id: &str) -> Result<()> {
        // Don't allow deleting the local node
        if id == "local" {
            anyhow::bail!("Cannot delete the local instance node");
        }

        self.nodes.write().remove(id);
        self.statuses.write().remove(id);

        // TODO: Delete from database once persistence is implemented

        tracing::info!("Deleted node configuration: {}", id);
        Ok(())
    }

    /// Update a node configuration
    pub async fn update_node(&self, node: NodeConfig) -> Result<()> {
        self.save_node(&node).await
    }

    /// Get all node statuses
    pub fn get_statuses(&self) -> Vec<NodeStatus> {
        self.statuses.read().values().cloned().collect()
    }

    /// Get a specific node status
    pub fn get_status(&self, id: &str) -> Option<NodeStatus> {
        self.statuses.read().get(id).cloned()
    }

    /// Update a node's status (called by poller)
    pub fn update_status(&self, status: NodeStatus) {
        self.statuses.write().insert(status.node_id.clone(), status);
    }

    /// Get the local manager
    pub fn manager(&self) -> &TerminusDBManager {
        &self.manager
    }
}
