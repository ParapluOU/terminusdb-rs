//! User management operations

use {
    crate::{TerminusDBAdapterError, debug::{OperationEntry, OperationType}},
    ::tracing::{debug, error, instrument},
    anyhow::Context,
    serde::{Serialize, Deserialize},
    serde_json::json,
    std::time::Instant,
};

/// User information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// User identifier
    pub id: String,
    /// User name
    pub name: Option<String>,
    /// User email
    pub email: Option<String>,
    /// User roles
    pub roles: Vec<String>,
}

/// User creation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    /// User identifier
    pub id: String,
    /// User name
    pub name: Option<String>,
    /// User email
    pub email: Option<String>,
    /// User password
    pub password: String,
}

/// User update request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserRequest {
    /// New user name
    pub name: Option<String>,
    /// New user email
    pub email: Option<String>,
    /// New password
    pub password: Option<String>,
}

/// User management operations for the TerminusDB HTTP client
impl super::client::TerminusDBHttpClient {
    /// Creates a new user.
    ///
    /// # Arguments
    /// * `user_id` - Unique identifier for the user
    /// * `name` - Optional user name
    /// * `email` - Optional user email
    /// * `password` - User password
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.create_user(
    ///     "john_doe",
    ///     Some("John Doe"),
    ///     Some("john@example.com"),
    ///     "secure_password"
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.user.create",
        skip(self, password),
        fields(
            user_id = %user_id,
            name = ?name,
            email = ?email
        ),
        err
    )]
    pub async fn create_user(
        &self,
        user_id: &str,
        name: Option<&str>,
        email: Option<&str>,
        password: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("user").build();

        debug!("POST {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("create_user".to_string()),
            "/api/user".to_string()
        ).with_context(None, None);

        let request = CreateUserRequest {
            id: user_id.to_string(),
            name: name.map(String::from),
            email: email.map(String::from),
            password: password.to_string(),
        };

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("failed to create user")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("create user operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("create user failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully created user in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Gets information about a user.
    ///
    /// # Arguments
    /// * `user_id` - User identifier
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let user = client.get_user("john_doe").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.user.get",
        skip(self),
        fields(
            user_id = %user_id
        ),
        err
    )]
    pub async fn get_user(
        &self,
        user_id: &str,
    ) -> anyhow::Result<User> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("user").add_path(user_id).build();

        debug!("GET {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("get_user".to_string()),
            format!("/api/user/{}", user_id)
        ).with_context(None, None);

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to get user")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("get user operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("get user failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<User>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully retrieved user in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Updates a user's information.
    ///
    /// # Arguments
    /// * `user_id` - User identifier
    /// * `name` - New name (if provided)
    /// * `email` - New email (if provided)
    /// * `password` - New password (if provided)
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.update_user(
    ///     "john_doe",
    ///     Some("John Smith"),
    ///     None,
    ///     None
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.user.update",
        skip(self, password),
        fields(
            user_id = %user_id,
            name = ?name,
            email = ?email
        ),
        err
    )]
    pub async fn update_user(
        &self,
        user_id: &str,
        name: Option<&str>,
        email: Option<&str>,
        password: Option<&str>,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("user").add_path(user_id).build();

        debug!("PUT {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("update_user".to_string()),
            format!("/api/user/{}", user_id)
        ).with_context(None, None);

        let request = UpdateUserRequest {
            name: name.map(String::from),
            email: email.map(String::from),
            password: password.map(String::from),
        };

        let res = self
            .http
            .put(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("failed to update user")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("update user operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("update user failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully updated user in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Deletes a user.
    ///
    /// # Arguments
    /// * `user_id` - User identifier to delete
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.delete_user("john_doe").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.user.delete",
        skip(self),
        fields(
            user_id = %user_id
        ),
        err
    )]
    pub async fn delete_user(
        &self,
        user_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("user").add_path(user_id).build();

        debug!("DELETE {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("delete_user".to_string()),
            format!("/api/user/{}", user_id)
        ).with_context(None, None);

        let res = self
            .http
            .delete(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to delete user")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("delete user operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("delete user failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully deleted user in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Lists all users.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let users = client.list_users().await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.user.list",
        skip(self),
        err
    )]
    pub async fn list_users(&self) -> anyhow::Result<Vec<User>> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("user").build();

        debug!("GET {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("list_users".to_string()),
            "/api/user".to_string()
        ).with_context(None, None);

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to list users")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("list users operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("list users failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<Vec<User>>(res).await?;
        
        operation = operation.success(Some(response.len()), duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully listed {} users in {:?}", response.len(), start_time.elapsed());

        Ok(response)
    }
}