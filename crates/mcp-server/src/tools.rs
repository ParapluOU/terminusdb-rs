//! MCP tool definitions (request payloads) for the TerminusDB MCP server.

use crate::config::ConnectionConfig;
use rust_mcp_sdk::macros::{mcp_tool, JsonSchema};
use serde::Deserialize;

// Tool definitions using mcp_tool macro
#[derive(Debug, Deserialize, JsonSchema)]
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

#[derive(Debug, Deserialize, JsonSchema)]
#[mcp_tool(
    name = "execute_woql",
    description = "Execute a WOQL query using JavaScript syntax or JSON-LD format.\n\n\
        **JavaScript Syntax** (compatible with terminusdb-client-js):\n\
        - Variables in queries use \"v:\" prefix: \"v:Person\", \"v:Name\"\n\
        - Variables in select/distinct are strings without prefix: select(\"Name\", \"Age\", ...)\n\
        - Example: select(\"Name\", triple(\"v:Person\", \"@schema:name\", \"v:Name\"))\n\
        - Functions: triple(), and(), or(), not(), opt(), select(), distinct(), limit(), greater(), less(), eq()\n\n\
        **JSON-LD Format**:\n\
        - Full JSON-LD object following the WOQL schema\n\
        - Example: {\"@type\": \"Triple\", \"subject\": {\"@type\": \"NodeValue\", \"variable\": \"Person\"}, ...}\n\n\
        The tool auto-detects the format. For mutating queries (add_triple, delete_triple, insert_document, etc.), provide 'author' and 'message' parameters for commit tracking.\n\n\
        For more documentation and examples, see the ./docs directory in the terminusdb-rs crate source."
)]
pub struct ExecuteWoqlTool {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
    /// Optional timeout in seconds (overrides default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
    /// Author of the changes (for mutating queries)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Commit message describing the changes (for mutating queries)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[mcp_tool(name = "list_databases", description = "List all available databases")]
pub struct ListDatabasesTool {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[mcp_tool(
    name = "get_schema",
    description = "Get the schema for a specific database"
)]
pub struct GetSchemaTool {
    pub database: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
    /// Optional timeout in seconds (overrides default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[mcp_tool(
    name = "check_server_status",
    description = "Check if the TerminusDB server is running and accessible"
)]
pub struct CheckServerStatusTool {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[mcp_tool(
    name = "reset_database",
    description = "Reset a database by deleting and recreating it. WARNING: This permanently deletes all data in the database!"
)]
pub struct ResetDatabaseTool {
    pub database: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
}

#[derive(Debug, Deserialize, JsonSchema)]
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

#[derive(Debug, Deserialize, JsonSchema)]
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

#[derive(Debug, Deserialize, JsonSchema)]
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

#[derive(Debug, Deserialize, JsonSchema)]
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

#[derive(Debug, Deserialize, JsonSchema)]
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

#[derive(Debug, Deserialize, JsonSchema)]
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

#[derive(Debug, Deserialize, JsonSchema)]
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
    /// Optional timeout in seconds (overrides default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
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

#[derive(Debug, Deserialize, JsonSchema)]
#[mcp_tool(
    name = "optimize",
    description = "Optimize a database by removing unreachable data. This can significantly reduce database size and improve performance."
)]
pub struct OptimizeTool {
    /// Path to optimize (e.g., "admin/mydb/_meta" or "admin/mydb/local/branch/main")
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionConfig>,
    /// Optional timeout in seconds (overrides default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
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
    /// Optional timeout in seconds (overrides default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

/// Clone a remote repository to create a new local database
#[derive(Debug, Deserialize, JsonSchema)]
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
    /// Optional timeout in seconds (overrides default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

/// Fetch changes from a remote repository
#[derive(Debug, Deserialize, JsonSchema)]
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
    /// Optional timeout in seconds (overrides default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

/// Push changes to a remote repository
#[derive(Debug, Deserialize, JsonSchema)]
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
    /// Optional timeout in seconds (overrides default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

/// Pull changes from a remote repository (fetch + merge)
#[derive(Debug, Deserialize, JsonSchema)]
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
    /// Optional timeout in seconds (overrides default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

/// Add a new remote repository
#[derive(Debug, Deserialize, JsonSchema)]
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
#[derive(Debug, Deserialize, JsonSchema)]
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
#[derive(Debug, Deserialize, JsonSchema)]
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
#[derive(Debug, Deserialize, JsonSchema)]
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

fn default_true() -> bool {
    true
}

/// Start a local TerminusDB server
#[derive(Debug, Deserialize, JsonSchema)]
#[mcp_tool(
    name = "start_local_server",
    description = "Start a local TerminusDB server instance for testing and development. \
                   The server runs as a separate process on http://localhost:6363. \
                   Returns a server_id that can be used to stop the server or configure connections."
)]
pub struct StartLocalServerTool {
    /// Server ID for tracking (optional, auto-generated if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_id: Option<String>,

    /// Run server in memory-only mode (no persistence)
    #[serde(default)]
    pub memory: bool,

    /// Admin password (defaults to "root")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    /// Suppress server stdout/stderr
    #[serde(default = "default_true")]
    pub quiet: bool,

    /// Automatically set as default connection for subsequent operations
    #[serde(default)]
    pub set_as_default: bool,
}

/// Stop a local TerminusDB server
#[derive(Debug, Deserialize, JsonSchema)]
#[mcp_tool(
    name = "stop_local_server",
    description = "Stop a running local TerminusDB server instance by its server_id. \
                   The server process will be terminated and all resources cleaned up."
)]
pub struct StopLocalServerTool {
    /// Server ID to stop (required)
    pub server_id: String,
}

/// List running local TerminusDB servers
#[derive(Debug, Deserialize, JsonSchema)]
#[mcp_tool(
    name = "list_local_servers",
    description = "List all running local TerminusDB server instances managed by this MCP server."
)]
pub struct ListLocalServersTool {}
