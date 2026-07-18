//! Versioning, maintenance, and collaboration handlers:
//! squash, reset, optimize, GraphQL schema, clone/fetch/push/pull, remotes.

use crate::handler::TerminusDBMcpHandler;
use crate::tools::*;
use anyhow::Result;
use tracing::{error, info};

impl TerminusDBMcpHandler {
    pub(crate) async fn handle_squash(&self, request: SquashTool) -> Result<serde_json::Value> {
        info!("Squashing commits for path: {}", request.path);

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        // Convert timeout from seconds to Duration if provided
        let timeout = request.timeout_seconds.map(std::time::Duration::from_secs);

        // Execute the squash operation
        match client
            .squash(&request.path, &request.author, &request.message, timeout)
            .await
        {
            Ok(response) => Ok(serde_json::json!({
                "status": "success",
                "path": request.path,
                "new_commit": response.commit,
                "old_commit": response.old_commit,
                "api_status": format!("{:?}", response.status),
                "message": format!("Successfully squashed commits. New commit: {}", response.commit)
            })),
            Err(e) => {
                error!("Failed to squash commits: {}", e);
                Err(anyhow::anyhow!("Failed to squash commits: {}", e))
            }
        }
    }

    pub(crate) async fn handle_reset(&self, request: ResetTool) -> Result<serde_json::Value> {
        info!(
            "Resetting branch {} to {}",
            request.branch_path, request.commit_descriptor
        );

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        // Execute the reset operation
        match client
            .reset(&request.branch_path, &request.commit_descriptor)
            .await
        {
            Ok(response) => Ok(serde_json::json!({
                "status": "success",
                "branch_path": request.branch_path,
                "commit_descriptor": request.commit_descriptor,
                "api_response": response,
                "message": format!("Successfully reset branch {} to {}",
                    request.branch_path, request.commit_descriptor)
            })),
            Err(e) => {
                error!("Failed to reset branch: {}", e);
                Err(anyhow::anyhow!("Failed to reset branch: {}", e))
            }
        }
    }

    pub(crate) async fn handle_optimize(&self, request: OptimizeTool) -> Result<serde_json::Value> {
        info!("Optimizing database at path: {}", request.path);

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        // Convert timeout from seconds to Duration if provided
        let timeout = request.timeout_seconds.map(std::time::Duration::from_secs);

        // Execute the optimize operation
        match client.optimize(&request.path, timeout).await {
            Ok(response) => Ok(serde_json::json!({
                "status": "success",
                "path": request.path,
                "api_response": response,
                "message": format!("Successfully optimized database at path: {}", request.path)
            })),
            Err(e) => {
                error!("Failed to optimize database: {}", e);
                Err(anyhow::anyhow!("Failed to optimize database: {}", e))
            }
        }
    }

    pub(crate) async fn handle_get_graphql_schema(
        &self,
        request: GetGraphQLSchemaTool,
    ) -> Result<serde_json::Value> {
        info!(
            "Retrieving GraphQL schema for database: {}",
            request.database
        );

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        // Use the branch from request or default to "main"
        let branch = request.branch.as_deref();

        // Convert timeout from seconds to Duration if provided
        let timeout = request.timeout_seconds.map(std::time::Duration::from_secs);

        // Introspect the GraphQL schema
        let schema = client
            .introspect_schema(&request.database, branch, timeout)
            .await?;

        // Determine output path
        let output_path = request
            .output_path
            .unwrap_or_else(|| "./schema.json".to_string());

        // Convert schema to pretty JSON string
        let schema_json = serde_json::to_string_pretty(&schema)?;

        // Write to file
        tokio::fs::write(&output_path, &schema_json)
            .await
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

    pub(crate) async fn handle_clone(&self, request: CloneTool) -> Result<serde_json::Value> {
        info!(
            "Cloning repository: {} to {}/{}",
            request.remote_url, request.organization, request.database
        );

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        // Prepare remote authentication if provided
        let remote_auth = match (
            request.remote_username.as_deref(),
            request.remote_password.as_deref(),
        ) {
            (Some(username), Some(password)) => Some((username, password)),
            _ => None,
        };

        // Convert timeout from seconds to Duration if provided
        let timeout = request.timeout_seconds.map(std::time::Duration::from_secs);

        let result = client
            .clone_repository(
                &request.organization,
                &request.database,
                &request.remote_url,
                request.label.as_deref(),
                request.comment.as_deref(),
                remote_auth,
                timeout,
            )
            .await?;

        Ok(serde_json::json!({
            "status": "success",
            "organization": request.organization,
            "database": request.database,
            "remote_url": request.remote_url,
            "result": result,
            "message": format!("Successfully cloned repository to {}/{}", request.organization, request.database)
        }))
    }

    pub(crate) async fn handle_fetch(&self, request: FetchTool) -> Result<serde_json::Value> {
        info!(
            "Fetching from remote: {} into path: {}",
            request.remote_url, request.path
        );

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        // Prepare remote authentication if provided
        let remote_auth = match (
            request.remote_username.as_deref(),
            request.remote_password.as_deref(),
        ) {
            (Some(username), Some(password)) => Some((username, password)),
            _ => None,
        };

        // Convert timeout from seconds to Duration if provided
        let timeout = request.timeout_seconds.map(std::time::Duration::from_secs);

        let result = client
            .fetch(
                &request.path,
                &request.remote_url,
                request.remote_branch.as_deref(),
                remote_auth,
                timeout,
            )
            .await?;

        Ok(serde_json::json!({
            "status": "success",
            "path": request.path,
            "remote_url": request.remote_url,
            "remote_branch": request.remote_branch.as_deref().unwrap_or("main"),
            "result": result,
            "message": format!("Successfully fetched from remote into {}", request.path)
        }))
    }

    pub(crate) async fn handle_push(&self, request: PushTool) -> Result<serde_json::Value> {
        info!(
            "Pushing from path: {} to remote: {}",
            request.path, request.remote_url
        );

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        // Prepare remote authentication if provided
        let remote_auth = match (
            request.remote_username.as_deref(),
            request.remote_password.as_deref(),
        ) {
            (Some(username), Some(password)) => Some((username, password)),
            _ => None,
        };

        // Convert timeout from seconds to Duration if provided
        let timeout = request.timeout_seconds.map(std::time::Duration::from_secs);

        let result = client
            .push(
                &request.path,
                &request.remote_url,
                request.remote_branch.as_deref(),
                remote_auth,
                timeout,
            )
            .await?;

        Ok(serde_json::json!({
            "status": "success",
            "path": request.path,
            "remote_url": request.remote_url,
            "remote_branch": request.remote_branch,
            "result": result,
            "message": format!("Successfully pushed from {} to remote", request.path)
        }))
    }

    pub(crate) async fn handle_pull(&self, request: PullTool) -> Result<serde_json::Value> {
        info!(
            "Pulling into path: {} from remote: {}",
            request.path, request.remote_url
        );

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        // Prepare remote authentication if provided
        let remote_auth = match (
            request.remote_username.as_deref(),
            request.remote_password.as_deref(),
        ) {
            (Some(username), Some(password)) => Some((username, password)),
            _ => None,
        };

        // Convert timeout from seconds to Duration if provided
        let timeout = request.timeout_seconds.map(std::time::Duration::from_secs);

        let result = client
            .pull(
                &request.path,
                &request.remote_url,
                request.remote_branch.as_deref(),
                &request.author,
                &request.message,
                remote_auth,
                timeout,
            )
            .await?;

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
    pub(crate) fn split_remote_path(path: &str) -> Result<(&str, &str)> {
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

    pub(crate) async fn handle_add_remote(&self, request: AddRemoteTool) -> Result<serde_json::Value> {
        info!(
            "Adding remote at path: {} with URL: {}",
            request.path, request.remote_url
        );

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        let (db_path, remote_name) = Self::split_remote_path(&request.path)?;
        let result = client
            .add_remote(db_path, remote_name, &request.remote_url)
            .await?;

        Ok(serde_json::json!({
            "status": "success",
            "path": request.path,
            "remote_url": request.remote_url,
            "result": result,
            "message": format!("Successfully added remote at {}", request.path)
        }))
    }

    pub(crate) async fn handle_get_remote(&self, request: GetRemoteTool) -> Result<serde_json::Value> {
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

    pub(crate) async fn handle_update_remote(&self, request: UpdateRemoteTool) -> Result<serde_json::Value> {
        info!(
            "Updating remote at path: {} with new URL: {}",
            request.path, request.remote_url
        );

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        let (db_path, remote_name) = Self::split_remote_path(&request.path)?;
        let result = client
            .update_remote(db_path, remote_name, &request.remote_url)
            .await?;

        Ok(serde_json::json!({
            "status": "success",
            "path": request.path,
            "remote_url": request.remote_url,
            "result": result,
            "message": format!("Successfully updated remote at {}", request.path)
        }))
    }

    pub(crate) async fn handle_delete_remote(&self, request: DeleteRemoteTool) -> Result<serde_json::Value> {
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
