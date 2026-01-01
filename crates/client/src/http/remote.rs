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
#[derive(Debug, Clone)]
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
    /// * `path` - Path to the database (format: org/database, e.g., "admin/mydb")
    /// * `remote_name` - Name for the remote (e.g., "origin")
    /// * `remote_location` - URL/location of the remote repository
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.add_remote(
    ///     "admin/mydb",
    ///     "origin",
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
            remote_name = %remote_name,
            remote_location = %remote_location
        ),
        err
    )]
    pub async fn add_remote(
        &self,
        path: &str,
        remote_name: &str,
        remote_location: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("remote").add_path(path).build();

        debug!("POST {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("add_remote".to_string()),
            format!("/api/remote/{}", path)
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
                    "remote_name": remote_name,
                    "remote_location": remote_location
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
    /// * `path` - Path to the database (format: org/database, e.g., "admin/mydb")
    /// * `remote_name` - Name of the remote (e.g., "origin")
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let info = client.get_remote("admin/mydb", "origin").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.remote.get",
        skip(self),
        fields(
            path = %path,
            remote_name = %remote_name
        ),
        err
    )]
    pub async fn get_remote(
        &self,
        path: &str,
        remote_name: &str,
    ) -> anyhow::Result<RemoteInfo> {
        let start_time = Instant::now();
        let uri = self.build_url()
            .endpoint("remote")
            .add_path(path)
            .query("remote_name", remote_name)
            .build();

        debug!("GET {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("get_remote".to_string()),
            format!("/api/remote/{}?remote_name={}", path, remote_name)
        ).with_context(None, None);

        // Apply rate limiting for read operations
        let _permit = self.acquire_read_permit().await;

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

    /// Updates a remote repository location.
    ///
    /// # Arguments
    /// * `path` - Path to the database (format: org/database, e.g., "admin/mydb")
    /// * `remote_name` - Name of the remote (e.g., "origin")
    /// * `remote_location` - New URL/location for the remote repository
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.update_remote(
    ///     "admin/mydb",
    ///     "origin",
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
            remote_name = %remote_name,
            remote_location = %remote_location
        ),
        err
    )]
    pub async fn update_remote(
        &self,
        path: &str,
        remote_name: &str,
        remote_location: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("remote").add_path(path).build();

        debug!("PUT {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("update_remote".to_string()),
            format!("/api/remote/{}", path)
        ).with_context(None, None);

        // Apply rate limiting for write operations
        let _permit = self.acquire_write_permit().await;

        let res = self
            .http
            .put(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(
                json!({
                    "remote_name": remote_name,
                    "remote_location": remote_location
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

    /// Lists all remote repositories for a database.
    ///
    /// # Arguments
    /// * `path` - Path to the database (format: org/database, e.g., "admin/mydb")
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let remotes = client.list_remotes("admin/mydb").await?;
    /// for remote in remotes {
    ///     println!("{}: {}", remote.name, remote.remote_url);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.remote.list",
        skip(self),
        fields(path = %path),
        err
    )]
    pub async fn list_remotes(
        &self,
        path: &str,
    ) -> anyhow::Result<Vec<RemoteInfo>> {
        let start_time = Instant::now();
        let uri = self.build_url()
            .endpoint("remote")
            .add_path(path)
            .build();

        debug!("GET {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("list_remotes".to_string()),
            format!("/api/remote/{}", path)
        ).with_context(None, None);

        // Apply rate limiting for read operations
        let _permit = self.acquire_read_permit().await;

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to list remotes")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("list remotes operation failed with status {}", status);

            let error_text = res.text().await?;
            let error_msg = format!("list remotes failed: {:#?}", error_text);

            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);

            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<Vec<RemoteInfo>>(res).await?;

        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully listed remotes in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Deletes a remote repository.
    ///
    /// # Arguments
    /// * `path` - Path to the database (format: org/database, e.g., "admin/mydb")
    /// * `remote_name` - Name of the remote to delete (e.g., "origin")
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.delete_remote("admin/mydb", "origin").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.remote.delete",
        skip(self),
        fields(
            path = %path,
            remote_name = %remote_name
        ),
        err
    )]
    pub async fn delete_remote(
        &self,
        path: &str,
        remote_name: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url()
            .endpoint("remote")
            .add_path(path)
            .query("remote_name", remote_name)
            .build();

        debug!("DELETE {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("delete_remote".to_string()),
            format!("/api/remote/{}?remote_name={}", path, remote_name)
        ).with_context(None, None);

        // Apply rate limiting for write operations
        let _permit = self.acquire_write_permit().await;

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