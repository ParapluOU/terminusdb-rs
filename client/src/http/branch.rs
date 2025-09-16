//! Branch management operations

use {
    crate::{CommitInfo, SquashResponse, TerminusDBAdapterError, debug::{OperationEntry, OperationType, QueryLogEntry}},
    ::tracing::{debug, error, instrument},
    anyhow::Context,
    serde_json::json,
    std::time::Instant,
};

/// Branch operations methods for the TerminusDB HTTP client
impl super::client::TerminusDBHttpClient {
    /// Squashes a commit history into a single commit.
    ///
    /// This operation creates a new unattached commit containing the squashed data. 
    /// This commit can be queried directly, or be assigned to a particular branch 
    /// using the reset endpoint.
    ///
    /// # Arguments
    /// * `path` - Path for a commit or branch (e.g., "admin/test/local/branch/foo" or "admin/test/local/commit/abc123")
    /// * `author` - The author of the squash operation
    /// * `message` - Commit message describing the squash
    ///
    /// # Returns
    /// A `SquashResponse` containing the new commit ID and the old commit ID
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let response = client.squash(
    ///     "admin/mydb/local/branch/main",
    ///     "admin",
    ///     "Squash all commits into one"
    /// ).await?;
    /// println!("New commit: {}", response.commit);
    /// println!("Old commit: {}", response.old_commit);
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.branch.squash",
        skip(self),
        fields(
            path = %path,
            author = %author,
            message = %message
        ),
        err
    )]
    pub async fn squash(
        &self,
        path: &str,
        author: &str,
        message: &str,
    ) -> anyhow::Result<SquashResponse> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("squash").add_path(path).build();

        debug!("POST {}", &uri);

        // Create operation entry
        let mut operation = OperationEntry::new(
            OperationType::Squash,
            format!("/api/squash/{}", path)
        ).with_context(None, None);

        // Create the commit info
        let commit_info = CommitInfo {
            author: author.to_string(),
            message: message.to_string(),
        };

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(
                json!({
                    "commit_info": commit_info
                })
                .to_string(),
            )
            .send()
            .await
            .context("failed to squash commits")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("squash operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("squash failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation.clone());
            
            // Log to query log if enabled
            let logger_opt = self.query_logger.read().ok().and_then(|guard| guard.clone());
            if let Some(logger) = logger_opt {
                let log_entry = QueryLogEntry {
                    timestamp: chrono::Utc::now(),
                    operation_type: "squash".to_string(),
                    database: None,
                    branch: None,
                    endpoint: format!("/api/squash/{}", path),
                    details: serde_json::json!({
                        "path": path,
                        "author": author,
                        "message": message
                    }),
                    success: false,
                    result_count: None,
                    duration_ms,
                    error: Some(error_msg.clone()),
                };
                logger.log(log_entry).await;
            }
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<SquashResponse>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation.clone());
        
        // Log to query log if enabled
        let logger_opt = self.query_logger.read().ok().and_then(|guard| guard.clone());
        if let Some(logger) = logger_opt {
            let log_entry = QueryLogEntry {
                timestamp: chrono::Utc::now(),
                operation_type: "squash".to_string(),
                database: None,
                branch: None,
                endpoint: format!("/api/squash/{}", path),
                details: serde_json::json!({
                    "path": path,
                    "author": author,
                    "message": message,
                    "new_commit": response.commit,
                    "old_commit": response.old_commit
                }),
                success: true,
                result_count: None,
                duration_ms,
                error: None,
            };
            logger.log(log_entry).await;
        }

        debug!("Successfully squashed commits in {:?}", start_time.elapsed());
        debug!("New commit: {}, Old commit: {}", response.commit, response.old_commit);

        Ok(response)
    }

    /// Resets a branch to a specific commit.
    ///
    /// This operation sets the branch HEAD to the specified commit. The commit can be 
    /// specified as either a commit path or another branch path.
    ///
    /// # Arguments
    /// * `branch_path` - Path to the branch to reset (e.g., "admin/mydb/local/branch/main")
    /// * `commit_descriptor` - Path to commit or branch to reset to (e.g., "admin/mydb/local/commit/abc123")
    ///
    /// # Returns
    /// A success response if the reset was successful
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// // Reset main branch to a specific commit
    /// client.reset(
    ///     "admin/mydb/local/branch/main",
    ///     "admin/mydb/local/commit/abc123"
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.branch.reset",
        skip(self),
        fields(
            branch_path = %branch_path,
            commit_descriptor = %commit_descriptor
        ),
        err
    )]
    pub async fn reset(
        &self,
        branch_path: &str,
        commit_descriptor: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("reset").add_path(branch_path).build();

        debug!("POST {}", &uri);

        // Create operation entry
        let mut operation = OperationEntry::new(
            OperationType::Other("reset".to_string()),
            format!("/api/reset/{}", branch_path)
        ).with_context(None, None);

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(
                json!({
                    "commit_descriptor": commit_descriptor
                })
                .to_string(),
            )
            .send()
            .await
            .context("failed to reset branch")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("reset operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("reset failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully reset branch in {:?}", start_time.elapsed());

        Ok(response)
    }
}