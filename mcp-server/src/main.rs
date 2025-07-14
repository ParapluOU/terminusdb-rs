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
use terminusdb_client::{BranchSpec, TerminusDBHttpClient};
use terminusdb_woql_dsl::parse_woql_dsl;
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

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
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
    env::var("TERMINUSDB_PASSWORD").unwrap_or_else(|_| "root".to_string())
}

fn default_branch() -> String {
    "main".to_string()
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            user: default_user(),
            password: default_password(),
            database: None,
            branch: default_branch(),
            commit_ref: None,
        }
    }
}

// Tool definitions using mcp_tool macro
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "execute_woql",
    description = "Execute a WOQL query using DSL syntax. The DSL uses a compositional, function-based syntax. Variables are prefixed with $ (e.g., $Person). Common operations include: triple($Subject, predicate, $Object), and(...), or(...), select([$Var1, $Var2], query), greater($Var, value). For full syntax and examples, see the woql-dsl crate source code and README.md in woql-dsl/."
)]
pub struct ExecuteWoqlTool {
    pub query: String,
    #[serde(default)]
    pub connection: ConnectionConfig,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(name = "list_databases", description = "List all available databases")]
pub struct ListDatabasesTool {
    #[serde(default)]
    pub connection: ConnectionConfig,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "get_schema",
    description = "Get the schema for a specific database"
)]
pub struct GetSchemaTool {
    pub database: String,
    #[serde(default)]
    pub connection: ConnectionConfig,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[mcp_tool(
    name = "check_server_status",
    description = "Check if the TerminusDB server is running and accessible"
)]
pub struct CheckServerStatusTool {
    #[serde(default)]
    pub connection: ConnectionConfig,
}

pub struct TerminusDBMcpHandler;

impl TerminusDBMcpHandler {
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

    async fn execute_woql(&self, request: ExecuteWoqlTool) -> Result<serde_json::Value> {
        info!("Executing WOQL query: {}", request.query);

        // Parse the WOQL DSL
        let query = parse_woql_dsl(&request.query)?;

        // Create client
        let client = Self::create_client(&request.connection).await?;

        // Execute query
        if let Some(database) = &request.connection.database {
            let branch_spec = match &request.connection.commit_ref {
                Some(commit_id) => {
                    // Time-travel query to specific commit
                    BranchSpec::with_commit(database, commit_id)
                }
                None => {
                    // Regular query on branch
                    BranchSpec::with_branch(database, &request.connection.branch)
                }
            };
            let response: terminusdb_client::WOQLResult<serde_json::Value> =
                client.query(Some(branch_spec), query).await?;
            Ok(serde_json::to_value(&response)?)
        } else {
            Err(anyhow::anyhow!("Database must be specified"))
        }
    }

    async fn list_databases(&self, request: ListDatabasesTool) -> Result<serde_json::Value> {
        info!("Listing databases");

        let client = Self::create_client(&request.connection).await?;
        
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

        let client = Self::create_client(&request.connection).await?;
        let branch_spec = match &request.connection.commit_ref {
            Some(commit_id) => {
                // Time-travel query to specific commit
                BranchSpec::with_commit(&request.database, commit_id)
            }
            None => {
                // Regular query on branch
                BranchSpec::with_branch(&request.database, &request.connection.branch)
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

        let client = Self::create_client(&request.connection).await?;

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
                ExecuteWoqlTool::tool(),
                ListDatabasesTool::tool(),
                GetSchemaTool::tool(),
                CheckServerStatusTool::tool(),
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
    let handler = TerminusDBMcpHandler;

    // Create and start server
    let server = server_runtime::create_server(server_details, transport, handler);
    server.start().await?;

    Ok(())
}
