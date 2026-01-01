use serde::{Deserialize, Serialize};

/// Database information with metadata
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    /// Database name/path
    pub name: String,

    /// Number of commits in the database
    pub commit_count: usize,

    /// Last modification timestamp (ISO 8601 format)
    pub last_modified: String,

    /// Number of remotes configured
    #[serde(default)]
    pub remote_count: usize,
}

/// Model/Entity type information
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// Model/Entity type name
    pub name: String,

    /// Number of instances of this type
    pub instance_count: usize,
}

/// Commit information
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitInfo {
    /// Commit ID/hash
    pub id: String,

    /// Commit author
    pub author: String,

    /// Commit message
    pub message: String,

    /// Commit timestamp (ISO 8601 format)
    pub timestamp: String,
}

/// Remote information (already exists in status.rs but re-exported here for convenience)
pub use super::status::RemoteInfo;

/// Request to add a remote
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddRemoteRequest {
    /// Remote name (e.g., "origin", "backup")
    pub remote_name: String,

    /// Remote URL
    pub remote_url: String,
}
