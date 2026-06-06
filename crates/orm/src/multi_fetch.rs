//! Multi-type document fetching.
//!
//! Provides the core trait and implementation for fetching multiple documents
//! of potentially different types in a single API call.

use std::collections::{HashMap, HashSet};

use terminusdb_client::{BranchSpec, GetOpts, TerminusDBHttpClient};

use crate::{result::OrmResult, ClientProvider, GlobalClient};

/// Normalize an id / `@id` to its `Type/local` form for order matching:
/// strips a leading `terminusdb:///<graph>/` IRI prefix if present, so a short
/// id (`Page/abc`) and the full IRI TDB returns (`terminusdb:///data/Page/abc`)
/// compare equal. Url-encoded characters inside lexical-key locals are left
/// untouched (they aren't real `/` separators).
fn normalize_id(id: &str) -> &str {
    match id.find("///") {
        Some(idx) => {
            let after = &id[idx + 3..]; // e.g. `data/Page/abc`
            match after.find('/') {
                Some(slash) => &after[slash + 1..], // drop the graph segment
                None => after,
            }
        }
        None => id,
    }
}

/// Reorder fetched documents to match the requested id order. Documents whose
/// id isn't in `order` (shouldn't happen) are appended at the end rather than
/// dropped. Duplicate ids in `order` each consume one matching document.
fn reorder_documents_to(
    order: &[String],
    documents: Vec<serde_json::Value>,
) -> Vec<serde_json::Value> {
    let mut by_id: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
    let mut leftover = Vec::new();
    for doc in documents {
        match doc.get("@id").and_then(|v| v.as_str()) {
            Some(id) => by_id
                .entry(normalize_id(id).to_string())
                .or_default()
                .push(doc),
            None => leftover.push(doc),
        }
    }
    let mut out = Vec::with_capacity(order.len());
    for id in order {
        if let Some(docs) = by_id.get_mut(normalize_id(id)) {
            if !docs.is_empty() {
                out.push(docs.remove(0));
            }
        }
    }
    for docs in by_id.into_values() {
        out.extend(docs);
    }
    out.extend(leftover);
    out
}

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

    // Deduplicate IDs while PRESERVING first-seen order, so a Phase-1 query with
    // `orderBy` keeps its ordering through this Phase-2 batch fetch.
    let mut seen = HashSet::new();
    let mut unique_ids: Vec<String> = Vec::with_capacity(ids.len());
    for id in ids {
        if seen.insert(id.clone()) {
            unique_ids.push(id);
        }
    }

    // Single API call for all IDs, regardless of type
    let documents = client.get_documents(unique_ids.clone(), spec, opts).await?;

    // TDB's batch fetch does not guarantee documents come back in the requested
    // id order, so re-sort to `unique_ids`. Without this an ordered query's sort
    // would be silently lost here.
    let documents = reorder_documents_to(&unique_ids, documents);

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
