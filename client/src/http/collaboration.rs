//! Collaboration operations (fetch, push, pull, clone)

use {
    crate::{TerminusDBAdapterError, debug::{OperationEntry, OperationType}},
    ::tracing::{debug, error, instrument},
    anyhow::Context,
    serde::{Serialize, Deserialize},
    serde_json::json,
    std::time::Instant,
};

/// Clone request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneRequest {
    /// URL of the remote repository to clone
    pub remote_url: String,
    /// Optional label for the cloned database
    pub label: Option<String>,
    /// Optional comment for the cloned database
    pub comment: Option<String>,
}

/// Collaboration operations for the TerminusDB HTTP client
impl super::client::TerminusDBHttpClient {
    /// Fetches changes from a remote repository.
    ///
    /// # Arguments
    /// * `path` - Path to fetch into (e.g., "admin/mydb/local/_commits")
    /// * `remote_url` - URL of the remote repository
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.fetch(
    ///     "admin/mydb/local/_commits",
    ///     "https://github.com/user/repo.git"
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.collaboration.fetch",
        skip(self),
        fields(
            path = %path,
            remote_url = %remote_url
        ),
        err
    )]
    pub async fn fetch(
        &self,
        path: &str,
        remote_url: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("fetch").add_path(path).build();

        debug!("POST {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("fetch".to_string()),
            format!("/api/fetch/{}", path)
        ).with_context(None, None);

        // Apply rate limiting for write operations
        self.wait_for_write_rate_limit().await;

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(
                json!({
                    "remote": remote_url
                })
                .to_string(),
            )
            .send()
            .await
            .context("failed to fetch from remote")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("fetch operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("fetch failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully fetched from remote in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Pushes changes to a remote repository.
    ///
    /// # Arguments
    /// * `path` - Path to push from (e.g., "admin/mydb/local/branch/main")
    /// * `remote_url` - URL of the remote repository
    /// * `remote_branch` - Optional remote branch name (defaults to same as local)
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.push(
    ///     "admin/mydb/local/branch/main",
    ///     "https://github.com/user/repo.git",
    ///     Some("main")
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.collaboration.push",
        skip(self),
        fields(
            path = %path,
            remote_url = %remote_url,
            remote_branch = ?remote_branch
        ),
        err
    )]
    pub async fn push(
        &self,
        path: &str,
        remote_url: &str,
        remote_branch: Option<&str>,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("push").add_path(path).build();

        debug!("POST {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("push".to_string()),
            format!("/api/push/{}", path)
        ).with_context(None, None);

        let mut body = json!({
            "remote": remote_url
        });

        if let Some(branch) = remote_branch {
            body["remote_branch"] = json!(branch);
        }

        // Apply rate limiting for write operations
        self.wait_for_write_rate_limit().await;

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .await
            .context("failed to push to remote")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("push operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("push failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully pushed to remote in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Pulls changes from a remote repository (fetch + merge).
    ///
    /// # Arguments
    /// * `path` - Path to pull into (e.g., "admin/mydb/local/branch/main")
    /// * `remote_url` - URL of the remote repository
    /// * `remote_branch` - Optional remote branch name
    /// * `author` - Author for the merge commit
    /// * `message` - Message for the merge commit
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.pull(
    ///     "admin/mydb/local/branch/main",
    ///     "https://github.com/user/repo.git",
    ///     Some("main"),
    ///     "admin",
    ///     "Pull from origin"
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.collaboration.pull",
        skip(self),
        fields(
            path = %path,
            remote_url = %remote_url,
            remote_branch = ?remote_branch,
            author = %author,
            message = %message
        ),
        err
    )]
    pub async fn pull(
        &self,
        path: &str,
        remote_url: &str,
        remote_branch: Option<&str>,
        author: &str,
        message: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("pull").add_path(path).build();

        debug!("POST {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("pull".to_string()),
            format!("/api/pull/{}", path)
        ).with_context(None, None);

        let mut body = json!({
            "remote": remote_url,
            "author": author,
            "message": message
        });

        if let Some(branch) = remote_branch {
            body["remote_branch"] = json!(branch);
        }

        // Apply rate limiting for write operations
        self.wait_for_write_rate_limit().await;

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .await
            .context("failed to pull from remote")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("pull operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("pull failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully pulled from remote in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Clones a remote repository to create a new database.
    ///
    /// # Arguments
    /// * `organization` - Organization to create database in
    /// * `database` - Name for the new database
    /// * `remote_url` - URL of the remote repository to clone
    /// * `label` - Optional label for the database
    /// * `comment` - Optional comment for the database
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.clone_repository(
    ///     "admin",
    ///     "my-clone",
    ///     "https://github.com/user/repo.git",
    ///     Some("My Cloned DB"),
    ///     Some("Cloned from GitHub")
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.collaboration.clone",
        skip(self),
        fields(
            organization = %organization,
            database = %database,
            remote_url = %remote_url,
            label = ?label,
            comment = ?comment
        ),
        err
    )]
    pub async fn clone_repository(
        &self,
        organization: &str,
        database: &str,
        remote_url: &str,
        label: Option<&str>,
        comment: Option<&str>,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url()
            .endpoint("clone")
            .add_path(organization)
            .add_path(database)
            .build();

        debug!("POST {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("clone".to_string()),
            format!("/api/clone/{}/{}", organization, database)
        ).with_context(Some(database.to_string()), None);

        let clone_req = CloneRequest {
            remote_url: remote_url.to_string(),
            label: label.map(String::from),
            comment: comment.map(String::from),
        };

        // Apply rate limiting for write operations
        self.wait_for_write_rate_limit().await;

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .json(&clone_req)
            .send()
            .await
            .context("failed to clone repository")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("clone operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("clone failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully cloned repository in {:?}", start_time.elapsed());

        Ok(response)
    }
}