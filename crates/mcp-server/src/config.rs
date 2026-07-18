//! Connection configuration for the TerminusDB MCP server.

use rust_mcp_sdk::macros::JsonSchema;
use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ConnectionConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_user")]
    pub user: String,
    #[serde(default = "default_password")]
    pub password: String,
    pub database: Option<String>,
    #[serde(default = "default_branch")]
    pub branch: String,
    /// Commit ID for time-travel queries (optional)
    pub commit_ref: Option<String>,
}

fn default_host() -> String {
    env::var("TERMINUSDB_HOST").unwrap_or_else(|_| "http://localhost:6363".to_string())
}

fn default_user() -> String {
    env::var("TERMINUSDB_USER").unwrap_or_else(|_| "admin".to_string())
}

fn default_password() -> String {
    // Check TERMINUSDB_ADMIN_PASS first (Docker image compatibility),
    // then fall back to TERMINUSDB_PASS, then to "root"
    env::var("TERMINUSDB_ADMIN_PASS")
        .or_else(|_| env::var("TERMINUSDB_PASS"))
        .unwrap_or_else(|_| "root".to_string())
}

fn default_branch() -> String {
    env::var("TERMINUSDB_BRANCH").unwrap_or_else(|_| "main".to_string())
}

fn default_database() -> Option<String> {
    env::var("TERMINUSDB_DATABASE").ok()
}

fn default_commit_ref() -> Option<String> {
    env::var("TERMINUSDB_COMMIT_REF").ok()
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            user: default_user(),
            password: default_password(),
            database: default_database(),
            branch: default_branch(),
            commit_ref: default_commit_ref(),
        }
    }
}
