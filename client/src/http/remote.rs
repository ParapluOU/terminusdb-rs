//! Remote repository management operations

use {
    crate::{TerminusDBAdapterError, debug::{OperationEntry, OperationType}},
    ::tracing::{debug, error, instrument},
    anyhow::Context,
    serde::{Serialize, Deserialize},
    serde_json::json,
    std::time::Instant,
};

/// Remote repository configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteConfig {
    /// Remote repository URL
    pub remote_url: String,
}

/// Remote repository information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteInfo {
    /// Remote repository URL
    pub remote_url: String,
    /// Remote name (extracted from path)
    pub name: String,
}

/// Remote repository operations for the TerminusDB HTTP client
impl super::client::TerminusDBHttpClient {
    /// Adds a new remote repository.
    ///
    /// # Arguments
    /// * `path` - Path where the remote will be added (e.g., "admin/mydb/remote/origin")
    /// * `remote_url` - URL of the remote repository
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.add_remote(
    ///     "admin/mydb/remote/origin",
    ///     "https://github.com/user/repo.git"
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.remote.add",
        skip(self),
        fields(
            path = %path,
            remote_url = %remote_url
        ),
        err
    )]
    pub async fn add_remote(
        &self,
        path: &str,
        remote_url: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("remote").add_path(path).build();

        debug!("POST {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("add_remote".to_string()),
            format!("/api/remote/{}", path)
        ).with_context(None, None);

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(
                json!({
                    "remote_url": remote_url
                })
                .to_string(),
            )
            .send()
            .await
            .context("failed to add remote")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("add remote operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("add remote failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully added remote in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Gets information about a remote repository.
    ///
    /// # Arguments
    /// * `path` - Path to the remote (e.g., "admin/mydb/remote/origin")
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let info = client.get_remote("admin/mydb/remote/origin").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.remote.get",
        skip(self),
        fields(
            path = %path
        ),
        err
    )]
    pub async fn get_remote(
        &self,
        path: &str,
    ) -> anyhow::Result<RemoteInfo> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("remote").add_path(path).build();

        debug!("GET {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("get_remote".to_string()),
            format!("/api/remote/{}", path)
        ).with_context(None, None);

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to get remote")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("get remote operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("get remote failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<RemoteInfo>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully retrieved remote in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Updates a remote repository URL.
    ///
    /// # Arguments
    /// * `path` - Path to the remote (e.g., "admin/mydb/remote/origin")
    /// * `remote_url` - New URL for the remote repository
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.update_remote(
    ///     "admin/mydb/remote/origin",
    ///     "https://github.com/user/new-repo.git"
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.remote.update",
        skip(self),
        fields(
            path = %path,
            remote_url = %remote_url
        ),
        err
    )]
    pub async fn update_remote(
        &self,
        path: &str,
        remote_url: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("remote").add_path(path).build();

        debug!("PUT {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("update_remote".to_string()),
            format!("/api/remote/{}", path)
        ).with_context(None, None);

        let res = self
            .http
            .put(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(
                json!({
                    "remote_url": remote_url
                })
                .to_string(),
            )
            .send()
            .await
            .context("failed to update remote")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("update remote operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("update remote failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully updated remote in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Deletes a remote repository.
    ///
    /// # Arguments
    /// * `path` - Path to the remote to delete (e.g., "admin/mydb/remote/origin")
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.delete_remote("admin/mydb/remote/origin").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.remote.delete",
        skip(self),
        fields(
            path = %path
        ),
        err
    )]
    pub async fn delete_remote(
        &self,
        path: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("remote").add_path(path).build();

        debug!("DELETE {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("delete_remote".to_string()),
            format!("/api/remote/{}", path)
        ).with_context(None, None);

        let res = self
            .http
            .delete(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to delete remote")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("delete remote operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("delete remote failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully deleted remote in {:?}", start_time.elapsed());

        Ok(response)
    }
}