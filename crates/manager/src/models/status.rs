use serde::{Deserialize, Serialize};

use super::database::DatabaseInfo;

/// Connectivity level for a node
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectivityLevel {
    /// Node is unreachable
    Unreachable,
    /// Node responds to info() but auth might be wrong
    Reachable,
    /// Full access - can list databases
    Accessible,
}

/// Health status for a TerminusDB instance node
#[derive(Debug, Clone, Serialize)]
pub struct NodeStatus {
    /// Node ID this status belongs to
    pub node_id: String,

    /// Whether the node is online/reachable
    pub online: bool,

    /// Connectivity level (unreachable, reachable, accessible)
    pub connectivity: ConnectivityLevel,

    /// Number of databases on this instance
    pub database_count: usize,

    /// Cached database information (populated by background poller)
    #[serde(default)]
    pub databases: Vec<DatabaseInfo>,

    /// Remote connections from databases on this node
    pub remotes: Vec<RemoteInfo>,

    /// Last time status was checked (ISO 8601)
    pub last_check: String,

    /// Optional error message if offline
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl NodeStatus {
    /// Create a new online status with full access
    pub fn online(
        node_id: String,
        database_count: usize,
        databases: Vec<DatabaseInfo>,
        remotes: Vec<RemoteInfo>,
    ) -> Self {
        Self {
            node_id,
            online: true,
            connectivity: ConnectivityLevel::Accessible,
            database_count,
            databases,
            remotes,
            last_check: chrono::Utc::now().to_rfc3339(),
            error: None,
        }
    }

    /// Create a new reachable status (info() works but can't list databases)
    pub fn reachable(node_id: String) -> Self {
        Self {
            node_id,
            online: true,
            connectivity: ConnectivityLevel::Reachable,
            database_count: 0,
            databases: Vec::new(),
            remotes: Vec::new(),
            last_check: chrono::Utc::now().to_rfc3339(),
            error: Some("Cannot list databases - check credentials".to_string()),
        }
    }

    /// Create a new offline status
    pub fn offline(node_id: String, error: String) -> Self {
        Self {
            node_id,
            online: false,
            connectivity: ConnectivityLevel::Unreachable,
            database_count: 0,
            databases: Vec::new(),
            remotes: Vec::new(),
            last_check: chrono::Utc::now().to_rfc3339(),
            error: Some(error),
        }
    }
}

/// Information about a remote connection
#[derive(Debug, Clone, Serialize)]
pub struct RemoteInfo {
    /// Database name that has this remote
    pub database: String,

    /// Remote name (e.g., "origin", "backup")
    pub remote_name: String,

    /// Remote URL
    pub remote_url: String,

    /// Matched node ID if URL maps to a known node
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_node_id: Option<String>,
}
