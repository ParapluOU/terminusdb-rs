//! Core MCP handler: connection state, managed-server registry, and the
//! foundational request handlers (connect, query, schema, document reads).

use crate::config::ConnectionConfig;
use crate::tools::*;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::debug::QueryLogEntry;
use terminusdb_client::{BranchSpec, GetOpts, TerminusDBHttpClient};
use terminusdb_woql2::prelude::{FromTDBInstance, Query};
use tokio::sync::RwLock;
use tracing::{info, warn};
use url::Url;

pub(crate) struct ManagedServer {
    pub(crate) server: Arc<TerminusDBServer>,
    pub(crate) url: String,
    pub(crate) password: String,
}

pub struct TerminusDBMcpHandler {
    pub(crate) saved_config: Arc<RwLock<Option<ConnectionConfig>>>,
    pub(crate) managed_servers: Arc<RwLock<HashMap<String, ManagedServer>>>,
}

impl TerminusDBMcpHandler {
    pub(crate) fn new() -> Self {
        Self {
            saved_config: Arc::new(RwLock::new(None)),
            managed_servers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub(crate) async fn create_client(config: &ConnectionConfig) -> Result<TerminusDBHttpClient> {
        let url = Url::parse(&config.host)?;
        TerminusDBHttpClient::new(
            url,
            &config.user,
            &config.password,
            &config.user, // org defaults to user
        )
        .await
    }

    pub(crate) async fn get_connection_config(&self, provided: Option<ConnectionConfig>) -> ConnectionConfig {
        if let Some(config) = provided {
            return config;
        }

        if let Some(config) = self.saved_config.read().await.clone() {
            return config;
        }

        ConnectionConfig::default()
    }

    pub(crate) async fn connect(&self, request: ConnectTool) -> Result<serde_json::Value> {
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

    pub(crate) async fn execute_woql(&self, request: ExecuteWoqlTool) -> Result<serde_json::Value> {
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

            // Convert timeout from seconds to Duration if provided
            let timeout = request.timeout_seconds.map(std::time::Duration::from_secs);

            // Check if this is a mutating query (author or message provided)
            let response: terminusdb_client::WOQLResult<serde_json::Value> =
                if request.author.is_some() || request.message.is_some() {
                    // Mutating query - use query_mut_string (a wrapper we'll need to add)
                    let author = request.author.unwrap_or_else(|| "system".to_string());
                    let message = request
                        .message
                        .unwrap_or_else(|| "execute query".to_string());

                    info!(
                        "Executing mutating query with author: {}, message: {}",
                        author, message
                    );

                    // Parse query using the client's existing logic, then call query_mut
                    let query = if let Ok(json_value) =
                        serde_json::from_str::<serde_json::Value>(&request.query)
                    {
                        // Try parsing as JSON-LD
                        Query::from_json(json_value)
                            .map_err(|e| anyhow::anyhow!("Failed to parse JSON-LD query: {}", e))?
                    } else {
                        // Try parsing as JS syntax using woql-js
                        terminusdb_woql_js::parse_js_woql_to_query(&request.query).map_err(|e| {
                            anyhow::anyhow!("Failed to parse JavaScript syntax: {}", e)
                        })?
                    };

                    client
                        .query_mut(branch_spec, query, author, message)
                        .await?
                } else {
                    // Read-only query - use existing query_string
                    client
                        .query_string(Some(branch_spec), &request.query, timeout)
                        .await?
                };

            Ok(serde_json::to_value(&response)?)
        } else {
            Err(anyhow::anyhow!("Database must be specified"))
        }
    }

    pub(crate) async fn list_databases(&self, request: ListDatabasesTool) -> Result<serde_json::Value> {
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

    pub(crate) async fn get_schema(&self, request: GetSchemaTool) -> Result<serde_json::Value> {
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

        // Convert timeout from seconds to Duration if provided
        let timeout = request.timeout_seconds.map(std::time::Duration::from_secs);

        let response: terminusdb_client::WOQLResult<serde_json::Value> = client
            .query_string(Some(branch_spec), schema_query, timeout)
            .await?;
        Ok(serde_json::to_value(&response)?)
    }

    pub(crate) async fn check_server_status(
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

    pub(crate) async fn reset_database(&self, request: ResetDatabaseTool) -> Result<serde_json::Value> {
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

    pub(crate) async fn get_document(&self, request: GetDocumentTool) -> Result<serde_json::Value> {
        info!("Retrieving document: {}", request.document_id);

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        if config.database.is_none() {
            return Err(anyhow::anyhow!("Database must be specified"));
        }

        let db = config.database.unwrap();
        let branch_spec = match &config.commit_ref {
            Some(commit_id) => BranchSpec::with_commit(&db, commit_id.as_str()),
            None => BranchSpec::with_branch(&db, &config.branch),
        };

        // Format document ID if type_name is provided
        let formatted_id = match &request.type_name {
            Some(type_name) if !request.document_id.contains('/') => {
                format!("{}/{}", type_name, request.document_id)
            }
            _ => request.document_id.clone(),
        };

        let opts = GetOpts::default()
            .with_unfold(request.unfold)
            .with_as_list(request.as_list);

        if request.include_headers {
            let result = client
                .get_document_with_headers(&formatted_id, &branch_spec, opts)
                .await?;
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
            let document = client
                .get_document(&formatted_id, &branch_spec, opts)
                .await?;
            Ok(document)
        }
    }

    pub(crate) async fn handle_query_log(&self, request: QueryLogTool) -> Result<serde_json::Value> {
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
                let path = request
                    .log_path
                    .as_deref()
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

                Err(anyhow::anyhow!(
                    "Query log rotation is temporarily unavailable due to client implementation"
                ))?
            }

            "view" => {
                // For now, we'll use the default path or the one from request
                let path = request
                    .log_path
                    .as_deref()
                    .unwrap_or("/tmp/terminusdb_queries.log");

                // Read the log file
                let content = tokio::fs::read_to_string(&path)
                    .await
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
                let limit = request
                    .limit
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(20);
                let offset = request
                    .offset
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);

                let total = filtered.len();
                let paginated: Vec<_> = filtered.into_iter().skip(offset).take(limit).collect();

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
            _ => Err(anyhow::anyhow!(
                "Invalid action: {}. Must be one of: status, enable, disable, rotate, view",
                action
            ))?,
        }
    }
}
