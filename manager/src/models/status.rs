use serde::{Deserialize, Serialize};

/// Health status for a TerminusDB instance node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStatus {
    /// Node ID this status belongs to
    pub node_id: String,

    /// Whether the node is online/reachable
    pub online: bool,

    /// Number of databases on this instance
    pub database_count: usize,

    /// Remote connections from databases on this node
    pub remotes: Vec<RemoteInfo>,

    /// Last time status was checked (ISO 8601)
    pub last_check: String,

    /// Optional error message if offline
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl NodeStatus {
    /// Create a new online status
    pub fn online(node_id: String, database_count: usize, remotes: Vec<RemoteInfo>) -> Self {
        Self {
            node_id,
            online: true,
            database_count,
            remotes,
            last_check: chrono::Utc::now().to_rfc3339(),
            error: None,
        }
    }

    /// Create a new offline status
    pub fn offline(node_id: String, error: String) -> Self {
        Self {
            node_id,
            online: false,
            database_count: 0,
            remotes: Vec::new(),
            last_check: chrono::Utc::now().to_rfc3339(),
            error: Some(error),
        }
    }
}

/// Information about a remote connection
#[derive(Debug, Clone, Serialize, Deserialize)]
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
