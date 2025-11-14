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
        timeout: Option<std::time::Duration>,
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

        // Apply rate limiting for write operations
        let _permit = self.acquire_write_permit().await;

        let mut request = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json");

        // Apply timeout: use provided timeout or default to 8 hours for squash operations
        let request = request.timeout(timeout.unwrap_or_else(|| std::time::Duration::from_secs(28800)));

        let res = request
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

    /// Squashes a branch's commit history and immediately resets the branch to the new squashed commit.
    ///
    /// This is a convenience method that combines squash and reset operations in a single call.
    /// It first squashes the branch history into a single commit, then automatically resets
    /// the branch to point to that new commit.
    ///
    /// # Arguments
    /// * `path` - Path to the branch (e.g., "admin/mydb/local/branch/main")
    /// * `author` - Author name for the squash commit
    /// * `message` - Commit message for the squash
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.squash_and_reset(
    ///     "admin/mydb/local/branch/main",
    ///     "admin",
    ///     "Squash all commits into one"
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.branch.squash_and_reset",
        skip(self),
        fields(
            path = %path,
            author = %author,
            message = %message
        ),
        err
    )]
    pub async fn squash_and_reset(
        &self,
        path: &str,
        author: &str,
        message: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();

        debug!("Starting squash and reset for {}", path);

        // Step 1: Squash the branch
        let squash_result = self.squash(path, author, message, None).await
            .context("failed to squash branch")?;

        // Step 2: Extract the new commit ID
        let new_commit = &squash_result.commit;

        debug!("Squash created commit: {}", new_commit);

        // Step 3: Reset the branch to the new commit
        let reset_result = self.reset(path, &new_commit).await
            .context("failed to reset branch to squashed commit")?;

        debug!("Successfully squashed and reset branch in {:?}", start_time.elapsed());

        // Return a combined result
        Ok(json!({
            "squash": squash_result,
            "reset": reset_result,
            "status": "Success"
        }))
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

        // Apply rate limiting for write operations
        let _permit = self.acquire_write_permit().await;

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

    /// Creates a new branch from an existing branch or commit.
    ///
    /// # Arguments
    /// * `branch_path` - Path where the new branch will be created (e.g., "admin/mydb/local/branch/feature")
    /// * `origin` - Source branch or commit to branch from (e.g., "admin/mydb/local/branch/main")
    /// 
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.create_branch(
    ///     "admin/mydb/local/branch/feature",
    ///     "admin/mydb/local/branch/main"
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.branch.create",
        skip(self),
        fields(
            branch_path = %branch_path,
            origin = %origin
        ),
        err
    )]
    pub async fn create_branch(
        &self,
        branch_path: &str,
        origin: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("branch").add_path(branch_path).build();

        debug!("POST {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("create_branch".to_string()),
            format!("/api/branch/{}", branch_path)
        ).with_context(None, None);

        // Apply rate limiting for write operations
        let _permit = self.acquire_write_permit().await;

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(
                json!({
                    "origin": origin
                })
                .to_string(),
            )
            .send()
            .await
            .context("failed to create branch")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("create branch operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("create branch failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully created branch in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Deletes a branch.
    ///
    /// # Arguments
    /// * `branch_path` - Path to the branch to delete (e.g., "admin/mydb/local/branch/feature")
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.delete_branch("admin/mydb/local/branch/feature").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.branch.delete",
        skip(self),
        fields(
            branch_path = %branch_path
        ),
        err
    )]
    pub async fn delete_branch(
        &self,
        branch_path: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("branch").add_path(branch_path).build();

        debug!("DELETE {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("delete_branch".to_string()),
            format!("/api/branch/{}", branch_path)
        ).with_context(None, None);

        // Apply rate limiting for write operations
        let _permit = self.acquire_write_permit().await;

        let res = self
            .http
            .delete(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to delete branch")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("delete branch operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("delete branch failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully deleted branch in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Rebases commits from one branch onto another.
    ///
    /// This finds the most recent common commit between the source and target branches,
    /// then reapplies commits from the source followed by commits from the target branch.
    ///
    /// # Arguments
    /// * `branch_path` - Target branch path (e.g., "admin/mydb/local/branch/feature")
    /// * `rebase_source` - Source branch/commit to rebase from (e.g., "admin/mydb/local/branch/main")
    /// * `author` - Author of the rebase operation
    /// * `message` - Commit message for the rebase
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.rebase(
    ///     "admin/mydb/local/branch/feature",
    ///     "admin/mydb/local/branch/main", 
    ///     "admin",
    ///     "Rebase feature onto main"
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.branch.rebase",
        skip(self),
        fields(
            branch_path = %branch_path,
            rebase_source = %rebase_source,
            author = %author,
            message = %message
        ),
        err
    )]
    pub async fn rebase(
        &self,
        branch_path: &str,
        rebase_source: &str,
        author: &str,
        message: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("rebase").add_path(branch_path).build();

        debug!("POST {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("rebase".to_string()),
            format!("/api/rebase/{}", branch_path)
        ).with_context(None, None);

        // Apply rate limiting for write operations
        let _permit = self.acquire_write_permit().await;

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(
                json!({
                    "rebase_from": rebase_source,
                    "author": author,
                    "message": message
                })
                .to_string(),
            )
            .send()
            .await
            .context("failed to rebase branch")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("rebase operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("rebase failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully rebased branch in {:?}", start_time.elapsed());

        Ok(response)
    }
}