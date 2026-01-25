use anyhow::{Context, Result};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use terminusdb_client::{
    deserialize::DefaultTDBDeserializer, BranchSpec, DeleteOpts, DocumentInsertArgs, GetOpts,
    TerminusDBHttpClient,
};

use crate::manager::TerminusDBManager;
use crate::models::{NodeConfig, NodeStatus};

const META_DATABASE: &str = "manager_config";

/// Application state shared across Rocket handlers
#[derive(Clone)]
pub struct AppState {
    /// Local TerminusDB instance manager
    manager: TerminusDBManager,

    /// Cached node configurations
    nodes: Arc<RwLock<HashMap<String, NodeConfig>>>,

    /// Cached node statuses (updated by poller)
    statuses: Arc<RwLock<HashMap<String, NodeStatus>>>,

    /// Cached HTTP clients per node (to avoid creating new connections constantly)
    clients: Arc<RwLock<HashMap<String, Arc<TerminusDBHttpClient>>>>,

    /// Per-node poller task handles (for lifecycle management)
    poller_handles: Arc<RwLock<HashMap<String, tokio::task::JoinHandle<()>>>>,
}

impl AppState {
    /// Create new application state and initialize
    pub async fn new() -> Result<Self> {
        let manager = TerminusDBManager::new()
            .await
            .context("Failed to start local TerminusDB instance")?;

        let state = Self {
            manager,
            nodes: Arc::new(RwLock::new(HashMap::new())),
            statuses: Arc::new(RwLock::new(HashMap::new())),
            clients: Arc::new(RwLock::new(HashMap::new())),
            poller_handles: Arc::new(RwLock::new(HashMap::new())),
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
            _client
                .ensure_database(META_DATABASE)
                .await
                .context("Failed to create metadata database")?;

            // Add the schema for NodeConfig
            let spec = BranchSpec {
                db: META_DATABASE.to_string(),
                branch: Some("main".to_string()),
                ref_commit: None,
            };

            _client
                .insert_entity_schema::<NodeConfig>(spec.clone().into())
                .await
                .context("Failed to add NodeConfig schema")?;

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
            Ok(databases) => Ok(databases
                .iter()
                .any(|db| db.path.as_ref().map(|p| p == db_path).unwrap_or(false))),
            Err(e) => {
                tracing::warn!("Failed to list databases: {}", e);
                Ok(false)
            }
        }
    }

    /// Load all node configurations from the metadata database
    async fn load_nodes(&self) -> Result<()> {
        let client = self.manager.client().await?;

        let spec = BranchSpec {
            db: META_DATABASE.to_string(),
            branch: Some("main".to_string()),
            ref_commit: None,
        };

        let opts = GetOpts::default();
        let mut deserializer = DefaultTDBDeserializer;

        // Get all NodeConfig instances (empty vec means "get all")
        match client
            .get_instances::<NodeConfig>(vec![], &spec, opts, &mut deserializer)
            .await
        {
            Ok(mut nodes) => {
                // Auto-layout nodes that are all at (0, 0)
                let all_at_origin = nodes
                    .iter()
                    .all(|n| n.position_x == 0.0 && n.position_y == 0.0);
                if all_at_origin && nodes.len() > 1 {
                    // Arrange in a horizontal grid with spacing
                    const SPACING_X: f64 = 250.0;
                    const SPACING_Y: f64 = 150.0;
                    const NODES_PER_ROW: usize = 3;

                    for (i, node) in nodes.iter_mut().enumerate() {
                        let row = i / NODES_PER_ROW;
                        let col = i % NODES_PER_ROW;
                        node.position_x = col as f64 * SPACING_X;
                        node.position_y = row as f64 * SPACING_Y;
                    }
                }

                for node in nodes {
                    self.nodes.write().insert(node.id.clone(), node);
                }
                tracing::info!("Loaded {} node configuration(s)", self.nodes.read().len());
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to load nodes from database: {}. Starting with empty cache.",
                    e
                );
            }
        }

        Ok(())
    }

    /// Save a node configuration to the metadata database
    pub async fn save_node(&self, node: &NodeConfig) -> Result<()> {
        // Use a dedicated client for the local manager database
        // We can't use get_or_create_client here because the node might not be in cache yet
        let client = self.manager.client().await?;

        let spec = BranchSpec {
            db: META_DATABASE.to_string(),
            branch: Some("main".to_string()),
            ref_commit: None,
        };

        let args = DocumentInsertArgs::from(spec);

        // Insert into database
        client
            .insert_instance(node, args)
            .await
            .context("Failed to persist node to database")?;

        // Update in-memory cache
        let is_new = !self.nodes.read().contains_key(&node.id);
        self.nodes.write().insert(node.id.clone(), node.clone());
        tracing::info!("Saved node configuration: {}", node.id);

        // Start poller for new nodes
        if is_new {
            self.start_node_poller(node.id.clone());
        }

        // Cache will be created on first use via get_or_create_client()

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

        let client = self.manager.client().await?;

        let spec = BranchSpec {
            db: META_DATABASE.to_string(),
            branch: Some("main".to_string()),
            ref_commit: None,
        };

        let args = DocumentInsertArgs::from(spec);
        let opts = DeleteOpts::document_only();

        // Delete from database
        client
            .delete_instance_by_id::<NodeConfig>(id, args, opts)
            .await
            .context("Failed to delete node from database")?;

        // Stop the poller for this node
        self.stop_node_poller(id);

        // Remove from in-memory caches
        self.nodes.write().remove(id);
        self.statuses.write().remove(id);
        self.invalidate_client(id);

        tracing::info!("Deleted node configuration: {}", id);
        Ok(())
    }

    /// Update a node configuration
    pub async fn update_node(&self, node: NodeConfig) -> Result<()> {
        // Check if credentials or connection details changed
        let should_invalidate = if let Some(old_node) = self.get_node(&node.id) {
            old_node.host != node.host
                || old_node.port != node.port
                || old_node.username != node.username
                || old_node.password != node.password
        } else {
            false
        };

        // Save the updated node
        self.save_node(&node).await?;

        // Invalidate cached client if connection details changed
        if should_invalidate {
            self.invalidate_client(&node.id);
        }

        Ok(())
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

    /// Get or create a cached HTTP client for a node
    pub async fn get_or_create_client(&self, node_id: &str) -> Result<Arc<TerminusDBHttpClient>> {
        // Check if client exists in cache
        {
            let clients = self.clients.read();
            if let Some(client) = clients.get(node_id) {
                return Ok(Arc::clone(client));
            }
        }

        // Client not in cache, create a new one
        let node = self
            .get_node(node_id)
            .ok_or_else(|| anyhow::anyhow!("Node not found: {}", node_id))?;

        let url = url::Url::parse(&node.base_url()).context("Invalid node URL")?;

        let client =
            TerminusDBHttpClient::new(url, &node.username, &node.password, "admin").await?;

        let client_arc = Arc::new(client);

        // Store in cache
        self.clients
            .write()
            .insert(node_id.to_string(), Arc::clone(&client_arc));

        tracing::debug!("Created new HTTP client for node: {}", node_id);

        Ok(client_arc)
    }

    /// Invalidate a cached client (e.g., when credentials change)
    pub fn invalidate_client(&self, node_id: &str) {
        if self.clients.write().remove(node_id).is_some() {
            tracing::debug!("Invalidated HTTP client for node: {}", node_id);
        }
    }

    /// Get the local manager
    pub fn manager(&self) -> &TerminusDBManager {
        &self.manager
    }

    /// Start a per-node poller for the given node ID
    pub fn start_node_poller(&self, node_id: String) {
        use crate::poller::spawn_node_poller;

        // Check if a poller is already running for this node
        if self.poller_handles.read().contains_key(&node_id) {
            tracing::debug!("Poller already running for node: {}", node_id);
            return;
        }

        // Spawn the poller
        let handle = spawn_node_poller(self.clone(), node_id.clone());

        // Store the handle
        self.poller_handles.write().insert(node_id.clone(), handle);

        tracing::info!("Started poller for node: {}", node_id);
    }

    /// Stop the poller for the given node ID
    pub fn stop_node_poller(&self, node_id: &str) {
        if let Some(handle) = self.poller_handles.write().remove(node_id) {
            handle.abort();
            tracing::info!("Stopped poller for node: {}", node_id);
        }
    }

    /// Start pollers for all nodes
    pub fn start_all_pollers(&self) {
        let node_ids: Vec<String> = self.nodes.read().keys().cloned().collect();
        for node_id in node_ids {
            self.start_node_poller(node_id);
        }
    }

    /// Stop all pollers (useful for graceful shutdown)
    pub fn stop_all_pollers(&self) {
        let node_ids: Vec<String> = self.poller_handles.read().keys().cloned().collect();
        for node_id in node_ids {
            self.stop_node_poller(&node_id);
        }
    }
}
