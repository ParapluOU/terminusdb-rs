//! Branch-scoped client for atomic merge branch operations
//!
//! `BranchClient` wraps a `TerminusDBHttpClient` and locks all operations
//! to a specific temporary branch. It deliberately does NOT implement `Deref`
//! to ensure only whitelisted operations are available.

use {
    super::{
        client::TerminusDBHttpClient,
        document::DeleteOpts,
        InsertInstanceResult,
        TDBInsertInstanceResult,
        TerminusDBModel,
    },
    crate::{
        CommitId,
        document::{DocumentInsertArgs, GetOpts},
        result::ResponseWithHeaders,
        spec::BranchSpec,
        TDBInstanceDeserializer,
    },
    std::{collections::HashMap, fmt::Debug, time::Duration},
    terminusdb_schema::{InstanceFromJson, ToTDBInstance, ToTDBSchema},
    terminusdb_woql2::prelude::Query as Woql2Query,
};

#[cfg(not(target_arch = "wasm32"))]
use crate::log::{LogEntry, LogOpts};

/// A client bound to a specific temporary branch for atomic operations.
///
/// This client does NOT implement `Deref` to `TerminusDBHttpClient` to ensure
/// tight control over which operations are allowed. Only read/write data
/// operations are exposed - no branch or database management.
///
/// Operations that would normally take a `BranchSpec` parameter are automatically
/// scoped to the working branch.
#[derive(Clone)]
pub struct BranchClient {
    /// The underlying HTTP client (owned clone)
    client: TerminusDBHttpClient,
    /// The working branch spec (points to temporary branch)
    working_spec: BranchSpec,
    /// The target branch spec (where changes will be merged)
    #[allow(dead_code)]
    target_spec: BranchSpec,
    /// The temporary branch name (for reference)
    #[allow(dead_code)]
    temp_branch_name: String,
}

impl BranchClient {
    /// Create a new BranchClient
    pub(crate) fn new(
        client: TerminusDBHttpClient,
        working_spec: BranchSpec,
        target_spec: BranchSpec,
        temp_branch_name: String,
    ) -> Self {
        Self {
            client,
            working_spec,
            target_spec,
            temp_branch_name,
        }
    }

    /// Get the working branch spec for this client
    pub fn spec(&self) -> &BranchSpec {
        &self.working_spec
    }

    /// Get a clone of the working branch spec
    pub fn spec_clone(&self) -> BranchSpec {
        self.working_spec.clone()
    }

    // =========================================================================
    // Instance Operations - Read
    // =========================================================================

    /// Check if a strongly-typed model instance exists
    pub async fn has_instance<I: TerminusDBModel>(&self, model: &I) -> bool {
        self.client.has_instance(model, &self.working_spec).await
    }

    /// Check if an instance exists by ID
    pub async fn has_instance_id<I: TerminusDBModel>(&self, model_id: &str) -> bool {
        self.client
            .has_instance_id::<I>(model_id, &self.working_spec)
            .await
    }

    /// Get a strongly-typed model instance by ID
    pub async fn get_instance<Target: ToTDBInstance>(
        &self,
        id: &str,
        deserializer: &mut impl TDBInstanceDeserializer<Target>,
    ) -> anyhow::Result<Target> {
        self.client
            .get_instance(id, &self.working_spec, deserializer)
            .await
    }

    /// Get a strongly-typed model instance with unfolding
    pub async fn get_instance_unfolded<Target: ToTDBInstance>(
        &self,
        id: &str,
        deserializer: &mut impl TDBInstanceDeserializer<Target>,
    ) -> anyhow::Result<Target> {
        self.client
            .get_instance_unfolded(id, &self.working_spec, deserializer)
            .await
    }

    /// Get a strongly-typed model instance with custom options
    pub async fn get_instance_with_opts<Target: ToTDBInstance>(
        &self,
        id: &str,
        opts: GetOpts,
        deserializer: &mut impl TDBInstanceDeserializer<Target>,
    ) -> anyhow::Result<Target> {
        self.client
            .get_instance_with_opts(id, &self.working_spec, opts, deserializer)
            .await
    }

    /// Get instance if it exists, return None otherwise
    pub async fn get_instance_if_exists<Target: TerminusDBModel>(
        &self,
        id: &str,
        deserializer: &mut impl TDBInstanceDeserializer<Target>,
    ) -> anyhow::Result<Option<Target>> {
        self.client
            .get_instance_if_exists(id, &self.working_spec, deserializer)
            .await
    }

    /// Get multiple instances by IDs
    pub async fn get_instances<Target: TerminusDBModel>(
        &self,
        ids: Vec<String>,
        opts: GetOpts,
        deserializer: &mut impl TDBInstanceDeserializer<Target>,
    ) -> anyhow::Result<Vec<Target>> {
        self.client
            .get_instances(ids, &self.working_spec, opts, deserializer)
            .await
    }

    /// Get multiple instances with unfolding
    pub async fn get_instances_unfolded<Target: TerminusDBModel>(
        &self,
        ids: Vec<String>,
        deserializer: &mut impl TDBInstanceDeserializer<Target>,
    ) -> anyhow::Result<Vec<Target>> {
        self.client
            .get_instances_unfolded(ids, &self.working_spec, deserializer)
            .await
    }

    /// Get multiple instances with custom options
    pub async fn get_instances_with_opts<Target: TerminusDBModel>(
        &self,
        ids: Vec<String>,
        opts: GetOpts,
        deserializer: &mut impl TDBInstanceDeserializer<Target>,
    ) -> anyhow::Result<Vec<Target>> {
        self.client
            .get_instances_with_opts(ids, &self.working_spec, opts, deserializer)
            .await
    }

    // =========================================================================
    // Instance Operations - Write
    // =========================================================================

    /// Save (insert or update) a strongly-typed model instance
    ///
    /// The `args.spec` will be overridden to use the working branch.
    pub async fn save_instance<I: TerminusDBModel>(
        &self,
        model: &I,
        mut args: DocumentInsertArgs,
    ) -> anyhow::Result<InsertInstanceResult> {
        args.spec = self.working_spec.clone();
        self.client.save_instance(model, args).await
    }

    /// Create a new instance (fails if already exists)
    ///
    /// The `args.spec` will be overridden to use the working branch.
    pub async fn create_instance<I: TerminusDBModel>(
        &self,
        model: &I,
        mut args: DocumentInsertArgs,
    ) -> anyhow::Result<InsertInstanceResult> {
        args.spec = self.working_spec.clone();
        self.client.create_instance(model, args).await
    }

    /// Update an existing instance
    ///
    /// The `args.spec` will be overridden to use the working branch.
    pub async fn update_instance<I: TerminusDBModel>(
        &self,
        model: &I,
        mut args: DocumentInsertArgs,
    ) -> anyhow::Result<InsertInstanceResult> {
        args.spec = self.working_spec.clone();
        self.client.update_instance(model, args).await
    }

    /// Insert an instance and get the commit ID
    ///
    /// The `args.spec` will be overridden to use the working branch.
    pub async fn insert_instance_with_commit_id<I: TerminusDBModel>(
        &self,
        model: &I,
        mut args: DocumentInsertArgs,
    ) -> anyhow::Result<(InsertInstanceResult, CommitId)> {
        args.spec = self.working_spec.clone();
        self.client.insert_instance_with_commit_id(model, args).await
    }

    /// Insert multiple instances
    ///
    /// The `args.spec` will be overridden to use the working branch.
    pub async fn insert_instances<M: crate::IntoBoxedTDBInstances>(
        &self,
        models: M,
        mut args: DocumentInsertArgs,
    ) -> anyhow::Result<ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>> {
        args.spec = self.working_spec.clone();
        self.client.insert_instances(models, args).await
    }

    /// Delete an instance by model reference
    ///
    /// The `args.spec` will be overridden to use the working branch.
    pub async fn delete_instance<T: TerminusDBModel>(
        &self,
        model: &T,
        mut args: DocumentInsertArgs,
        opts: DeleteOpts,
    ) -> anyhow::Result<TerminusDBHttpClient> {
        args.spec = self.working_spec.clone();
        self.client.delete_instance(model, args, opts).await
    }

    /// Delete an instance by ID
    ///
    /// The `args.spec` will be overridden to use the working branch.
    pub async fn delete_instance_by_id<T: TerminusDBModel>(
        &self,
        id: &str,
        mut args: DocumentInsertArgs,
        opts: DeleteOpts,
    ) -> anyhow::Result<TerminusDBHttpClient> {
        args.spec = self.working_spec.clone();
        self.client.delete_instance_by_id::<T>(id, args, opts).await
    }

    // =========================================================================
    // Query Operations
    // =========================================================================

    /// Execute a WOQL query
    pub async fn query<T: Debug + serde::de::DeserializeOwned>(
        &self,
        query: Woql2Query,
    ) -> anyhow::Result<crate::WOQLResult<T>> {
        self.client
            .query(Some(self.working_spec.clone()), query)
            .await
    }

    /// Execute a raw WOQL query with custom variables
    pub async fn query_raw<T: Debug + serde::de::DeserializeOwned>(
        &self,
        woql: serde_json::Value,
        timeout: Option<Duration>,
    ) -> anyhow::Result<crate::WOQLResult<T>> {
        self.client
            .query_raw(Some(self.working_spec.clone()), woql, timeout)
            .await
    }

    /// Execute a WOQL query from a string
    pub async fn query_string<T: Debug + serde::de::DeserializeOwned>(
        &self,
        woql: &str,
        timeout: Option<Duration>,
    ) -> anyhow::Result<crate::WOQLResult<T>> {
        self.client
            .query_string(Some(self.working_spec.clone()), woql, timeout)
            .await
    }

    /// List instances of a type using WOQL
    pub async fn list_instances<T: TerminusDBModel + InstanceFromJson>(
        &self,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> anyhow::Result<Vec<T>> {
        self.client
            .list_instances(&self.working_spec, limit, offset)
            .await
    }

    /// Count instances of a type
    pub async fn count_instances<T: ToTDBSchema>(
        &self,
    ) -> anyhow::Result<usize> {
        self.client
            .count_instances::<T>(&self.working_spec)
            .await
    }

    // =========================================================================
    // Document Operations
    // =========================================================================

    /// Check if a document exists
    pub async fn has_document(&self, doc_id: &str) -> bool {
        self.client.has_document(doc_id, &self.working_spec).await
    }

    /// Get a document by ID
    pub async fn get_document(
        &self,
        doc_id: &str,
        opts: GetOpts,
    ) -> anyhow::Result<serde_json::Value> {
        self.client
            .get_document(doc_id, &self.working_spec, opts)
            .await
    }

    /// Get a document with response headers
    pub async fn get_document_with_headers(
        &self,
        doc_id: &str,
        opts: GetOpts,
    ) -> anyhow::Result<ResponseWithHeaders<serde_json::Value>> {
        self.client
            .get_document_with_headers(doc_id, &self.working_spec, opts)
            .await
    }

    /// Get multiple documents by IDs
    pub async fn get_documents(
        &self,
        ids: Vec<String>,
        opts: GetOpts,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        self.client
            .get_documents(ids, &self.working_spec, opts)
            .await
    }

    // =========================================================================
    // Log Operations (non-WASM only)
    // =========================================================================

    /// Get commit log
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn log(&self, opts: LogOpts) -> anyhow::Result<Vec<LogEntry>> {
        self.client.log(&self.working_spec, opts).await
    }

    /// Get the latest commit ID for the working branch
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn get_latest_commit_id(&self) -> anyhow::Result<CommitId> {
        self.client.get_latest_commit_id(&self.working_spec).await
    }

    /// Get instance history
    pub async fn get_instance_history<I: TerminusDBModel>(
        &self,
        id: &str,
        params: Option<crate::document::DocumentHistoryParams>,
    ) -> anyhow::Result<Vec<crate::document::CommitHistoryEntry>> {
        self.client
            .get_instance_history::<I>(id, &self.working_spec, params)
            .await
    }

    /// Get document history
    pub async fn get_document_history(
        &self,
        doc_id: &str,
        params: Option<crate::document::DocumentHistoryParams>,
    ) -> anyhow::Result<Vec<crate::document::CommitHistoryEntry>> {
        self.client
            .get_document_history(doc_id, &self.working_spec, params)
            .await
    }
}
