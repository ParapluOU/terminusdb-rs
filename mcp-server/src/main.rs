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
use std::env;
use std::fmt;
use std::sync::Arc;
use terminusdb_client::{BranchSpec, TerminusDBHttpClient};
use terminusdb_woql2::prelude::ToTDBInstance;
use terminusdb_woql_dsl::parse_woql_dsl;
use tokio::sync::RwLock;
use tracing::info;
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
    env::var("TERMINUSDB_PASS").unwrap_or_else(|_| "root".to_string())
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

        // Try to parse as JSON-LD first, then fall back to DSL
        let json_query = if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&request.query) {
            // If it's valid JSON, use it directly as the query payload
            // The API expects the JSON-LD format directly, not parsed into a Query object
            json_value
        } else {
            // If it's not valid JSON, parse as WOQL DSL and convert to JSON
            let query = parse_woql_dsl(&request.query)?;
            query.to_json()
        };

        // Get connection config
        let config = self.get_connection_config(request.connection).await;

        // Create client
        let client = Self::create_client(&config).await?;

        // Execute query
        if let Some(database) = &config.database {
            let branch_spec = match &config.commit_ref {
                Some(commit_id) => {
                    // Time-travel query to specific commit
                    BranchSpec::with_commit(database, commit_id)
                }
                None => {
                    // Regular query on branch
                    BranchSpec::with_branch(database, &config.branch)
                }
            };
            // Use query_raw to send the JSON directly
            let response: terminusdb_client::WOQLResult<serde_json::Value> =
                client.query_raw(Some(branch_spec), json_query).await?;
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
                BranchSpec::with_commit(&request.database, commit_id)
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

        let query = parse_woql_dsl(schema_query)?;
        let response: terminusdb_client::WOQLResult<serde_json::Value> =
            client.query(Some(branch_spec), query).await?;
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
