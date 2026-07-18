//! Local TerminusDB server lifecycle handlers (start / stop / list).

use crate::config::ConnectionConfig;
use crate::handler::{ManagedServer, TerminusDBMcpHandler};
use crate::tools::*;
use anyhow::Result;
use std::sync::Arc;
use terminusdb_bin::{start_server, ServerOptions};
use tracing::info;

impl TerminusDBMcpHandler {
    pub(crate) async fn handle_start_local_server(
        &self,
        request: StartLocalServerTool,
    ) -> Result<serde_json::Value> {
        // Generate server ID if not provided
        let server_id = request.server_id.unwrap_or_else(|| {
            use std::time::{SystemTime, UNIX_EPOCH};
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            format!("server_{}", ts)
        });

        info!("Starting local server with ID: {}", server_id);

        // Check if server with this ID already exists
        {
            let servers = self.managed_servers.read().await;
            if servers.contains_key(&server_id) {
                return Err(anyhow::anyhow!(
                    "Server with ID '{}' is already running",
                    server_id
                ));
            }
        }

        // Create server with options
        let password = request.password.unwrap_or_else(|| "root".to_string());
        let opts = ServerOptions {
            memory: request.memory,
            password: Some(password.clone()),
            quiet: request.quiet,
            db_path: None,
            port: None, // Auto-allocate port
            test_mode: false,
            workers: None,
            request_timeout: None,
        };

        // Start the server
        let server = start_server(opts)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to start server: {}", e))?;

        // Get connection info - TerminusDB always runs on localhost:6363
        let url = "http://localhost:6363".to_string();
        let username = "admin";

        // Store server handle with connection info
        {
            let mut servers = self.managed_servers.write().await;
            servers.insert(
                server_id.clone(),
                ManagedServer {
                    server: Arc::new(server),
                    url: url.clone(),
                    password: password.clone(),
                },
            );
        }

        // Auto-configure connection if requested
        if request.set_as_default {
            let mut config = self.saved_config.write().await;
            *config = Some(ConnectionConfig {
                host: url.clone(),
                user: username.to_string(),
                password: password.clone(),
                database: None,
                branch: "main".to_string(),
                commit_ref: None,
            });
            info!("Set local server as default connection");
        }

        Ok(serde_json::json!({
            "server_id": server_id,
            "url": url,
            "username": username,
            "password": password,
            "status": "running"
        }))
    }

    pub(crate) async fn handle_stop_local_server(
        &self,
        request: StopLocalServerTool,
    ) -> Result<serde_json::Value> {
        info!("Stopping local server with ID: {}", request.server_id);

        let mut servers = self.managed_servers.write().await;

        match servers.remove(&request.server_id) {
            Some(managed) => {
                // Check if this is the last reference
                let ref_count = Arc::strong_count(&managed.server);

                // Drop our reference - if it's the last one, server will stop
                drop(managed);

                Ok(serde_json::json!({
                    "server_id": request.server_id,
                    "status": "stopped",
                    "message": if ref_count == 1 {
                        "Server stopped and cleaned up".to_string()
                    } else {
                        format!("Reference removed ({} references remaining)", ref_count - 1)
                    }
                }))
            }
            None => Err(anyhow::anyhow!(
                "Server with ID '{}' not found",
                request.server_id
            )),
        }
    }

    pub(crate) async fn handle_list_local_servers(
        &self,
        _request: ListLocalServersTool,
    ) -> Result<serde_json::Value> {
        info!("Listing local servers");

        let servers = self.managed_servers.read().await;

        let server_list: Vec<_> = servers
            .iter()
            .map(|(id, managed)| {
                serde_json::json!({
                    "server_id": id,
                    "url": managed.url,
                    "status": "running"
                })
            })
            .collect();

        Ok(serde_json::json!({
            "servers": server_list,
            "count": server_list.len()
        }))
    }
}
