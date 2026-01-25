//! Organization management operations
//!
//! Implements the TerminusDB organization API endpoints for managing
//! organizations, users within organizations, and user capabilities/roles.

use {
    crate::debug::{OperationEntry, OperationType},
    ::tracing::{debug, error, instrument},
    anyhow::Context,
    serde::{Deserialize, Serialize},
    serde_json::json,
    std::time::Instant,
};

// ============================================================
// Types matching TerminusDB API responses
// ============================================================

/// Organization as returned by TerminusDB API
#[derive(Debug, Clone, Deserialize)]
pub struct Organization {
    /// Organization identifier (e.g., "Organization/admin")
    #[serde(rename = "@id")]
    pub id: String,
    /// Type identifier (always "Organization")
    #[serde(rename = "@type")]
    pub org_type: String,
    /// Organization name
    pub name: String,
    /// Databases belonging to this organization
    #[serde(default)]
    pub database: Vec<String>,
}

/// User with capabilities in an organization
#[derive(Debug, Clone, Deserialize)]
pub struct OrganizationUser {
    /// User identifier (e.g., "User/admin")
    #[serde(rename = "@id")]
    pub id: String,
    /// Username
    pub name: String,
    /// Capabilities granted to this user
    #[serde(default)]
    pub capability: Vec<Capability>,
}

/// Capability granted to a user
#[derive(Debug, Clone, Deserialize)]
pub struct Capability {
    /// Capability identifier
    #[serde(rename = "@id")]
    pub id: String,
    /// Type identifier (always "Capability")
    #[serde(rename = "@type")]
    pub cap_type: String,
    /// Resource scope (e.g., "Organization/admin")
    pub scope: String,
    /// Roles associated with this capability
    #[serde(default)]
    pub role: Vec<Role>,
}

/// Role within a capability
#[derive(Debug, Clone, Deserialize)]
pub struct Role {
    /// Role identifier (e.g., "Role/admin")
    #[serde(rename = "@id")]
    pub id: String,
    /// Type identifier (always "Role")
    #[serde(rename = "@type")]
    pub role_type: String,
    /// Role name
    pub name: String,
    /// Actions permitted by this role
    #[serde(default)]
    pub action: Vec<String>,
}

/// Database info returned from organization user databases endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct OrganizationDatabase {
    /// Database identifier
    #[serde(rename = "@id")]
    pub id: String,
    /// Type identifier (e.g., "SystemDatabase", "UserDatabase")
    #[serde(rename = "@type")]
    pub db_type: String,
    /// Database name
    pub name: String,
}

/// Success response from organization operations
#[derive(Debug, Clone, Deserialize)]
pub struct OrganizationResponse {
    /// Response type
    #[serde(rename = "@type")]
    pub response_type: String,
    /// Status (e.g., "api:success")
    #[serde(rename = "api:status")]
    pub status: String,
}

/// Request body for creating/updating user roles
#[derive(Debug, Clone, Serialize)]
pub struct UserRoleRequest {
    /// Resource scope
    pub scope: String,
    /// Role name
    pub role: String,
}

// ============================================================
// Organization CRUD Operations
// ============================================================

impl super::client::TerminusDBHttpClient {
    /// Lists all organizations.
    ///
    /// Returns all organizations visible to the authenticated user.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let orgs = client.list_organizations().await?;
    /// for org in orgs {
    ///     println!("Organization: {}", org.name);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(name = "terminus.organization.list", skip(self), err)]
    pub async fn list_organizations(&self) -> anyhow::Result<Vec<Organization>> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("organizations").build();

        debug!("GET {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("list_organizations".to_string()),
            "/api/organizations".to_string(),
        )
        .with_context(None, None);

        let _permit = self.acquire_read_permit().await;

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to list organizations")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("list organizations operation failed with status {}", status);
            let error_text = res.text().await?;
            let error_msg = format!("list organizations failed: {:#?}", error_text);
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<Vec<Organization>>(res).await?;

        operation = operation.success(Some(response.len()), duration_ms);
        self.operation_log.push(operation);

        debug!(
            "Successfully listed {} organizations in {:?}",
            response.len(),
            start_time.elapsed()
        );

        Ok(response)
    }

    /// Gets information about a specific organization.
    ///
    /// # Arguments
    /// * `org_name` - Organization name
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let org = client.get_organization("admin").await?;
    /// println!("Databases: {:?}", org.database);
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.organization.get",
        skip(self),
        fields(org_name = %org_name),
        err
    )]
    pub async fn get_organization(&self, org_name: &str) -> anyhow::Result<Organization> {
        let start_time = Instant::now();
        let uri = self
            .build_url()
            .endpoint("organizations")
            .add_path(org_name)
            .build();

        debug!("GET {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("get_organization".to_string()),
            format!("/api/organizations/{}", org_name),
        )
        .with_context(None, None);

        let _permit = self.acquire_read_permit().await;

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to get organization")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("get organization operation failed with status {}", status);
            let error_text = res.text().await?;
            let error_msg = format!("get organization failed: {:#?}", error_text);
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<Organization>(res).await?;

        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!(
            "Successfully retrieved organization in {:?}",
            start_time.elapsed()
        );

        Ok(response)
    }

    /// Creates a new organization.
    ///
    /// # Arguments
    /// * `org_name` - Name for the new organization
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.create_organization("my_org").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.organization.create",
        skip(self),
        fields(org_name = %org_name),
        err
    )]
    pub async fn create_organization(&self, org_name: &str) -> anyhow::Result<String> {
        let start_time = Instant::now();
        let uri = self
            .build_url()
            .endpoint("organizations")
            .add_path(org_name)
            .build();

        debug!("POST {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("create_organization".to_string()),
            format!("/api/organizations/{}", org_name),
        )
        .with_context(None, None);

        let _permit = self.acquire_write_permit().await;

        // JS client sends empty object {}
        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .json(&json!({}))
            .send()
            .await
            .context("failed to create organization")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!(
                "create organization operation failed with status {}",
                status
            );
            let error_text = res.text().await?;
            let error_msg = format!("create organization failed: {:#?}", error_text);
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            return Err(anyhow::anyhow!(error_msg));
        }

        // Response is a URI string like "terminusdb://system/data/Organization/my_org"
        let response = self.parse_response::<String>(res).await?;

        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!(
            "Successfully created organization in {:?}",
            start_time.elapsed()
        );

        Ok(response)
    }

    /// Deletes an organization.
    ///
    /// # Arguments
    /// * `org_name` - Name of the organization to delete
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.delete_organization("my_org").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.organization.delete",
        skip(self),
        fields(org_name = %org_name),
        err
    )]
    pub async fn delete_organization(
        &self,
        org_name: &str,
    ) -> anyhow::Result<OrganizationResponse> {
        let start_time = Instant::now();
        let uri = self
            .build_url()
            .endpoint("organizations")
            .add_path(org_name)
            .build();

        debug!("DELETE {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("delete_organization".to_string()),
            format!("/api/organizations/{}", org_name),
        )
        .with_context(None, None);

        let _permit = self.acquire_write_permit().await;

        let res = self
            .http
            .delete(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to delete organization")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!(
                "delete organization operation failed with status {}",
                status
            );
            let error_text = res.text().await?;
            let error_msg = format!("delete organization failed: {:#?}", error_text);
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<OrganizationResponse>(res).await?;

        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!(
            "Successfully deleted organization in {:?}",
            start_time.elapsed()
        );

        Ok(response)
    }

    // ============================================================
    // Organization Users Operations
    // ============================================================

    /// Gets all users in an organization.
    ///
    /// # Arguments
    /// * `org_name` - Organization name
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let users = client.get_organization_users("admin").await?;
    /// for user in users {
    ///     println!("User: {} with {} capabilities", user.name, user.capability.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.organization.get_users",
        skip(self),
        fields(org_name = %org_name),
        err
    )]
    pub async fn get_organization_users(
        &self,
        org_name: &str,
    ) -> anyhow::Result<Vec<OrganizationUser>> {
        let start_time = Instant::now();
        let uri = self
            .build_url()
            .endpoint("organizations")
            .add_path(org_name)
            .add_path("users")
            .build();

        debug!("GET {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("get_organization_users".to_string()),
            format!("/api/organizations/{}/users", org_name),
        )
        .with_context(None, None);

        let _permit = self.acquire_read_permit().await;

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to get organization users")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!(
                "get organization users operation failed with status {}",
                status
            );
            let error_text = res.text().await?;
            let error_msg = format!("get organization users failed: {:#?}", error_text);
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<Vec<OrganizationUser>>(res).await?;

        operation = operation.success(Some(response.len()), duration_ms);
        self.operation_log.push(operation);

        debug!(
            "Successfully retrieved {} organization users in {:?}",
            response.len(),
            start_time.elapsed()
        );

        Ok(response)
    }

    /// Gets a specific user's info within an organization.
    ///
    /// # Arguments
    /// * `org_name` - Organization name
    /// * `user_name` - User name
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let user = client.get_organization_user("admin", "admin").await?;
    /// println!("User {} has {} capabilities", user.name, user.capability.len());
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.organization.get_user",
        skip(self),
        fields(org_name = %org_name, user_name = %user_name),
        err
    )]
    pub async fn get_organization_user(
        &self,
        org_name: &str,
        user_name: &str,
    ) -> anyhow::Result<OrganizationUser> {
        let start_time = Instant::now();
        let uri = self
            .build_url()
            .endpoint("organizations")
            .add_path(org_name)
            .add_path("users")
            .add_path(user_name)
            .build();

        debug!("GET {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("get_organization_user".to_string()),
            format!("/api/organizations/{}/users/{}", org_name, user_name),
        )
        .with_context(None, None);

        let _permit = self.acquire_read_permit().await;

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to get organization user")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!(
                "get organization user operation failed with status {}",
                status
            );
            let error_text = res.text().await?;
            let error_msg = format!("get organization user failed: {:#?}", error_text);
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<OrganizationUser>(res).await?;

        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!(
            "Successfully retrieved organization user in {:?}",
            start_time.elapsed()
        );

        Ok(response)
    }

    /// Gets databases available to a user within an organization.
    ///
    /// # Arguments
    /// * `org_name` - Organization name
    /// * `user_name` - User name
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let dbs = client.get_organization_user_databases("admin", "admin").await?;
    /// for db in dbs {
    ///     println!("Database: {} ({})", db.name, db.db_type);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.organization.get_user_databases",
        skip(self),
        fields(org_name = %org_name, user_name = %user_name),
        err
    )]
    pub async fn get_organization_user_databases(
        &self,
        org_name: &str,
        user_name: &str,
    ) -> anyhow::Result<Vec<OrganizationDatabase>> {
        let start_time = Instant::now();
        let uri = self
            .build_url()
            .endpoint("organizations")
            .add_path(org_name)
            .add_path("users")
            .add_path(user_name)
            .add_path("databases")
            .build();

        debug!("GET {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("get_organization_user_databases".to_string()),
            format!(
                "/api/organizations/{}/users/{}/databases",
                org_name, user_name
            ),
        )
        .with_context(None, None);

        let _permit = self.acquire_read_permit().await;

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to get organization user databases")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!(
                "get organization user databases operation failed with status {}",
                status
            );
            let error_text = res.text().await?;
            let error_msg = format!("get organization user databases failed: {:#?}", error_text);
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self
            .parse_response::<Vec<OrganizationDatabase>>(res)
            .await?;

        operation = operation.success(Some(response.len()), duration_ms);
        self.operation_log.push(operation);

        debug!(
            "Successfully retrieved {} databases in {:?}",
            response.len(),
            start_time.elapsed()
        );

        Ok(response)
    }

    // ============================================================
    // User Role Management Operations
    // ============================================================

    /// Removes a user from an organization.
    ///
    /// # Arguments
    /// * `org_name` - Organization name
    /// * `user_name` - User name to remove
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.remove_user_from_org("my_org", "some_user").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.organization.remove_user",
        skip(self),
        fields(org_name = %org_name, user_name = %user_name),
        err
    )]
    pub async fn remove_user_from_org(
        &self,
        org_name: &str,
        user_name: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self
            .build_url()
            .endpoint("organizations")
            .add_path(org_name)
            .add_path("users")
            .add_path(user_name)
            .build();

        debug!("DELETE {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("remove_user_from_org".to_string()),
            format!("/api/organizations/{}/users/{}", org_name, user_name),
        )
        .with_context(None, None);

        let _permit = self.acquire_write_permit().await;

        let res = self
            .http
            .delete(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to remove user from organization")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!(
                "remove user from organization operation failed with status {}",
                status
            );
            let error_text = res.text().await?;
            let error_msg = format!("remove user from organization failed: {:#?}", error_text);
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;

        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!(
            "Successfully removed user from organization in {:?}",
            start_time.elapsed()
        );

        Ok(response)
    }

    /// Creates a role (capability) for a user within an organization.
    ///
    /// # Arguments
    /// * `org_name` - Organization name
    /// * `user_name` - User name
    /// * `scope` - Resource scope (e.g., "Organization/my_org")
    /// * `role` - Role name (e.g., "admin", "reader")
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.create_user_role("my_org", "some_user", "Organization/my_org", "admin").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.organization.create_user_role",
        skip(self),
        fields(org_name = %org_name, user_name = %user_name, scope = %scope, role = %role),
        err
    )]
    pub async fn create_user_role(
        &self,
        org_name: &str,
        user_name: &str,
        scope: &str,
        role: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self
            .build_url()
            .endpoint("organizations")
            .add_path(org_name)
            .add_path("users")
            .add_path(user_name)
            .add_path("capabilities")
            .build();

        debug!("POST {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("create_user_role".to_string()),
            format!(
                "/api/organizations/{}/users/{}/capabilities",
                org_name, user_name
            ),
        )
        .with_context(None, None);

        let request = UserRoleRequest {
            scope: scope.to_string(),
            role: role.to_string(),
        };

        let _permit = self.acquire_write_permit().await;

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("failed to create user role")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("create user role operation failed with status {}", status);
            let error_text = res.text().await?;
            let error_msg = format!("create user role failed: {:#?}", error_text);
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;

        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!(
            "Successfully created user role in {:?}",
            start_time.elapsed()
        );

        Ok(response)
    }

    /// Updates a user's role (capability) within an organization.
    ///
    /// # Arguments
    /// * `org_name` - Organization name
    /// * `user_name` - User name
    /// * `capability_hash` - Capability identifier hash
    /// * `scope` - New resource scope
    /// * `role` - New role name
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.update_user_role("my_org", "some_user", "cap123", "Organization/my_org", "reader").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.organization.update_user_role",
        skip(self),
        fields(
            org_name = %org_name,
            user_name = %user_name,
            capability_hash = %capability_hash,
            scope = %scope,
            role = %role
        ),
        err
    )]
    pub async fn update_user_role(
        &self,
        org_name: &str,
        user_name: &str,
        capability_hash: &str,
        scope: &str,
        role: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self
            .build_url()
            .endpoint("organizations")
            .add_path(org_name)
            .add_path("users")
            .add_path(user_name)
            .add_path("capabilities")
            .add_path(capability_hash)
            .build();

        debug!("PUT {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("update_user_role".to_string()),
            format!(
                "/api/organizations/{}/users/{}/capabilities/{}",
                org_name, user_name, capability_hash
            ),
        )
        .with_context(None, None);

        let request = UserRoleRequest {
            scope: scope.to_string(),
            role: role.to_string(),
        };

        let _permit = self.acquire_write_permit().await;

        let res = self
            .http
            .put(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("failed to update user role")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("update user role operation failed with status {}", status);
            let error_text = res.text().await?;
            let error_msg = format!("update user role failed: {:#?}", error_text);
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;

        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!(
            "Successfully updated user role in {:?}",
            start_time.elapsed()
        );

        Ok(response)
    }
}
