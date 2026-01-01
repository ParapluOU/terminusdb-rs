use serde::{Deserialize, Serialize};
use terminusdb_schema::*;
use terminusdb_schema_derive::{TerminusDBModel, FromTDBInstance};

/// Configuration for a TerminusDB instance node
#[derive(Debug, Clone, TerminusDBModel, FromTDBInstance)]
#[tdb(key = "id")]
pub struct NodeConfig {
    /// Unique identifier for the node (also used as TerminusDB key)
    pub id: String,

    /// Display label for the node
    pub label: String,

    /// Hostname or IP address
    pub host: String,

    /// Port number
    pub port: u32,

    /// Username for authentication
    pub username: String,

    /// Password for authentication (plaintext)
    pub password: String,

    /// Whether SSH access is available
    pub ssh_enabled: bool,

    /// X position on canvas
    pub position_x: f64,

    /// Y position on canvas
    pub position_y: f64,
}

impl NodeConfig {
    /// Create a new node configuration
    pub fn new(
        id: String,
        label: String,
        host: String,
        port: u32,
        username: String,
        password: String,
    ) -> Self {
        Self {
            id,
            label,
            host,
            port,
            username,
            password,
            ssh_enabled: false,
            position_x: 0.0,
            position_y: 0.0,
        }
    }

    /// Create the default localhost node configuration
    pub fn localhost(port: u32) -> Self {
        Self::new(
            "local".to_string(),
            "Local Instance".to_string(),
            "localhost".to_string(),
            port,
            "admin".to_string(),
            "root".to_string(),
        )
    }

    /// Get the base URL for this node
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}
