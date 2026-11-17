//! Diff and patch operations

use {
    crate::debug::{OperationEntry, OperationType},
    ::tracing::{debug, error, instrument},
    anyhow::Context,
    serde::{Deserialize, Serialize},
    serde_json::json,
    std::time::Instant,
};

/// Response from a diff operation
#[derive(Debug, Clone, Deserialize)]
pub struct DiffResponse {
    /// Array of diff operations
    pub diffs: Vec<serde_json::Value>,
}

/// Options for the diff endpoint (`keep` / `copy_value`).
#[derive(Debug, Clone, Default)]
pub struct DiffOptions {
    /// A document describing which fields *must* be copied into the final
    /// object even when unchanged, e.g. `{"@id": true}`.
    pub keep: Option<serde_json::Value>,
    /// Record the exact value in list-copy operations (makes copies explicit).
    pub copy_value: bool,
    /// TerminusDB 12.1 `unfold` (server default `true`): compare documents
    /// with their subdocuments/unfoldable links expanded. Set to `false` to
    /// diff documents exactly as stored, so a changed subdocument shows up as
    /// a changed *link* rather than as a nested diff.
    pub unfold: Option<bool>,
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
        let uri_builder = self.build_url().endpoint("diff");

        debug!("POST /api/diff");

        let mut operation = OperationEntry::new(
            OperationType::Other("diff".to_string()),
            "/api/diff".to_string(),
        )
        .with_context(None, None);

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

        debug!(
            "Successfully got diff with {} operations in {:?}",
            response.diffs.len(),
            start_time.elapsed()
        );

        Ok(response)
    }

    /// Diff two database states identified by their `TerminusDB-Data-Version`
    /// (the value returned in that header on a commit, e.g.
    /// `"branch:s7dde27..."`), covering the v12 data-version diff body forms:
    ///
    /// - `document_id = Some(id)` → diff just that document between the two
    ///   versions.
    /// - `document_id = None` → diff **all** documents between the two versions.
    ///
    /// Returns the raw diff JSON (an object for a single document, an array for
    /// the all-documents form).
    pub async fn diff_data_versions(
        &self,
        spec: &crate::spec::BranchSpec,
        before_data_version: &str,
        after_data_version: &str,
        document_id: Option<&str>,
        options: DiffOptions,
    ) -> anyhow::Result<serde_json::Value> {
        let mut body = json!({
            "before_data_version": before_data_version,
            "after_data_version": after_data_version,
        });
        if let Some(doc_id) = document_id {
            body["document_id"] = json!(doc_id);
        }
        if let Some(keep) = &options.keep {
            body["keep"] = keep.clone();
        }
        if options.copy_value {
            body["copy_value"] = json!(true);
        }
        if let Some(unfold) = options.unfold {
            body["unfold"] = json!(unfold);
        }

        let _permit = self.acquire_read_permit().await;
        let res = self
            .http
            .post(self.build_url().endpoint("diff").database(spec).build())
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("failed to diff data versions")?;
        self.parse_response(res).await
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
        let uri = self
            .build_url()
            .endpoint("patch")
            .add_path(branch_path)
            .build();

        debug!("POST {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("patch".to_string()),
            format!("/api/patch/{}", branch_path),
        )
        .with_context(None, None);

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

        debug!(
            "Successfully applied patch with {} operations in {:?}",
            patch.len(),
            start_time.elapsed()
        );

        Ok(response)
    }
}

/// A typed entry in a `POST /api/patch` patch list (TerminusDB 12.1).
///
/// The patch endpoint accepts three shapes, which this enum keeps apart:
///
/// * **Insert** — `{"@op": "Insert", "@insert": {...}}`, creating a whole
///   document. The inserted document is identified either by its `@id` or by a
///   `@capture` name, which other entries in the same patch can point at with
///   [`PatchOperation::reference`].
/// * **Delete** — `{"@op": "Delete", "@delete": {"@id": "..."}}`.
/// * **Field-level diffs** — a document-shaped entry carrying per-field
///   operations such as `SwapValue`, e.g.
///   `{"@id": "Person/jane", "name": {"@op": "SwapValue", "@before": "Jane",
///   "@after": "Janet"}}`.
#[derive(Debug, Clone)]
pub enum PatchOperation {
    /// Insert a whole document.
    Insert {
        /// The document to insert, including its `@type` and either an `@id` or
        /// a `@capture`.
        document: serde_json::Value,
    },
    /// Delete a whole document by id.
    Delete {
        /// The document id, e.g. `"Person/jane"`.
        id: String,
    },
    /// Replace one field's value on an existing document.
    SwapValue {
        /// The document id.
        id: String,
        /// The field (property) name.
        field: String,
        /// The value the field currently holds. The server refuses the patch
        /// with a conflict if it does not match.
        before: serde_json::Value,
        /// The value to write.
        after: serde_json::Value,
    },
    /// Any other patch entry, passed through verbatim.
    Raw(serde_json::Value),
}

impl PatchOperation {
    /// Insert a document that already carries its own `@id`.
    pub fn insert(document: serde_json::Value) -> Self {
        PatchOperation::Insert { document }
    }

    /// Insert a document under a `@capture` name instead of an explicit `@id`,
    /// so the server mints the id and other entries in the same patch can link
    /// to it through [`PatchOperation::reference`].
    ///
    /// Two inserts may reference each other, which is the only way to create a
    /// cycle in a single transaction.
    pub fn insert_captured(capture: &str, mut document: serde_json::Value) -> Self {
        if let Some(obj) = document.as_object_mut() {
            obj.insert(
                "@capture".to_string(),
                serde_json::Value::String(capture.to_string()),
            );
        }
        PatchOperation::Insert { document }
    }

    /// Delete a document by id.
    pub fn delete(id: impl Into<String>) -> Self {
        PatchOperation::Delete { id: id.into() }
    }

    /// Replace a single field value.
    pub fn swap_value(
        id: impl Into<String>,
        field: impl Into<String>,
        before: serde_json::Value,
        after: serde_json::Value,
    ) -> Self {
        PatchOperation::SwapValue {
            id: id.into(),
            field: field.into(),
            before,
            after,
        }
    }

    /// A link to a document inserted under `capture` elsewhere in the same
    /// patch: `{"@ref": "<capture>"}`.
    pub fn reference(capture: &str) -> serde_json::Value {
        json!({ "@ref": capture })
    }

    /// Render this operation as the JSON the patch endpoint expects.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            PatchOperation::Insert { document } => json!({
                "@op": "Insert",
                "@insert": document,
            }),
            PatchOperation::Delete { id } => json!({
                "@op": "Delete",
                "@delete": { "@id": id },
            }),
            PatchOperation::SwapValue {
                id,
                field,
                before,
                after,
            } => {
                let mut entry = serde_json::Map::new();
                entry.insert("@id".to_string(), json!(id));
                entry.insert(
                    field.clone(),
                    json!({
                        "@op": "SwapValue",
                        "@before": before,
                        "@after": after,
                    }),
                );
                serde_json::Value::Object(entry)
            }
            PatchOperation::Raw(value) => value.clone(),
        }
    }
}

impl super::client::TerminusDBHttpClient {
    /// Apply a list of typed [`PatchOperation`]s to a branch.
    ///
    /// A thin typed wrapper over [`patch`](Self::patch): it renders each
    /// operation to the wire shape so callers do not hand-build
    /// `@op`/`@insert`/`@delete` objects.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # use serde_json::json;
    /// # async fn example(client: TerminusDBHttpClient) -> anyhow::Result<()> {
    /// client
    ///     .patch_typed(
    ///         "admin/mydb/local/branch/main",
    ///         &[
    ///             PatchOperation::insert(json!({
    ///                 "@id": "City/Leipzig", "@type": "City", "name": "Leipzig"
    ///             })),
    ///             PatchOperation::delete("City/Warsaw"),
    ///         ],
    ///         "admin",
    ///         "swap one city for another",
    ///     )
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn patch_typed(
        &self,
        branch_path: &str,
        operations: &[PatchOperation],
        author: &str,
        message: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let patch: Vec<serde_json::Value> = operations.iter().map(|op| op.to_json()).collect();
        self.patch(branch_path, &patch, author, message).await
    }
}
