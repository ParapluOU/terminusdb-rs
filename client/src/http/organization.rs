//! Organization management operations

use {
    crate::{TerminusDBAdapterError, debug::{OperationEntry, OperationType}},
    ::tracing::{debug, error, instrument},
    anyhow::Context,
    serde::{Serialize, Deserialize},
    serde_json::json,
    std::time::Instant,
};

/// Organization information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    /// Organization identifier
    pub id: String,
    /// Organization name
    pub name: String,
    /// Organization description
    pub description: Option<String>,
    /// Organization members
    pub members: Vec<OrganizationMember>,
}

/// Organization member
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationMember {
    /// User ID
    pub user_id: String,
    /// Member role in the organization
    pub role: String,
}

/// Organization creation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrganizationRequest {
    /// Organization identifier
    pub id: String,
    /// Organization name
    pub name: String,
    /// Organization description
    pub description: Option<String>,
}

/// Organization update request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateOrganizationRequest {
    /// New organization name
    pub name: Option<String>,
    /// New organization description
    pub description: Option<String>,
}

/// Organization management operations for the TerminusDB HTTP client
impl super::client::TerminusDBHttpClient {
    /// Creates a new organization.
    ///
    /// # Arguments
    /// * `org_id` - Unique identifier for the organization
    /// * `name` - Organization name
    /// * `description` - Optional organization description
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.create_organization(
    ///     "acme_corp",
    ///     "ACME Corporation",
    ///     Some("Leading provider of innovative solutions")
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.organization.create",
        skip(self),
        fields(
            org_id = %org_id,
            name = %name,
            description = ?description
        ),
        err
    )]
    pub async fn create_organization(
        &self,
        org_id: &str,
        name: &str,
        description: Option<&str>,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("organization").build();

        debug!("POST {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("create_organization".to_string()),
            "/api/organization".to_string()
        ).with_context(None, None);

        let request = CreateOrganizationRequest {
            id: org_id.to_string(),
            name: name.to_string(),
            description: description.map(String::from),
        };

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("failed to create organization")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("create organization operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("create organization failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully created organization in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Gets information about an organization.
    ///
    /// # Arguments
    /// * `org_id` - Organization identifier
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let org = client.get_organization("acme_corp").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.organization.get",
        skip(self),
        fields(
            org_id = %org_id
        ),
        err
    )]
    pub async fn get_organization(
        &self,
        org_id: &str,
    ) -> anyhow::Result<Organization> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("organization").add_path(org_id).build();

        debug!("GET {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("get_organization".to_string()),
            format!("/api/organization/{}", org_id)
        ).with_context(None, None);

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

        debug!("Successfully retrieved organization in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Updates an organization's information.
    ///
    /// # Arguments
    /// * `org_id` - Organization identifier
    /// * `name` - New name (if provided)
    /// * `description` - New description (if provided)
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.update_organization(
    ///     "acme_corp",
    ///     Some("ACME Corporation Inc."),
    ///     None
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.organization.update",
        skip(self),
        fields(
            org_id = %org_id,
            name = ?name,
            description = ?description
        ),
        err
    )]
    pub async fn update_organization(
        &self,
        org_id: &str,
        name: Option<&str>,
        description: Option<&str>,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("organization").add_path(org_id).build();

        debug!("PUT {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("update_organization".to_string()),
            format!("/api/organization/{}", org_id)
        ).with_context(None, None);

        let request = UpdateOrganizationRequest {
            name: name.map(String::from),
            description: description.map(String::from),
        };

        let res = self
            .http
            .put(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("failed to update organization")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("update organization operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("update organization failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully updated organization in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Deletes an organization.
    ///
    /// # Arguments
    /// * `org_id` - Organization identifier to delete
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.delete_organization("acme_corp").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.organization.delete",
        skip(self),
        fields(
            org_id = %org_id
        ),
        err
    )]
    pub async fn delete_organization(
        &self,
        org_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("organization").add_path(org_id).build();

        debug!("DELETE {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("delete_organization".to_string()),
            format!("/api/organization/{}", org_id)
        ).with_context(None, None);

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
            error!("delete organization operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("delete organization failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully deleted organization in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Lists all organizations.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let orgs = client.list_organizations().await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.organization.list",
        skip(self),
        err
    )]
    pub async fn list_organizations(&self) -> anyhow::Result<Vec<Organization>> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("organization").build();

        debug!("GET {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("list_organizations".to_string()),
            "/api/organization".to_string()
        ).with_context(None, None);

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

        debug!("Successfully listed {} organizations in {:?}", response.len(), start_time.elapsed());

        Ok(response)
    }

    /// Adds a member to an organization.
    ///
    /// # Arguments
    /// * `org_id` - Organization identifier
    /// * `user_id` - User identifier to add
    /// * `role` - Role for the user in the organization
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.add_organization_member(
    ///     "acme_corp",
    ///     "john_doe",
    ///     "admin"
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.organization.add_member",
        skip(self),
        fields(
            org_id = %org_id,
            user_id = %user_id,
            role = %role
        ),
        err
    )]
    pub async fn add_organization_member(
        &self,
        org_id: &str,
        user_id: &str,
        role: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url()
            .endpoint("organization")
            .add_path(org_id)
            .endpoint("member")
            .build();

        debug!("POST {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("add_organization_member".to_string()),
            format!("/api/organization/{}/member", org_id)
        ).with_context(None, None);

        let body = json!({
            "user_id": user_id,
            "role": role
        });

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("failed to add organization member")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("add organization member operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("add organization member failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully added organization member in {:?}", start_time.elapsed());

        Ok(response)
    }

    /// Removes a member from an organization.
    ///
    /// # Arguments
    /// * `org_id` - Organization identifier
    /// * `user_id` - User identifier to remove
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.remove_organization_member(
    ///     "acme_corp",
    ///     "john_doe"
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.organization.remove_member",
        skip(self),
        fields(
            org_id = %org_id,
            user_id = %user_id
        ),
        err
    )]
    pub async fn remove_organization_member(
        &self,
        org_id: &str,
        user_id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url()
            .endpoint("organization")
            .add_path(org_id)
            .endpoint("member")
            .add_path(user_id)
            .build();

        debug!("DELETE {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("remove_organization_member".to_string()),
            format!("/api/organization/{}/member/{}", org_id, user_id)
        ).with_context(None, None);

        let res = self
            .http
            .delete(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to remove organization member")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("remove organization member operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("remove organization member failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully removed organization member in {:?}", start_time.elapsed());

        Ok(response)
    }
}