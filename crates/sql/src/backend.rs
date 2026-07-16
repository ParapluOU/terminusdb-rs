//! The platform seam: the small async trait the catalog loader and runner need
//! from a TerminusDB-shaped backend.
//!
//! Keeping DB access behind a trait (rather than depending concretely on the HTTP
//! client) lets the SQL layer be backed by the HTTP client today and an internal
//! engine later, and keeps `terminusdb-client` an *optional* dependency. The trait
//! speaks only `serde_json`/`String`, so it carries no client-specific types.

use std::collections::HashMap;

use serde_json::Value;

use crate::error::Result;

/// Everything the SQL layer needs from a backend: resolve a branch to a commit,
/// read the schema graph at that commit, and execute a WOQL query pinned to it.
#[async_trait::async_trait]
pub trait CatalogBackend {
    /// Validate that `db`/`branch` exist and auth is valid, and resolve the branch
    /// (default `main`) to a concrete commit id.
    async fn resolve_commit(&self, db: &str, branch: Option<&str>) -> Result<String>;

    /// Read the authored schema-graph documents at the pinned `commit`.
    async fn read_schema_documents(&self, db: &str, commit: &str) -> Result<Vec<Value>>;

    /// Execute a WOQL query (as `to_woql_json`) pinned to `commit`, returning one
    /// binding map per solution. The implementation is responsible for pinning
    /// (e.g. wrapping in a `Using` over the commit resource).
    async fn execute_woql(
        &self,
        db: &str,
        commit: &str,
        woql: Value,
    ) -> Result<Vec<HashMap<String, Value>>>;
}

#[cfg(feature = "client")]
mod client_impl {
    use super::*;
    use crate::error::SqlError;
    use terminusdb_client::{BranchSpec, TerminusDBHttpClient, WOQLResult};

    #[async_trait::async_trait]
    impl CatalogBackend for TerminusDBHttpClient {
        async fn resolve_commit(&self, db: &str, branch: Option<&str>) -> Result<String> {
            if !self
                .database_exists(db)
                .await
                .map_err(|e| SqlError::SchemaRead(e.to_string()))?
            {
                return Err(SqlError::DatabaseNotFound(db.to_string()));
            }
            let branch_name = branch.unwrap_or("main");
            let spec = BranchSpec::with_branch(db, branch_name);
            // A single call that proves the branch exists and auth is valid, and
            // yields the concrete commit to pin.
            let commit = self.get_latest_commit_id(&spec).await.map_err(|_| {
                SqlError::BranchNotFound(branch_name.to_string(), db.to_string())
            })?;
            Ok(commit.as_str().to_string())
        }

        async fn read_schema_documents(&self, db: &str, commit: &str) -> Result<Vec<Value>> {
            let spec = BranchSpec::with_commit(db, commit);
            self.get_schema_documents(&spec)
                .await
                .map_err(|e| SqlError::SchemaRead(e.to_string()))
        }

        async fn execute_woql(
            &self,
            db: &str,
            commit: &str,
            woql: Value,
        ) -> Result<Vec<HashMap<String, Value>>> {
            // The WOQL executor path ignores the branch/commit on the spec (it
            // targets `{org}/{db}`), so pin by wrapping the query in `Using` over
            // the commit resource. The resource string is built here, inside the
            // client impl, so the SQL crate never hand-formats it.
            let collection = format!("{}/{}/local/commit/{}", self.org(), db, commit);
            let pinned = serde_json::json!({
                "@type": "Using",
                "collection": collection,
                "query": woql,
            });
            let result: WOQLResult<HashMap<String, Value>> = self
                .query_raw(Some(BranchSpec::new(db)), pinned, None)
                .await
                .map_err(|e| SqlError::Plan(e.to_string()))?;
            Ok(result.bindings)
        }
    }
}
