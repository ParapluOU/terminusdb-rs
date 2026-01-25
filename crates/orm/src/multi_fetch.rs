//! Multi-type document fetching.
//!
//! Provides the core trait and implementation for fetching multiple documents
//! of potentially different types in a single API call.

use std::collections::HashSet;

use terminusdb_client::{BranchSpec, GetOpts, TerminusDBHttpClient};

use crate::{result::OrmResult, ClientProvider, GlobalClient};

/// Trait for fetching multiple documents of mixed types.
///
/// This is the core trait that enables efficient batch loading of related
/// entities. A single `get_documents` call is used to fetch all IDs,
/// regardless of type, and results are routed by their `@type` field.
#[async_trait::async_trait]
pub trait MultiTypeFetch {
    /// Fetch documents by their IDs.
    ///
    /// # Arguments
    /// * `ids` - Vector of document IDs (can be mixed types)
    /// * `spec` - Branch specification for the query
    /// * `opts` - Get options (unfold, prefixes, etc.)
    ///
    /// # Returns
    /// An `OrmResult` containing all fetched documents, accessible via
    /// type-specific `.get::<T>()` calls.
    async fn fetch_by_ids(
        &self,
        ids: Vec<String>,
        spec: &BranchSpec,
        opts: GetOpts,
    ) -> anyhow::Result<OrmResult>;

    /// Fetch documents by their IDs with default options.
    async fn fetch_by_ids_default(
        &self,
        ids: Vec<String>,
        spec: &BranchSpec,
    ) -> anyhow::Result<OrmResult> {
        self.fetch_by_ids(ids, spec, GetOpts::default()).await
    }
}

/// Core implementation of multi-type fetching.
async fn fetch_by_ids_impl(
    client: &TerminusDBHttpClient,
    ids: Vec<String>,
    spec: &BranchSpec,
    opts: GetOpts,
) -> anyhow::Result<OrmResult> {
    if ids.is_empty() {
        return Ok(OrmResult::empty());
    }

    // Deduplicate IDs to avoid redundant fetches
    let unique_ids: Vec<String> = ids
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    // Single API call for all IDs, regardless of type
    let documents = client.get_documents(unique_ids, spec, opts).await?;

    Ok(OrmResult::new(documents))
}

#[async_trait::async_trait]
impl MultiTypeFetch for TerminusDBHttpClient {
    async fn fetch_by_ids(
        &self,
        ids: Vec<String>,
        spec: &BranchSpec,
        opts: GetOpts,
    ) -> anyhow::Result<OrmResult> {
        fetch_by_ids_impl(self, ids, spec, opts).await
    }
}

#[async_trait::async_trait]
impl MultiTypeFetch for GlobalClient {
    async fn fetch_by_ids(
        &self,
        ids: Vec<String>,
        spec: &BranchSpec,
        opts: GetOpts,
    ) -> anyhow::Result<OrmResult> {
        fetch_by_ids_impl(self.client(), ids, spec, opts).await
    }
}

#[async_trait::async_trait]
impl MultiTypeFetch for &TerminusDBHttpClient {
    async fn fetch_by_ids(
        &self,
        ids: Vec<String>,
        spec: &BranchSpec,
        opts: GetOpts,
    ) -> anyhow::Result<OrmResult> {
        fetch_by_ids_impl(self, ids, spec, opts).await
    }
}

/// Convenience function to fetch using the global client.
pub async fn fetch_by_ids(
    ids: Vec<String>,
    spec: &BranchSpec,
    opts: GetOpts,
) -> anyhow::Result<OrmResult> {
    GlobalClient.fetch_by_ids(ids, spec, opts).await
}

/// Convenience function to fetch using the global client with default options.
pub async fn fetch_by_ids_default(
    ids: Vec<String>,
    spec: &BranchSpec,
) -> anyhow::Result<OrmResult> {
    GlobalClient.fetch_by_ids_default(ids, spec).await
}

/// Builder for constructing multi-type fetch requests.
///
/// Collects IDs from various sources and executes a single batch fetch.
///
/// # Example
/// ```ignore
/// let result = FetchBuilder::new()
///     .add_ids(comment_ids)
///     .add_ids(reply_ids)
///     .execute(&spec)
///     .await?;
/// ```
pub struct FetchBuilder<C: ClientProvider = GlobalClient> {
    ids: Vec<String>,
    opts: GetOpts,
    client: C,
}

impl FetchBuilder<GlobalClient> {
    /// Create a new fetch builder using the global client.
    pub fn new() -> Self {
        Self {
            ids: Vec::new(),
            opts: GetOpts::default(),
            client: GlobalClient,
        }
    }
}

impl Default for FetchBuilder<GlobalClient> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: ClientProvider> FetchBuilder<C> {
    /// Create a new fetch builder with a specific client.
    pub fn with_client(client: C) -> Self {
        Self {
            ids: Vec::new(),
            opts: GetOpts::default(),
            client,
        }
    }

    /// Add a single ID to fetch.
    pub fn add_id(mut self, id: impl Into<String>) -> Self {
        self.ids.push(id.into());
        self
    }

    /// Add multiple IDs to fetch.
    pub fn add_ids(mut self, ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.ids.extend(ids.into_iter().map(|id| id.into()));
        self
    }

    /// Set the get options.
    pub fn opts(mut self, opts: GetOpts) -> Self {
        self.opts = opts;
        self
    }

    /// Enable unfolding of linked documents.
    pub fn unfold(mut self) -> Self {
        self.opts.unfold = true;
        self
    }

    /// Get the current list of IDs.
    pub fn ids(&self) -> &[String] {
        &self.ids
    }

    /// Get the number of IDs.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Check if no IDs have been added.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Execute the fetch and return results.
    pub async fn execute(self, spec: &BranchSpec) -> anyhow::Result<OrmResult>
    where
        C: MultiTypeFetch + Sync,
    {
        self.client.fetch_by_ids(self.ids, spec, self.opts).await
    }
}
