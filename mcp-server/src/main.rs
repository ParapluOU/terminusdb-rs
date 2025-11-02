use anyhow::Result;
use async_trait::async_trait;
use rust_mcp_sdk::macros::{mcp_tool, JsonSchema};
use rust_mcp_sdk::schema::{
    schema_utils::CallToolError, CallToolRequest, CallToolResult, Implementation, InitializeResult,
    ListToolsRequest, ListToolsResult, RpcError, TextContent, ServerCapabilities,
};
use rust_mcp_sdk::{
    mcp_server::{server_runtime, ServerHandler},
    McpServer, StdioTransport, TransportOptions,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::fmt;
use std::sync::Arc;
use terminusdb_client::{BranchSpec, TerminusDBHttpClient, GetOpts, DocumentInsertArgs};
use terminusdb_client::debug::QueryLogEntry;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use tracing_subscriber;
use url::Url;

// Simple error wrapper for anyhow::Error
#[derive(Debug)]
struct McpError(anyhow::Error);

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for McpError {}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
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

// Tool definitions using mcp_tool macro
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "connect",
    description = "Establish and save a connection to TerminusDB. Once connected, other commands will use these saved credentials automatically. Optionally provide an env_file path to load environment variables."
)]
pub struct ConnectTool {
    pub host: Option<String>,
    pub user: Option<String>,
    pub password: Option<String>,
    pub database: Option<String>,
    pub branch: Option<String>,
    /// Commit ID for time-travel queries (optional)
    pub commit_ref: Option<String>,
    /// Path to .env file to load additional environment variables
    pub env_file: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "execute_woql",
    description = "Execute a WOQL query using either DSL syntax or JSON-LD format. DSL syntax uses a compositional, function-based syntax with variables prefixed with $ (e.g., $Person). Common operations include: triple($Subject, predicate, $Object), and(...), or(...), select([$Var1, $Var2], query), greater($Var, value). Alternatively, you can provide a JSON-LD query object following the WOQL schema. The tool automatically detects the format and parses accordingly."
)]
pub struct ExecuteWoqlTool {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(name = "list_databases", description = "List all available databases")]
pub struct ListDatabasesTool {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "get_schema",
    description = "Get the schema for a specific database"
)]
pub struct GetSchemaTool {
    pub database: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "check_server_status",
    description = "Check if the TerminusDB server is running and accessible"
)]
pub struct CheckServerStatusTool {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "reset_database",
    description = "Reset a database by deleting and recreating it. WARNING: This permanently deletes all data in the database!"
)]
pub struct ResetDatabaseTool {
    pub database: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "get_document",
    description = "Retrieve a document by ID from TerminusDB. Returns the document as JSON with optional metadata like commit ID."
)]
pub struct GetDocumentTool {
    /// Document ID to retrieve (e.g., "Person/12345" or just "12345" with type_name)
    pub document_id: String,
    /// Optional document type/class name (e.g., "Person") - used if document_id doesn't include type prefix
    pub type_name: Option<String>,
    /// Whether to unfold linked documents (default: false)
    #[serde(default)]
    pub unfold: bool,
    /// Return document as list format (default: false)
    #[serde(default)]
    pub as_list: bool,
    /// Include commit/version information in response (default: false)
    #[serde(default)]
    pub include_headers: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}


#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "query_log",
    description = "Manage and view TerminusDB query logging. Supports enabling/disabling file-based query logging, viewing recent entries, and log rotation."
)]
pub struct QueryLogTool {
    /// Action to perform: "status", "enable", "disable", "rotate", "view"
    pub action: String,
    /// Path to log file (required for "enable" action)
    pub log_path: Option<String>,
    /// For "view" action: number of recent entries to return (default: 20, max: 100)
    pub limit: Option<String>,
    /// For "view" action: number of entries to skip from the end (for pagination)
    pub offset: Option<String>,
    /// For "view" action: filter by operation type
    pub operation_type_filter: Option<String>,
    /// For "view" action: filter by success status
    pub success_filter: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "insert_document",
    description = "Insert a single JSON document into TerminusDB. The document must include @id and @type fields. Supports both creation and update (upsert) behavior."
)]
pub struct InsertDocumentTool {
    /// JSON document to insert (must include @id and @type fields)
    pub document: serde_json::Value,
    /// Database name
    pub database: String,
    /// Branch name (defaults to "main" if not specified)
    pub branch: Option<String>,
    /// Commit message (defaults to "insert document")
    pub message: Option<String>,
    /// Author name (defaults to "system")
    pub author: Option<String>,
    /// Force insertion even if document exists
    #[serde(default)]
    pub force: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "insert_documents",
    description = "Insert multiple JSON documents into TerminusDB in a single transaction. All documents must include @id and @type fields. This is an atomic operation - all documents succeed or all fail."
)]
pub struct InsertDocumentsTool {
    /// Array of JSON documents to insert (each must include @id and @type fields)
    pub documents: Vec<serde_json::Value>,
    /// Database name
    pub database: String,
    /// Branch name (defaults to "main" if not specified)
    pub branch: Option<String>,
    /// Commit message (defaults to "insert documents")
    pub message: Option<String>,
    /// Author name (defaults to "system")
    pub author: Option<String>,
    /// Force insertion for existing documents
    #[serde(default)]
    pub force: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "replace_document",
    description = "Replace an existing document entirely. This operation will fail if the document doesn't exist (safer than upsert). Use this when you want to guarantee the document already exists."
)]
pub struct ReplaceDocumentTool {
    /// JSON document to replace with (must include @id and @type fields)
    pub document: serde_json::Value,
    /// Database name
    pub database: String,
    /// Branch name (defaults to "main" if not specified)
    pub branch: Option<String>,
    /// Commit message (defaults to "replace document")
    pub message: Option<String>,
    /// Author name (defaults to "system")
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "delete_classes",
    description = "Delete schema classes (tables) from TerminusDB by removing all their schema triples. WARNING: This permanently deletes the class definitions from the schema!"
)]
pub struct DeleteClassesTool {
    /// List of class names to delete (e.g., ["AwsDBPublication", "AwsDBPublicationMap"])
    pub class_names: Vec<String>,
    /// Database name
    pub database: String,
    /// Branch name (defaults to "main" if not specified)
    pub branch: Option<String>,
    /// Commit message (defaults to "delete classes")
    pub message: Option<String>,
    /// Author name (defaults to "system")
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "squash",
    description = "Squash commit history into a single commit. Creates a new unattached commit containing the squashed data. This commit can be queried directly, or be assigned to a particular branch using the reset endpoint."
)]
pub struct SquashTool {
    /// Path for a commit or branch (e.g., "admin/mydb/local/branch/main" or "admin/mydb/local/commit/abc123")
    pub path: String,
    /// The author of the squash operation
    pub author: String,
    /// Commit message describing the squash
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "reset",
    description = "Reset branch to a specific commit. This will set the branch HEAD to the submitted commit."
)]
pub struct ResetTool {
    /// Path to the branch to reset (e.g., "admin/mydb/local/branch/main")
    pub branch_path: String,
    /// Path to a specific commit or to a branch (e.g., "admin/mydb/local/commit/abc123")
    pub commit_descriptor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "optimize",
    description = "Optimize a database by removing unreachable data. This can significantly reduce database size and improve performance."
)]
pub struct OptimizeTool {
    /// Path to optimize (e.g., "admin/mydb/_meta" or "admin/mydb/local/branch/main")
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "get_graphql_schema",
    description = "Download the GraphQL schema for a database using introspection. The schema is saved to a file (default: ./schema.json)."
)]
pub struct GetGraphQLSchemaTool {
    /// Database name to introspect
    pub database: String,
    /// Branch name (defaults to "main" if not specified)
    pub branch: Option<String>,
    /// Output file path (defaults to "./schema.json")
    pub output_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

/// Clone a remote repository to create a new local database
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "clone",
    description = "Clone a remote repository to create a new local database"
)]
pub struct CloneTool {
    /// Organization to create the database in
    pub organization: String,
    /// Name for the new database
    pub database: String,
    /// URL of the remote repository to clone
    pub remote_url: String,
    /// Optional label for the database
    pub label: Option<String>,
    /// Optional comment for the database
    pub comment: Option<String>,
    /// Optional username for authenticating to the remote repository
    pub remote_username: Option<String>,
    /// Optional password or token for authenticating to the remote repository
    pub remote_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

/// Fetch changes from a remote repository
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "fetch",
    description = "Fetch changes from a remote repository into the local database"
)]
pub struct FetchTool {
    /// Path to fetch into (e.g., "admin/mydb/local/branch/main")
    pub path: String,
    /// Name of the remote repository (e.g., "origin")
    pub remote_url: String,
    /// Optional remote branch name (defaults to "main" if not specified)
    pub remote_branch: Option<String>,
    /// Optional username for authenticating to the remote repository
    pub remote_username: Option<String>,
    /// Optional password or token for authenticating to the remote repository
    pub remote_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

/// Push changes to a remote repository
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "push",
    description = "Push changes from the local database to a remote repository"
)]
pub struct PushTool {
    /// Path to push from (e.g., "admin/mydb/local/branch/main")
    pub path: String,
    /// URL of the remote repository
    pub remote_url: String,
    /// Optional remote branch name (defaults to same as local)
    pub remote_branch: Option<String>,
    /// Optional username for authenticating to the remote repository
    pub remote_username: Option<String>,
    /// Optional password or token for authenticating to the remote repository
    pub remote_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

/// Pull changes from a remote repository (fetch + merge)
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "pull",
    description = "Pull changes from a remote repository and merge them into the local database"
)]
pub struct PullTool {
    /// Path to pull into (e.g., "admin/mydb/local/branch/main")
    pub path: String,
    /// URL of the remote repository
    pub remote_url: String,
    /// Optional remote branch name
    pub remote_branch: Option<String>,
    /// Author for the merge commit
    pub author: String,
    /// Message for the merge commit
    pub message: String,
    /// Optional username for authenticating to the remote repository
    pub remote_username: Option<String>,
    /// Optional password or token for authenticating to the remote repository
    pub remote_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

/// Add a new remote repository
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "add_remote",
    description = "Add a new remote repository to a database. The remote name is specified in the path (e.g., 'admin/mydb/remote/origin'). For full API details, see terminusdb-rs/docs/openapi.yaml or https://github.com/terminusdb/terminusdb/blob/main/docs/openapi.yaml"
)]
pub struct AddRemoteTool {
    /// Path where the remote will be added. Format: org/db/remote/remote_name (e.g., "admin/mydb/remote/origin"). The last segment of the path becomes the remote name.
    pub path: String,
    /// URL of the remote repository (e.g., "https://github.com/user/repo.git")
    pub remote_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

/// Get information about a remote repository
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "get_remote",
    description = "Get information about a remote repository"
)]
pub struct GetRemoteTool {
    /// Path to the remote (e.g., "admin/mydb/remote/origin")
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

/// Update a remote repository URL
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "update_remote",
    description = "Update the URL of an existing remote repository"
)]
pub struct UpdateRemoteTool {
    /// Path to the remote (e.g., "admin/mydb/remote/origin")
    pub path: String,
    /// New URL for the remote repository
    pub remote_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

/// Delete a remote repository
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "delete_remote",
    description = "Delete a remote repository from a database"
)]
pub struct DeleteRemoteTool {
    /// Path to the remote to delete (e.g., "admin/mydb/remote/origin")
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

pub struct TerminusDBMcpHandler {
    saved_config: Arc<RwLock<Option<ConnectionConfig>>>,
}

impl TerminusDBMcpHandler {
    fn new() -> Self {
        Self {
            saved_config: Arc::new(RwLock::new(None)),
        }
    }

    async fn create_client(config: &ConnectionConfig) -> Result<TerminusDBHttpClient> {
        let url = Url::parse(&config.host)?;
        TerminusDBHttpClient::new(
            url,
            &config.user,
            &config.password,
            &config.user, // org defaults to user
        )
        .await
    }

    async fn get_connection_config(&self, provided: Option<ConnectionConfig>) -> ConnectionConfig {
        if let Some(config) = provided {
            return config;
        }

        if let Some(config) = self.saved_config.read().await.clone() {
            return config;
        }

        ConnectionConfig::default()
    }

    async fn connect(&self, request: ConnectTool) -> Result<serde_json::Value> {
        info!("Establishing connection to TerminusDB");

        // Load env file if provided (this updates the environment for subsequent reads)
        if let Some(env_file) = &request.env_file {
            if let Err(e) = dotenv::from_path(env_file) {
                info!("Failed to load env file {}: {}", env_file, e);
                // Don't fail hard, just log the error
            }
        }

        // Start with defaults/environment variables (these will now read from the updated environment)
        let base_config = ConnectionConfig::default();
        
        // Merge request values with base config (request values take precedence)
        let config = ConnectionConfig {
            host: request.host.unwrap_or(base_config.host),
            user: request.user.unwrap_or(base_config.user),
            password: request.password.unwrap_or(base_config.password),
            database: request.database.or(base_config.database),
            branch: request.branch.unwrap_or(base_config.branch),
            commit_ref: request.commit_ref.or(base_config.commit_ref),
        };

        // Test the connection
        match Self::create_client(&config).await {
            Ok(client) => {
                // Check if server is running
                if !client.is_running().await {
                    return Err(anyhow::anyhow!("Server is not running at {}", config.host));
                }

                // Save the config
                let mut saved = self.saved_config.write().await;
                *saved = Some(config.clone());

                Ok(serde_json::json!({
                    "status": "connected",
                    "host": config.host,
                    "user": config.user,
                    "database": config.database,
                    "branch": config.branch,
                    "message": "Connection established and saved successfully"
                }))
            }
            Err(e) => Err(anyhow::anyhow!("Failed to connect: {}", e)),
        }
    }

    async fn execute_woql(&self, request: ExecuteWoqlTool) -> Result<serde_json::Value> {
        info!("Executing WOQL query: {}", request.query);

        // Get connection config
        let config = self.get_connection_config(request.connection).await;

        // Create client
        let client = Self::create_client(&config).await?;

        // Execute query
        if let Some(database) = &config.database {
            let branch_spec = match &config.commit_ref {
                Some(commit_id) => {
                    // Time-travel query to specific commit
                    BranchSpec::with_commit(database, commit_id.as_str())
                }
                None => {
                    // Regular query on branch
                    BranchSpec::with_branch(database, &config.branch)
                }
            };
            // Use the new query_string method that handles both DSL and JSON-LD
            let response: terminusdb_client::WOQLResult<serde_json::Value> =
                client.query_string(Some(branch_spec), &request.query).await?;
            Ok(serde_json::to_value(&response)?)
        } else {
            Err(anyhow::anyhow!("Database must be specified"))
        }
    }

    async fn list_databases(&self, request: ListDatabasesTool) -> Result<serde_json::Value> {
        info!("Listing databases");

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;
        
        // List databases with default options (not verbose, no branches)
        let databases = client.list_databases_simple().await?;
        
        // Convert to a more structured format for the MCP response
        let database_list: Vec<serde_json::Value> = databases
            .into_iter()
            .map(|db| {
                let mut obj = serde_json::Map::new();
                
                // Extract database name and organization first (they use references)
                if let Some(name) = db.database_name() {
                    obj.insert("name".to_string(), serde_json::Value::String(name));
                }
                if let Some(org) = db.organization() {
                    obj.insert("organization".to_string(), serde_json::Value::String(org));
                }
                
                // Always include path if available
                if let Some(path) = db.path {
                    obj.insert("path".to_string(), serde_json::Value::String(path));
                }
                
                // Include other fields if available
                if let Some(id) = db.id {
                    obj.insert("id".to_string(), serde_json::Value::String(id));
                }
                if let Some(db_type) = db.database_type {
                    obj.insert("type".to_string(), serde_json::Value::String(db_type));
                }
                if let Some(state) = db.state {
                    obj.insert("state".to_string(), serde_json::Value::String(state));
                }
                
                serde_json::Value::Object(obj)
            })
            .collect();
        
        Ok(serde_json::json!({
            "databases": database_list,
            "count": database_list.len()
        }))
    }

    async fn get_schema(&self, request: GetSchemaTool) -> Result<serde_json::Value> {
        info!("Getting schema for database: {}", request.database);

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;
        let branch_spec = match &config.commit_ref {
            Some(commit_id) => {
                // Time-travel query to specific commit
                BranchSpec::with_commit(&request.database, commit_id.as_str())
            }
            None => {
                // Regular query on branch
                BranchSpec::with_branch(&request.database, &config.branch)
            }
        };

        // Query to get all classes in the schema
        let schema_query = r#"
            select([$Class, $Label, $Comment],
                and(
                    triple($Class, "rdf:type", "owl:Class", schema),
                    opt(triple($Class, "rdfs:label", $Label, schema)),
                    opt(triple($Class, "rdfs:comment", $Comment, schema))
                )
            )
        "#;

        let response: terminusdb_client::WOQLResult<serde_json::Value> =
            client.query_string(Some(branch_spec), schema_query).await?;
        Ok(serde_json::to_value(&response)?)
    }

    async fn check_server_status(
        &self,
        request: CheckServerStatusTool,
    ) -> Result<serde_json::Value> {
        info!("Checking TerminusDB server status");

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        // Use the is_running() method from TerminusDBHttpClient
        let is_running = client.is_running().await;

        // If server is running, get additional info
        if is_running {
            match client.info().await {
                Ok(info) => Ok(serde_json::json!({
                    "status": "running",
                    "connected": true,
                    "server_info": info
                })),
                Err(e) => Ok(serde_json::json!({
                    "status": "error",
                    "connected": false,
                    "error": format!("Server responded but info request failed: {}", e)
                })),
            }
        } else {
            Ok(serde_json::json!({
                "status": "offline",
                "connected": false,
                "error": "Server is not responding"
            }))
        }
    }

    async fn reset_database(&self, request: ResetDatabaseTool) -> Result<serde_json::Value> {
        info!("Resetting database: {}", request.database);

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;
        
        // Reset the database using the client method
        client.reset_database(&request.database).await?;
        
        Ok(serde_json::json!({
            "status": "success",
            "message": format!("Database '{}' has been reset successfully", request.database),
            "database": request.database
        }))
    }

    async fn get_document(&self, request: GetDocumentTool) -> Result<serde_json::Value> {
        info!("Retrieving document: {}", request.document_id);

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;
        
        if config.database.is_none() {
            return Err(anyhow::anyhow!("Database must be specified"));
        }
        
        let db = config.database.unwrap();
        let branch_spec = match &config.commit_ref {
            Some(commit_id) => BranchSpec::with_commit(&db, commit_id.as_str()),
            None => BranchSpec::with_branch(&db, &config.branch)
        };

        // Format document ID if type_name is provided
        let formatted_id = match &request.type_name {
            Some(type_name) if !request.document_id.contains('/') => {
                format!("{}/{}", type_name, request.document_id)
            }
            _ => request.document_id.clone()
        };

        let opts = GetOpts::default()
            .with_unfold(request.unfold)
            .with_as_list(request.as_list);

        if request.include_headers {
            let result = client.get_document_with_headers(&formatted_id, &branch_spec, opts).await?;
            Ok(serde_json::json!({
                "document": *result,
                "commit_id": result.extract_commit_id(),
                "metadata": {
                    "unfold": request.unfold,
                    "as_list": request.as_list,
                    "database": db,
                    "branch": config.branch
                }
            }))
        } else {
            let document = client.get_document(&formatted_id, &branch_spec, opts).await?;
            Ok(document)
        }
    }

    async fn handle_query_log(&self, request: QueryLogTool) -> Result<serde_json::Value> {
        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;
        
        let action = request.action.as_str();
        match action {
            "status" => {
                let enabled = client.is_query_log_enabled().await;
                // Query log path is not directly accessible from client
                let path: Option<String> = None;
                
                Ok(serde_json::json!({
                    "enabled": enabled,
                    "log_path": path,
                    "message": if enabled {
                        format!("Query logging is enabled to: {:?}", path)
                    } else {
                        "Query logging is disabled".to_string()
                    }
                }))
            }
            
            "enable" => {
                let path = request.log_path.as_deref()
                    .unwrap_or("/tmp/terminusdb_queries.log");
                
                client.enable_query_log(path).await?;
                
                Ok(serde_json::json!({
                    "status": "success",
                    "enabled": true,
                    "log_path": path,
                    "message": format!("Query logging enabled to: {}", path)
                }))
            }
            
            "disable" => {
                client.disable_query_log().await;
                
                Ok(serde_json::json!({
                    "status": "success", 
                    "enabled": false,
                    "message": "Query logging disabled"
                }))
            }
            
            "rotate" => {
                // TODO: Fix Send trait issue in client rotate_query_log
                // client.rotate_query_log().await?;
                
                Err(anyhow::anyhow!("Query log rotation is temporarily unavailable due to client implementation"))?
            }
            
            "view" => {
                // For now, we'll use the default path or the one from request
                let path = request.log_path.as_deref()
                    .unwrap_or("/tmp/terminusdb_queries.log");
                
                // Read the log file
                let content = tokio::fs::read_to_string(&path).await
                    .map_err(|e| anyhow::anyhow!("Failed to read log file: {}", e))?;
                
                // Parse log entries
                let mut entries: Vec<QueryLogEntry> = Vec::new();
                for line in content.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    
                    match serde_json::from_str::<QueryLogEntry>(line) {
                        Ok(entry) => entries.push(entry),
                        Err(e) => {
                            warn!("Failed to parse log entry: {}", e);
                        }
                    }
                }
                
                // Apply filters
                let mut filtered = entries;
                
                // Filter by operation type
                if let Some(op_type) = &request.operation_type_filter {
                    filtered.retain(|e| e.operation_type == *op_type);
                }
                
                // Filter by success status
                if let Some(success) = request.success_filter {
                    filtered.retain(|e| e.success == success);
                }
                
                // Sort by timestamp (newest first)
                filtered.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                
                // Apply pagination
                let limit = request.limit
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(20);
                let offset = request.offset
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);
                
                let total = filtered.len();
                let paginated: Vec<_> = filtered.into_iter()
                    .skip(offset)
                    .take(limit)
                    .collect();
                
                Ok(serde_json::json!({
                    "entries": paginated,
                    "pagination": {
                        "total": total,
                        "limit": limit,
                        "offset": offset,
                        "has_more": offset + paginated.len() < total
                    }
                }))
            }
            _ => Err(anyhow::anyhow!("Invalid action: {}. Must be one of: status, enable, disable, rotate, view", action))?
        }
    }

    async fn handle_insert_document(&self, request: InsertDocumentTool) -> Result<serde_json::Value> {
        // Validate document has required fields
        if !request.document.is_object() {
            return Err(anyhow::anyhow!("Document must be a JSON object"));
        }
        
        let doc_obj = request.document.as_object().unwrap();
        if !doc_obj.contains_key("@id") || !doc_obj.contains_key("@type") {
            return Err(anyhow::anyhow!("Document must contain @id and @type fields"));
        }
        
        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;
        
        // Set database in config
        let mut config = config;
        config.database = Some(request.database.clone());
        
        // Create branch spec
        let branch_spec = BranchSpec::with_branch(
            &request.database,
            &request.branch.unwrap_or_else(|| "main".to_string())
        );
        
        // Create insert args
        let args = DocumentInsertArgs {
            spec: branch_spec,
            message: request.message.unwrap_or_else(|| "insert document".to_string()),
            author: request.author.unwrap_or_else(|| "system".to_string()),
            force: request.force,
            ..Default::default()
        };
        
        // Insert the document
        let doc_vec = vec![&request.document];
        let result = client.insert_documents(doc_vec, args).await?;
        
        // Extract results
        let mut response = serde_json::json!({
            "status": "success",
            "database": request.database,
            "results": {}
        });
        
        if let Some(results_map) = response.get_mut("results").and_then(|v| v.as_object_mut()) {
            for (id, insert_result) in result.iter() {
                let status = match insert_result {
                    terminusdb_client::TDBInsertInstanceResult::Inserted(_) => "inserted",
                    terminusdb_client::TDBInsertInstanceResult::AlreadyExists(_) => "already_exists",
                };
                results_map.insert(id.clone(), serde_json::json!({
                    "id": id,
                    "status": status
                }));
            }
        }
        
        // Add commit ID if available
        if let Some(commit_id) = result.extract_commit_id() {
            response["commit_id"] = serde_json::Value::String(commit_id.to_string());
        }
        
        Ok(response)
    }

    async fn handle_insert_documents(&self, request: InsertDocumentsTool) -> Result<serde_json::Value> {
        // Validate all documents have required fields
        for (idx, doc) in request.documents.iter().enumerate() {
            if !doc.is_object() {
                return Err(anyhow::anyhow!("Document at index {} must be a JSON object", idx));
            }
            
            let doc_obj = doc.as_object().unwrap();
            if !doc_obj.contains_key("@id") || !doc_obj.contains_key("@type") {
                return Err(anyhow::anyhow!("Document at index {} must contain @id and @type fields", idx));
            }
        }
        
        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;
        
        // Set database in config
        let mut config = config;
        config.database = Some(request.database.clone());
        
        // Create branch spec
        let branch_spec = BranchSpec::with_branch(
            &request.database,
            &request.branch.unwrap_or_else(|| "main".to_string())
        );
        
        // Create insert args
        let args = DocumentInsertArgs {
            spec: branch_spec,
            message: request.message.unwrap_or_else(|| "insert documents".to_string()),
            author: request.author.unwrap_or_else(|| "system".to_string()),
            force: request.force,
            ..Default::default()
        };
        
        // Convert documents to references
        let doc_refs: Vec<&serde_json::Value> = request.documents.iter().collect();
        
        // Insert the documents
        let result = client.insert_documents(doc_refs, args).await?;
        
        // Build response
        let mut response = serde_json::json!({
            "status": "success",
            "database": request.database,
            "total_documents": request.documents.len(),
            "results": {}
        });
        
        let mut inserted_count = 0;
        let mut already_exists_count = 0;
        
        if let Some(results_map) = response.get_mut("results").and_then(|v| v.as_object_mut()) {
            for (id, insert_result) in result.iter() {
                let status = match insert_result {
                    terminusdb_client::TDBInsertInstanceResult::Inserted(_) => {
                        inserted_count += 1;
                        "inserted"
                    },
                    terminusdb_client::TDBInsertInstanceResult::AlreadyExists(_) => {
                        already_exists_count += 1;
                        "already_exists"
                    },
                };
                results_map.insert(id.clone(), serde_json::json!({
                    "id": id,
                    "status": status
                }));
            }
        }
        
        response["summary"] = serde_json::json!({
            "inserted": inserted_count,
            "already_exists": already_exists_count
        });
        
        // Add commit ID if available
        if let Some(commit_id) = result.extract_commit_id() {
            response["commit_id"] = serde_json::Value::String(commit_id.to_string());
        }
        
        Ok(response)
    }

    async fn handle_replace_document(&self, request: ReplaceDocumentTool) -> Result<serde_json::Value> {
        // Validate document has required fields
        if !request.document.is_object() {
            return Err(anyhow::anyhow!("Document must be a JSON object"));
        }
        
        let doc_obj = request.document.as_object().unwrap();
        if !doc_obj.contains_key("@id") || !doc_obj.contains_key("@type") {
            return Err(anyhow::anyhow!("Document must contain @id and @type fields"));
        }
        
        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;
        
        // Set database in config
        let mut config = config;
        config.database = Some(request.database.clone());
        
        // Create branch spec
        let branch_spec = BranchSpec::with_branch(
            &request.database,
            &request.branch.unwrap_or_else(|| "main".to_string())
        );
        
        // Create insert args
        let args = DocumentInsertArgs {
            spec: branch_spec,
            message: request.message.unwrap_or_else(|| "replace document".to_string()),
            author: request.author.unwrap_or_else(|| "system".to_string()),
            force: false, // Never force for replace operation
            ..Default::default()
        };
        
        // Use put_documents which requires document to exist
        let doc_vec = vec![&request.document];
        let result = client.put_documents(doc_vec, args).await?;
        
        // Extract results
        let mut response = serde_json::json!({
            "status": "success",
            "database": request.database,
            "operation": "replace",
            "results": {}
        });
        
        if let Some(results_map) = response.get_mut("results").and_then(|v| v.as_object_mut()) {
            for (id, _) in result.iter() {
                results_map.insert(id.clone(), serde_json::json!({
                    "id": id,
                    "status": "replaced"
                }));
            }
        }
        
        // Add commit ID if available
        if let Some(commit_id) = result.extract_commit_id() {
            response["commit_id"] = serde_json::Value::String(commit_id.to_string());
        }
        
        Ok(response)
    }

    async fn handle_delete_classes(&self, request: DeleteClassesTool) -> Result<serde_json::Value> {
        info!("Deleting classes: {:?}", request.class_names);
        
        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;
        
        // Create branch spec
        let branch_name = request.branch.as_deref().unwrap_or("main");
        let branch_spec = BranchSpec::with_branch(
            &request.database,
            branch_name
        );
        
        // For each class, we need to delete all its schema triples
        // This includes the class definition itself and all its properties
        let mut deleted_classes = Vec::new();
        let mut errors = Vec::new();
        
        for class_name in &request.class_names {
            // Build a WOQL query to delete all triples where the subject is the class
            // or where the domain is the class (for properties)
            let class_uri = format!("@schema:{}", class_name);
            
            // Create a JSON-LD query to delete all schema triples for this class
            let delete_query = json!({
                "@type": "And",
                "and": [
                    // Delete the class definition itself
                    {
                        "@type": "DeleteTriple",
                        "subject": {
                            "@type": "NodeValue",
                            "node": &class_uri
                        },
                        "predicate": {
                            "@type": "NodeValue",
                            "variable": "Predicate1"
                        },
                        "object": {
                            "@type": "Value",
                            "variable": "Object1"
                        },
                        "graph": "schema"
                    },
                    // Delete all properties that have this class as domain
                    {
                        "@type": "DeleteTriple",
                        "subject": {
                            "@type": "NodeValue",
                            "variable": "Property"
                        },
                        "predicate": {
                            "@type": "NodeValue",
                            "node": "rdfs:domain"
                        },
                        "object": {
                            "@type": "Value",
                            "node": &class_uri
                        },
                        "graph": "schema"
                    },
                    // Delete all other triples related to those properties
                    {
                        "@type": "DeleteTriple",
                        "subject": {
                            "@type": "NodeValue",
                            "variable": "Property"
                        },
                        "predicate": {
                            "@type": "NodeValue",
                            "variable": "Predicate2"
                        },
                        "object": {
                            "@type": "Value",
                            "variable": "Object2"
                        },
                        "graph": "schema"
                    }
                ]
            });
            
            // First, find all the triples to delete
            let _find_query = json!({
                "@type": "And",
                "and": [
                    // Find class triples
                    {
                        "@type": "Triple",
                        "subject": {
                            "@type": "NodeValue",
                            "node": &class_uri
                        },
                        "predicate": {
                            "@type": "NodeValue",
                            "variable": "Predicate1"
                        },
                        "object": {
                            "@type": "Value",
                            "variable": "Object1"
                        },
                        "graph": "schema"
                    },
                    // Find properties with this class as domain
                    {
                        "@type": "Triple",
                        "subject": {
                            "@type": "NodeValue",
                            "variable": "Property"
                        },
                        "predicate": {
                            "@type": "NodeValue",
                            "node": "rdfs:domain"
                        },
                        "object": {
                            "@type": "Value",
                            "node": &class_uri
                        },
                        "graph": "schema"
                    },
                    // Find all triples for those properties
                    {
                        "@type": "Triple",
                        "subject": {
                            "@type": "NodeValue",
                            "variable": "Property"
                        },
                        "predicate": {
                            "@type": "NodeValue",
                            "variable": "Predicate2"
                        },
                        "object": {
                            "@type": "Value",
                            "variable": "Object2"
                        },
                        "graph": "schema"
                    }
                ]
            });
            
            // Execute the deletion query
            match client.query_string::<serde_json::Value>(Some(branch_spec.clone()), &serde_json::to_string(&delete_query)?).await {
                Ok(_) => {
                    deleted_classes.push(class_name.clone());
                    info!("Successfully deleted class: {}", class_name);
                }
                Err(e) => {
                    let error_msg = format!("Failed to delete class {}: {}", class_name, e);
                    error!("{}", error_msg);
                    errors.push(error_msg);
                }
            }
        }
        
        let response = if errors.is_empty() {
            serde_json::json!({
                "status": "success",
                "message": format!("Successfully deleted {} classes", deleted_classes.len()),
                "deleted_classes": deleted_classes,
                "database": request.database,
                "branch": request.branch.unwrap_or_else(|| "main".to_string())
            })
        } else {
            serde_json::json!({
                "status": "partial_success",
                "message": format!("Deleted {} classes, {} failed", deleted_classes.len(), errors.len()),
                "deleted_classes": deleted_classes,
                "errors": errors,
                "database": request.database,
                "branch": branch_name
            })
        };
        
        Ok(response)
    }

    async fn handle_squash(&self, request: SquashTool) -> Result<serde_json::Value> {
        info!("Squashing commits for path: {}", request.path);
        
        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;
        
        // Execute the squash operation
        match client.squash(&request.path, &request.author, &request.message).await {
            Ok(response) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "path": request.path,
                    "new_commit": response.commit,
                    "old_commit": response.old_commit,
                    "api_status": format!("{:?}", response.status),
                    "message": format!("Successfully squashed commits. New commit: {}", response.commit)
                }))
            }
            Err(e) => {
                error!("Failed to squash commits: {}", e);
                Err(anyhow::anyhow!("Failed to squash commits: {}", e))
            }
        }
    }

    async fn handle_reset(&self, request: ResetTool) -> Result<serde_json::Value> {
        info!("Resetting branch {} to {}", request.branch_path, request.commit_descriptor);
        
        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;
        
        // Execute the reset operation
        match client.reset(&request.branch_path, &request.commit_descriptor).await {
            Ok(response) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "branch_path": request.branch_path,
                    "commit_descriptor": request.commit_descriptor,
                    "api_response": response,
                    "message": format!("Successfully reset branch {} to {}", 
                        request.branch_path, request.commit_descriptor)
                }))
            }
            Err(e) => {
                error!("Failed to reset branch: {}", e);
                Err(anyhow::anyhow!("Failed to reset branch: {}", e))
            }
        }
    }

    async fn handle_optimize(&self, request: OptimizeTool) -> Result<serde_json::Value> {
        info!("Optimizing database at path: {}", request.path);

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        // Execute the optimize operation
        match client.optimize(&request.path).await {
            Ok(response) => {
                Ok(serde_json::json!({
                    "status": "success",
                    "path": request.path,
                    "api_response": response,
                    "message": format!("Successfully optimized database at path: {}", request.path)
                }))
            }
            Err(e) => {
                error!("Failed to optimize database: {}", e);
                Err(anyhow::anyhow!("Failed to optimize database: {}", e))
            }
        }
    }

    async fn handle_get_graphql_schema(&self, request: GetGraphQLSchemaTool) -> Result<serde_json::Value> {
        info!("Retrieving GraphQL schema for database: {}", request.database);

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        // Use the branch from request or default to "main"
        let branch = request.branch.as_deref();

        // Introspect the GraphQL schema
        let schema = client.introspect_schema(&request.database, branch).await?;

        // Determine output path
        let output_path = request.output_path.unwrap_or_else(|| "./schema.json".to_string());

        // Convert schema to pretty JSON string
        let schema_json = serde_json::to_string_pretty(&schema)?;

        // Write to file
        tokio::fs::write(&output_path, &schema_json).await
            .map_err(|e| anyhow::anyhow!("Failed to write schema to file: {}", e))?;

        // Get file size for reporting
        let file_size = schema_json.len();

        // Create a preview of the schema (first 500 chars)
        let preview_len = schema_json.len().min(500);
        let preview = &schema_json[..preview_len];

        Ok(serde_json::json!({
            "status": "success",
            "database": request.database,
            "branch": branch.unwrap_or("main"),
            "output_path": output_path,
            "file_size_bytes": file_size,
            "preview": preview,
            "message": format!("GraphQL schema downloaded successfully to: {}", output_path)
        }))
    }

    async fn handle_clone(&self, request: CloneTool) -> Result<serde_json::Value> {
        info!("Cloning repository: {} to {}/{}", request.remote_url, request.organization, request.database);

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        // Prepare remote authentication if provided
        let remote_auth = match (request.remote_username.as_deref(), request.remote_password.as_deref()) {
            (Some(username), Some(password)) => Some((username, password)),
            _ => None,
        };

        let result = client.clone_repository(
            &request.organization,
            &request.database,
            &request.remote_url,
            request.label.as_deref(),
            request.comment.as_deref(),
            remote_auth,
        ).await?;

        Ok(serde_json::json!({
            "status": "success",
            "organization": request.organization,
            "database": request.database,
            "remote_url": request.remote_url,
            "result": result,
            "message": format!("Successfully cloned repository to {}/{}", request.organization, request.database)
        }))
    }

    async fn handle_fetch(&self, request: FetchTool) -> Result<serde_json::Value> {
        info!("Fetching from remote: {} into path: {}", request.remote_url, request.path);

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        // Prepare remote authentication if provided
        let remote_auth = match (request.remote_username.as_deref(), request.remote_password.as_deref()) {
            (Some(username), Some(password)) => Some((username, password)),
            _ => None,
        };

        let result = client.fetch(
            &request.path,
            &request.remote_url,
            request.remote_branch.as_deref(),
            remote_auth
        ).await?;

        Ok(serde_json::json!({
            "status": "success",
            "path": request.path,
            "remote_url": request.remote_url,
            "remote_branch": request.remote_branch.as_deref().unwrap_or("main"),
            "result": result,
            "message": format!("Successfully fetched from remote into {}", request.path)
        }))
    }

    async fn handle_push(&self, request: PushTool) -> Result<serde_json::Value> {
        info!("Pushing from path: {} to remote: {}", request.path, request.remote_url);

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        // Prepare remote authentication if provided
        let remote_auth = match (request.remote_username.as_deref(), request.remote_password.as_deref()) {
            (Some(username), Some(password)) => Some((username, password)),
            _ => None,
        };

        let result = client.push(
            &request.path,
            &request.remote_url,
            request.remote_branch.as_deref(),
            remote_auth,
        ).await?;

        Ok(serde_json::json!({
            "status": "success",
            "path": request.path,
            "remote_url": request.remote_url,
            "remote_branch": request.remote_branch,
            "result": result,
            "message": format!("Successfully pushed from {} to remote", request.path)
        }))
    }

    async fn handle_pull(&self, request: PullTool) -> Result<serde_json::Value> {
        info!("Pulling into path: {} from remote: {}", request.path, request.remote_url);

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        // Prepare remote authentication if provided
        let remote_auth = match (request.remote_username.as_deref(), request.remote_password.as_deref()) {
            (Some(username), Some(password)) => Some((username, password)),
            _ => None,
        };

        let result = client.pull(
            &request.path,
            &request.remote_url,
            request.remote_branch.as_deref(),
            &request.author,
            &request.message,
            remote_auth,
        ).await?;

        Ok(serde_json::json!({
            "status": "success",
            "path": request.path,
            "remote_url": request.remote_url,
            "remote_branch": request.remote_branch,
            "author": request.author,
            "commit_message": request.message,
            "result": result,
            "message": format!("Successfully pulled from remote into {}", request.path)
        }))
    }

    /// Helper to extract database path and remote name from MCP path format
    /// Expects path like "org/db/remote/remote_name" and returns ("org/db", "remote_name")
    fn split_remote_path(path: &str) -> Result<(&str, &str)> {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() < 4 || parts[parts.len() - 2] != "remote" {
            return Err(anyhow::anyhow!(
                "Invalid remote path format. Expected 'org/db/remote/remote_name', got '{}'",
                path
            ));
        }

        let remote_name = parts[parts.len() - 1];
        let db_path = parts[..parts.len() - 2].join("/");

        Ok((Box::leak(db_path.into_boxed_str()), remote_name))
    }

    async fn handle_add_remote(&self, request: AddRemoteTool) -> Result<serde_json::Value> {
        info!("Adding remote at path: {} with URL: {}", request.path, request.remote_url);

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        let (db_path, remote_name) = Self::split_remote_path(&request.path)?;
        let result = client.add_remote(db_path, remote_name, &request.remote_url).await?;

        Ok(serde_json::json!({
            "status": "success",
            "path": request.path,
            "remote_url": request.remote_url,
            "result": result,
            "message": format!("Successfully added remote at {}", request.path)
        }))
    }

    async fn handle_get_remote(&self, request: GetRemoteTool) -> Result<serde_json::Value> {
        info!("Getting remote information for path: {}", request.path);

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        let (db_path, remote_name) = Self::split_remote_path(&request.path)?;
        let result = client.get_remote(db_path, remote_name).await?;

        Ok(serde_json::json!({
            "status": "success",
            "path": request.path,
            "remote_info": result,
            "message": format!("Successfully retrieved remote information for {}", request.path)
        }))
    }

    async fn handle_update_remote(&self, request: UpdateRemoteTool) -> Result<serde_json::Value> {
        info!("Updating remote at path: {} with new URL: {}", request.path, request.remote_url);

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        let (db_path, remote_name) = Self::split_remote_path(&request.path)?;
        let result = client.update_remote(db_path, remote_name, &request.remote_url).await?;

        Ok(serde_json::json!({
            "status": "success",
            "path": request.path,
            "remote_url": request.remote_url,
            "result": result,
            "message": format!("Successfully updated remote at {}", request.path)
        }))
    }

    async fn handle_delete_remote(&self, request: DeleteRemoteTool) -> Result<serde_json::Value> {
        info!("Deleting remote at path: {}", request.path);

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        let (db_path, remote_name) = Self::split_remote_path(&request.path)?;
        let result = client.delete_remote(db_path, remote_name).await?;

        Ok(serde_json::json!({
            "status": "success",
            "path": request.path,
            "result": result,
            "message": format!("Successfully deleted remote at {}", request.path)
        }))
    }
}

#[async_trait]
impl ServerHandler for TerminusDBMcpHandler {
    async fn handle_list_tools_request(
        &self,
        _request: ListToolsRequest,
        _runtime: &dyn McpServer,
    ) -> Result<ListToolsResult, RpcError> {
        Ok(ListToolsResult {
            tools: vec![
                ConnectTool::tool(),
                ExecuteWoqlTool::tool(),
                ListDatabasesTool::tool(),
                GetSchemaTool::tool(),
                CheckServerStatusTool::tool(),
                ResetDatabaseTool::tool(),
                GetDocumentTool::tool(),
                QueryLogTool::tool(),
                DeleteClassesTool::tool(),
                SquashTool::tool(),
                ResetTool::tool(),
                OptimizeTool::tool(),
                GetGraphQLSchemaTool::tool(),
                // Collaboration operations
                CloneTool::tool(),
                FetchTool::tool(),
                PushTool::tool(),
                PullTool::tool(),
                // Remote management operations
                AddRemoteTool::tool(),
                GetRemoteTool::tool(),
                UpdateRemoteTool::tool(),
                DeleteRemoteTool::tool(),
                // Temporarily disabled due to serde_json::Value schema issues
                // InsertDocumentTool::tool(),
                // InsertDocumentsTool::tool(),
                // ReplaceDocumentTool::tool(),
            ],
            meta: None,
            next_cursor: None,
        })
    }

    async fn handle_call_tool_request(
        &self,
        request: CallToolRequest,
        _runtime: &dyn McpServer,
    ) -> Result<CallToolResult, CallToolError> {
        let tool_name = &request.params.name;
        let args = request.params.arguments.clone().unwrap_or_default();

        match tool_name.as_str() {
            name if name == ConnectTool::tool_name() => {
                let tool_request: ConnectTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.connect(tool_request).await {
                    Ok(result) => {
                        // Convert to pretty string for text content
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        // Extract as object for structured content if possible
                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == ExecuteWoqlTool::tool_name() => {
                let tool_request: ExecuteWoqlTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.execute_woql(tool_request).await {
                    Ok(result) => {
                        // Convert to pretty string for text content
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        // Extract as object for structured content if possible
                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == ListDatabasesTool::tool_name() => {
                let tool_request: ListDatabasesTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.list_databases(tool_request).await {
                    Ok(result) => {
                        // Convert to pretty string for text content
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        // Extract as object for structured content if possible
                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == GetSchemaTool::tool_name() => {
                let tool_request: GetSchemaTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.get_schema(tool_request).await {
                    Ok(result) => {
                        // Convert to pretty string for text content
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        // Extract as object for structured content if possible
                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == CheckServerStatusTool::tool_name() => {
                let tool_request: CheckServerStatusTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.check_server_status(tool_request).await {
                    Ok(result) => {
                        // Convert to pretty string for text content
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        // Extract as object for structured content if possible
                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == ResetDatabaseTool::tool_name() => {
                let tool_request: ResetDatabaseTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.reset_database(tool_request).await {
                    Ok(result) => {
                        // Convert to pretty string for text content
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        // Extract as object for structured content if possible
                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == GetDocumentTool::tool_name() => {
                let tool_request: GetDocumentTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.get_document(tool_request).await {
                    Ok(result) => {
                        // Convert to pretty string for text content
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        // Extract as object for structured content if possible
                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == QueryLogTool::tool_name() => {
                let tool_request: QueryLogTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_query_log(tool_request).await {
                    Ok(result) => {
                        // Convert to pretty string for text content
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        // Extract as object for structured content if possible
                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == InsertDocumentTool::tool_name() => {
                let tool_request: InsertDocumentTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_insert_document(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == InsertDocumentsTool::tool_name() => {
                let tool_request: InsertDocumentsTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_insert_documents(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == ReplaceDocumentTool::tool_name() => {
                let tool_request: ReplaceDocumentTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_replace_document(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == DeleteClassesTool::tool_name() => {
                let tool_request: DeleteClassesTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_delete_classes(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == SquashTool::tool_name() => {
                let tool_request: SquashTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_squash(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == ResetTool::tool_name() => {
                let tool_request: ResetTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_reset(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == OptimizeTool::tool_name() => {
                let tool_request: OptimizeTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_optimize(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == GetGraphQLSchemaTool::tool_name() => {
                let tool_request: GetGraphQLSchemaTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_get_graphql_schema(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == CloneTool::tool_name() => {
                let tool_request: CloneTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_clone(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == FetchTool::tool_name() => {
                let tool_request: FetchTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_fetch(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == PushTool::tool_name() => {
                let tool_request: PushTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_push(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == PullTool::tool_name() => {
                let tool_request: PullTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_pull(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == AddRemoteTool::tool_name() => {
                let tool_request: AddRemoteTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_add_remote(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == GetRemoteTool::tool_name() => {
                let tool_request: GetRemoteTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_get_remote(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == UpdateRemoteTool::tool_name() => {
                let tool_request: UpdateRemoteTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_update_remote(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            name if name == DeleteRemoteTool::tool_name() => {
                let tool_request: DeleteRemoteTool =
                    serde_json::from_value(serde_json::Value::Object(args))
                        .map_err(|e| CallToolError::new(e))?;

                match self.handle_delete_remote(tool_request).await {
                    Ok(result) => {
                        let text_content = serde_json::to_string_pretty(&result)
                            .unwrap_or_else(|_| result.to_string());

                        let structured = match result {
                            serde_json::Value::Object(map) => Some(map),
                            _ => None,
                        };

                        Ok(CallToolResult {
                            content: vec![TextContent::new(text_content, None, None).into()],
                            is_error: None,
                            meta: None,
                            structured_content: structured,
                        })
                    }
                    Err(e) => Err(CallToolError::new(McpError(e))),
                }
            }
            _ => Err(CallToolError::unknown_tool(tool_name.to_string())),
        }
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // info!("Starting TerminusDB MCP Server");

    // Create server details
    let server_details = InitializeResult {
        server_info: Implementation {
            name: "TerminusDB MCP Server".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            title: Some("TerminusDB MCP Server".to_string()),
        },
        capabilities: ServerCapabilities {
            tools: Some(Default::default()),
            ..Default::default()
        },
        protocol_version: "2025-06-18".to_string(),
        instructions: Some(
            "This server provides access to TerminusDB via WOQL DSL queries. \
            Use execute_woql to run queries, list_databases to see available databases, \
            get_schema to inspect database schemas, and check_server_status to verify \
            the TerminusDB server is running and accessible."
                .to_string(),
        ),
        meta: None,
    };

    // Create transport
    let transport = StdioTransport::new(TransportOptions::default())?;

    // Create handler
    let handler = TerminusDBMcpHandler::new();

    // Create and start server
    let server = server_runtime::create_server(server_details, transport, handler);
    server.start().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn test_woql_json_wrapping() {
        // Test that the wrapping logic works correctly
        let original_json = serde_json::json!({
            "@type": "Select",
            "variables": ["Doc"],
            "query": {
                "@type": "And",
                "and": []
            }
        });
        
        let mut json_value = original_json.clone();
        
        // Check if needs wrapping
        let needs_wrapping = json_value.get("@type")
            .and_then(|t| t.as_str())
            .map(|t| t != "Query")
            .unwrap_or(false);
            
        assert!(needs_wrapping);
        
        // Apply wrapping
        if let Some(query_type) = json_value.get("@type").and_then(|t| t.as_str()) {
            let mut wrapper = serde_json::Map::new();
            wrapper.insert("@type".to_string(), serde_json::Value::String("Query".to_string()));
            wrapper.insert(query_type.to_lowercase(), json_value);
            json_value = serde_json::Value::Object(wrapper);
        }
        
        // Verify the wrapped structure
        assert_eq!(json_value.get("@type").and_then(|v| v.as_str()), Some("Query"));
        assert!(json_value.get("select").is_some());
        
        // The wrapped JSON should now be ready for deserialization
        // Note: The actual deserialization may still fail due to how FromTDBInstance
        // handles abstract tagged unions, but the wrapping structure is correct
    }
    
    #[test]
    fn test_complex_woql_query_json_ld() {
        // This test verifies that complex JSON-LD queries can be handled
        // by the execute_woql function without deserialization
        let query_json = json!({
            "@type": "Select",
            "query": {
                "@type": "And",
                "and": [
                    {
                        "@type": "OrderBy",
                        "ordering": [
                            {
                                "@type": "OrderTemplate",
                                "order": "asc",
                                "variable": "CreatedBy"
                            }
                        ],
                        "query": {
                            "@type": "And",
                            "and": [
                                {
                                    "@type": "Triple",
                                    "graph": "instance",
                                    "object": {
                                        "@type": "Value",
                                        "node": "@schema:AwsDBPublication"
                                    },
                                    "predicate": {
                                        "@type": "NodeValue",
                                        "node": "rdf:type"
                                    },
                                    "subject": {
                                        "@type": "NodeValue",
                                        "variable": "Subject"
                                    }
                                },
                                {
                                    "@type": "Triple",
                                    "graph": "instance",
                                    "object": {
                                        "@type": "Value",
                                        "node": "@schema:AwsDBPublication"
                                    },
                                    "predicate": {
                                        "@type": "NodeValue",
                                        "node": "rdf:type"
                                    },
                                    "subject": {
                                        "@type": "NodeValue",
                                        "variable": "Subject"
                                    }
                                },
                                {
                                    "@type": "Triple",
                                    "graph": "instance",
                                    "object": {
                                        "@type": "Value",
                                        "variable": "Title"
                                    },
                                    "predicate": {
                                        "@type": "NodeValue",
                                        "node": "title"
                                    },
                                    "subject": {
                                        "@type": "NodeValue",
                                        "variable": "Subject"
                                    }
                                },
                                {
                                    "@type": "Triple",
                                    "graph": "instance",
                                    "object": {
                                        "@type": "Value",
                                        "variable": "CreatedOn"
                                    },
                                    "predicate": {
                                        "@type": "NodeValue",
                                        "node": "created_on"
                                    },
                                    "subject": {
                                        "@type": "NodeValue",
                                        "variable": "Subject"
                                    }
                                },
                                {
                                    "@type": "Triple",
                                    "graph": "instance",
                                    "object": {
                                        "@type": "Value",
                                        "variable": "Title"
                                    },
                                    "predicate": {
                                        "@type": "NodeValue",
                                        "node": "title"
                                    },
                                    "subject": {
                                        "@type": "NodeValue",
                                        "variable": "Subject"
                                    }
                                },
                                {
                                    "@type": "Lower",
                                    "lower": {
                                        "@type": "DataValue",
                                        "variable": "LowerTitle"
                                    },
                                    "mixed": {
                                        "@type": "DataValue",
                                        "variable": "Title"
                                    }
                                },
                                {
                                    "@type": "Regexp",
                                    "pattern": {
                                        "@type": "DataValue",
                                        "data": ".*alpha.*"
                                    },
                                    "result": null,
                                    "string": {
                                        "@type": "DataValue",
                                        "variable": "LowerTitle"
                                    }
                                }
                            ]
                        }
                    },
                    {
                        "@type": "ReadDocument",
                        "document": {
                            "@type": "Value",
                            "variable": "Doc"
                        },
                        "identifier": {
                            "@type": "NodeValue",
                            "variable": "Subject"
                        }
                    }
                ]
            },
            "variables": [
                "Doc"
            ]
        });

        // Test that the JSON can be used directly without wrapping or deserialization
        let json_string = serde_json::to_string(&query_json).unwrap();
        
        // Simulate what execute_woql does - parse the JSON string
        let parsed_json = serde_json::from_str::<serde_json::Value>(&json_string).unwrap();
        
        // Verify that the JSON has the expected structure
        assert_eq!(parsed_json.get("@type").and_then(|v| v.as_str()), Some("Select"));
        assert!(parsed_json.get("variables").is_some());
        assert!(parsed_json.get("query").is_some());
        
        // Verify the nested structure
        let query_obj = parsed_json.get("query").unwrap();
        assert_eq!(query_obj.get("@type").and_then(|v| v.as_str()), Some("And"));
        
        let and_array = query_obj.get("and").and_then(|v| v.as_array()).unwrap();
        assert_eq!(and_array.len(), 2);
        
        // First element should be an OrderBy
        assert_eq!(and_array[0].get("@type").and_then(|v| v.as_str()), Some("OrderBy"));
        
        // Second element should be a ReadDocument
        assert_eq!(and_array[1].get("@type").and_then(|v| v.as_str()), Some("ReadDocument"));
        
        // The JSON should be ready to send to the API without any transformation
        println!("JSON-LD query ready for API: {}", serde_json::to_string_pretty(&parsed_json).unwrap())
    }
}
