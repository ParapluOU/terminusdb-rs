//! Diff and patch operations

use {
    crate::{TerminusDBAdapterError, debug::{OperationEntry, OperationType}},
    ::tracing::{debug, error, instrument},
    anyhow::Context,
    serde::{Serialize, Deserialize},
    serde_json::json,
    std::time::Instant,
};

/// Response from a diff operation
#[derive(Debug, Clone, Deserialize)]
pub struct DiffResponse {
    /// Array of diff operations
    pub diffs: Vec<serde_json::Value>,
}

/// Request for applying a patch
#[derive(Debug, Clone, Serialize)]
pub struct PatchRequest {
    /// Author of the patch
    pub author: String,
    /// Message describing the patch
    pub message: String,
    /// The patch operations to apply
    pub patch: Vec<serde_json::Value>,
}

/// Diff and patch operations for the TerminusDB HTTP client
impl super::client::TerminusDBHttpClient {
    /// Gets the diff between two commits or branches.
    ///
    /// # Arguments
    /// * `before` - Path to first commit/branch (e.g., "admin/mydb/local/commit/abc123")
    /// * `after` - Path to second commit/branch (e.g., "admin/mydb/local/branch/main")
    /// * `document_id` - Optional specific document ID to diff
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let diff = client.diff(
    ///     "admin/mydb/local/commit/abc123",
    ///     "admin/mydb/local/branch/main",
    ///     None
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.diff.get",
        skip(self),
        fields(
            before = %before,
            after = %after,
            document_id = ?document_id
        ),
        err
    )]
    pub async fn diff(
        &self,
        before: &str,
        after: &str,
        document_id: Option<&str>,
    ) -> anyhow::Result<DiffResponse> {
        let start_time = Instant::now();
        let mut uri_builder = self.build_url().endpoint("diff");

        debug!("POST /api/diff");

        let mut operation = OperationEntry::new(
            OperationType::Other("diff".to_string()),
            "/api/diff".to_string()
        ).with_context(None, None);

        let mut body = json!({
            "before": before,
            "after": after
        });

        if let Some(doc_id) = document_id {
            body["document_id"] = json!(doc_id);
        }

        // Apply rate limiting for read operations (diff is conceptually a read)
        let _permit = self.acquire_read_permit().await;

        let res = self
            .http
            .post(uri_builder.build())
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("failed to get diff")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("diff operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("diff failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<DiffResponse>(res).await?;
        
        operation = operation.success(Some(response.diffs.len()), duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully got diff with {} operations in {:?}", response.diffs.len(), start_time.elapsed());

        Ok(response)
    }

    /// Applies a patch to a branch.
    ///
    /// # Arguments
    /// * `branch_path` - Path to the branch (e.g., "admin/mydb/local/branch/main")
    /// * `patch` - Array of patch operations to apply
    /// * `author` - Author of the patch
    /// * `message` - Message describing the patch
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let patch = vec![
    ///     json!({
    ///         "op": "add",
    ///         "path": "/Person/123",
    ///         "value": { "name": "John Doe" }
    ///     })
    /// ];
    /// client.patch(
    ///     "admin/mydb/local/branch/main",
    ///     &patch,
    ///     "admin",
    ///     "Apply patch to add new person"
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.patch.apply",
        skip(self, patch),
        fields(
            branch_path = %branch_path,
            patch_operations = patch.len(),
            author = %author,
            message = %message
        ),
        err
    )]
    pub async fn patch(
        &self,
        branch_path: &str,
        patch: &[serde_json::Value],
        author: &str,
        message: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("patch").add_path(branch_path).build();

        debug!("POST {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("patch".to_string()),
            format!("/api/patch/{}", branch_path)
        ).with_context(None, None);

        let patch_request = PatchRequest {
            author: author.to_string(),
            message: message.to_string(),
            patch: patch.to_vec(),
        };

        // Apply rate limiting for write operations
        let _permit = self.acquire_write_permit().await;

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .json(&patch_request)
            .send()
            .await
            .context("failed to apply patch")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("patch operation failed with status {}", status);
            
            let error_text = res.text().await?;
            let error_msg = format!("patch failed: {:#?}", error_text);
            
            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);
            
            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;
        
        operation = operation.success(Some(patch.len()), duration_ms);
        self.operation_log.push(operation);

        debug!("Successfully applied patch with {} operations in {:?}", patch.len(), start_time.elapsed());

        Ok(response)
    }
}