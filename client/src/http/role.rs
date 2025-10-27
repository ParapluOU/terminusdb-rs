//! Role management operations

use {
    crate::{TerminusDBAdapterError, debug::{OperationEntry, OperationType}},
    ::tracing::{debug, error, instrument},
    anyhow::Context,
    serde::{Serialize, Deserialize},
    serde_json::json,
    std::time::Instant,
};

/// Role information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    /// Role identifier
    pub id: String,
    /// Role name
    pub name: String,
    /// Role permissions
    pub permissions: Vec<Permission>,
}

/// Permission structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    /// Resource type (database, organization, etc.)
    pub resource: String,
    /// Actions allowed (read, write, delete, etc.)
    pub actions: Vec<String>,
}

/// Role creation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoleRequest {
    /// Role identifier
    pub id: String,
    /// Role name
    pub name: String,
    /// Role permissions
    pub permissions: Vec<Permission>,
}

/// Role update request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRoleRequest {
    /// New role name
    pub name: Option<String>,
    /// New permissions (replaces all)
    pub permissions: Option<Vec<Permission>>,
}

/// Role management operations for the TerminusDB HTTP client
impl super::client::TerminusDBHttpClient {
    /// Creates a new role.
    ///
    /// # Arguments
    /// * `role_id` - Unique identifier for the role
    /// * `name` - Role name
    /// * `permissions` - List of permissions for the role
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # use terminusdb_client::http::role::Permission;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let permissions = vec![
    ///     Permission {
    ///         resource: "database".to_string(),
    ///         actions: vec!["read".to_string(), "write".to_string()],
    ///     }
    /// ];
    /// client.create_role(
    ///     "data_analyst",
    ///     "Data Analyst",
    ///     permissions
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.role.create",
        skip(self, permissions),
        fields(
            role_id = %role_id,
            name = %name,
            permissions_count = permissions.len()
        ),
        err
    )]
    pub async fn create_role(
        &self,
        role_id: &str,
        name: &str,
        permissions: Vec<Permission>,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("role").build();

        debug!("POST {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("create_role".to_string()),
            "/api/role".to_string()
        ).with_context(None, None);

        let request = CreateRoleRequest {
            id: role_id.to_string(),
            name: name.to_string(),
            permissions,
        };

        // Apply rate limiting for write operations
        self.wait_for_write_rate_limit().await;

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("failed to create role")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("create role operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("create role failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully created role in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Gets information about a role.
    ///
    /// # Arguments
    /// * `role_id` - Role identifier
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let role = client.get_role("data_analyst").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.role.get",
        skip(self),
        fields(
            role_id = %role_id
        ),
        err
    )]
    pub async fn get_role(
        &self,
        role_id: &str,
    ) -> anyhow::Result<Role> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("role").add_path(role_id).build();

        debug!("GET {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("get_role".to_string()),
            format!("/api/role/{}", role_id)
        ).with_context(None, None);

        // Apply rate limiting for read operations
        self.wait_for_read_rate_limit().await;

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to get role")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("get role operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("get role failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<Role>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully retrieved role in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Updates a role's information.
    ///
    /// # Arguments
    /// * `role_id` - Role identifier
    /// * `name` - New name (if provided)
    /// * `permissions` - New permissions (replaces all if provided)
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.update_role(
    ///     "data_analyst",
    ///     Some("Senior Data Analyst"),
    ///     None
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.role.update",
        skip(self, permissions),
        fields(
            role_id = %role_id,
            name = ?name
        ),
        err
    )]
    pub async fn update_role(
        &self,
        role_id: &str,
        name: Option<&str>,
        permissions: Option<Vec<Permission>>,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("role").add_path(role_id).build();

        debug!("PUT {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("update_role".to_string()),
            format!("/api/role/{}", role_id)
        ).with_context(None, None);

        let request = UpdateRoleRequest {
            name: name.map(String::from),
            permissions,
        };

        // Apply rate limiting for write operations
        self.wait_for_write_rate_limit().await;

        let res = self
            .http
            .put(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("failed to update role")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("update role operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("update role failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully updated role in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Deletes a role.
    ///
    /// # Arguments
    /// * `role_id` - Role identifier to delete
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.delete_role("data_analyst").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.role.delete",
        skip(self),
        fields(
            role_id = %role_id
        ),
        err
    )]
    pub async fn delete_role(
        &self,
        role_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("role").add_path(role_id).build();

        debug!("DELETE {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("delete_role".to_string()),
            format!("/api/role/{}", role_id)
        ).with_context(None, None);

        // Apply rate limiting for write operations
        self.wait_for_write_rate_limit().await;

        let res = self
            .http
            .delete(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to delete role")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("delete role operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("delete role failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully deleted role in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Lists all roles.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let roles = client.list_roles().await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.role.list",
        skip(self),
        err
    )]
    pub async fn list_roles(&self) -> anyhow::Result<Vec<Role>> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("role").build();

        debug!("GET {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("list_roles".to_string()),
            "/api/role".to_string()
        ).with_context(None, None);

        // Apply rate limiting for read operations
        self.wait_for_read_rate_limit().await;

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to list roles")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("list roles operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("list roles failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<Vec<Role>>(res).await?;
        
        operation = operation.success(Some(response.len()), duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully listed {} roles in {:?}", response.len(), start_time.elapsed());

        Ok(response)
    }

    /// Grants a role to a user.
    ///
    /// # Arguments
    /// * `user_id` - User identifier
    /// * `role_id` - Role identifier to grant
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.grant_role("john_doe", "data_analyst").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.role.grant",
        skip(self),
        fields(
            user_id = %user_id,
            role_id = %role_id
        ),
        err
    )]
    pub async fn grant_role(
        &self,
        user_id: &str,
        role_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url()
            .endpoint("user")
            .add_path(user_id)
            .endpoint("role")
            .add_path(role_id)
            .build();

        debug!("POST {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("grant_role".to_string()),
            format!("/api/user/{}/role/{}", user_id, role_id)
        ).with_context(None, None);

        // Apply rate limiting for write operations
        self.wait_for_write_rate_limit().await;

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to grant role")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("grant role operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("grant role failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully granted role in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Revokes a role from a user.
    ///
    /// # Arguments
    /// * `user_id` - User identifier
    /// * `role_id` - Role identifier to revoke
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.revoke_role("john_doe", "data_analyst").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.role.revoke",
        skip(self),
        fields(
            user_id = %user_id,
            role_id = %role_id
        ),
        err
    )]
    pub async fn revoke_role(
        &self,
        user_id: &str,
        role_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url()
            .endpoint("user")
            .add_path(user_id)
            .endpoint("role")
            .add_path(role_id)
            .build();

        debug!("DELETE {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("revoke_role".to_string()),
            format!("/api/user/{}/role/{}", user_id, role_id)
        ).with_context(None, None);

        // Apply rate limiting for write operations
        self.wait_for_write_rate_limit().await;

        let res = self
            .http
            .delete(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to revoke role")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("revoke role operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("revoke role failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully revoked role in {:?}", start_time.elapsed());

        Ok(response)
    }
}