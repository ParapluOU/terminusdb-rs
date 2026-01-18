//! Filter-based query builder for ORM operations.
//!
//! Provides a query API that uses TerminusDB's GraphQL filter types instead of
//! explicit IDs. This enables server-side filtering, reducing data transfer and
//! improving performance.
//!
//! # Example
//! ```ignore
//! use terminusdb_orm::prelude::*;
//!
//! // Query projects with a specific name filter
//! let filter = ProjectFilter {
//!     name: Some(StringFilter {
//!         eq: Some("My Project".to_string()),
//!         ..Default::default()
//!     }),
//!     ..Default::default()
//! };
//!
//! let projects: Vec<Project> = Project::filter(filter)
//!     .limit(10)
//!     .execute(&spec)
//!     .await?;
//! ```

use std::marker::PhantomData;

use serde::Serialize;
use terminusdb_client::{graphql::GraphQLRequest, BranchSpec, GetOpts};
use terminusdb_gql::TdbGQLModel;
use terminusdb_schema::ToSchemaClass;

use crate::{ClientProvider, GlobalClient, MultiTypeFetch, OrmModel, OrmResult};

/// A query builder for loading models using GraphQL filters.
///
/// Created via `Model::filter()` when the model implements `TdbGQLModel`.
pub struct FilterQuery<T, C = GlobalClient>
where
    T: OrmModel + TdbGQLModel,
{
    /// The filter to apply
    filter: Option<T::Filter>,
    /// Optional limit on results
    limit: Option<i32>,
    /// Optional offset for pagination
    offset: Option<i32>,
    /// Get options
    opts: GetOpts,
    /// Client to use
    client: C,
    /// Marker for the primary type
    _phantom: PhantomData<T>,
}

impl<T> FilterQuery<T, GlobalClient>
where
    T: OrmModel + TdbGQLModel + ToSchemaClass,
{
    /// Create a new filter query with the given filter.
    pub fn new(filter: T::Filter) -> Self {
        Self {
            filter: Some(filter),
            limit: None,
            offset: None,
            opts: GetOpts::default(),
            client: GlobalClient,
            _phantom: PhantomData,
        }
    }

    /// Create a filter query that matches all records.
    pub fn all() -> Self {
        Self {
            filter: None,
            limit: None,
            offset: None,
            opts: GetOpts::default(),
            client: GlobalClient,
            _phantom: PhantomData,
        }
    }
}

impl<T, C> FilterQuery<T, C>
where
    T: OrmModel + TdbGQLModel + ToSchemaClass,
    C: ClientProvider,
{
    /// Use a specific client instead of the global one.
    pub fn with_client<C2: ClientProvider>(self, client: C2) -> FilterQuery<T, C2> {
        FilterQuery {
            filter: self.filter,
            limit: self.limit,
            offset: self.offset,
            opts: self.opts,
            client,
            _phantom: PhantomData,
        }
    }

    /// Set a limit on the number of results.
    pub fn limit(mut self, limit: i32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set an offset for pagination.
    pub fn offset(mut self, offset: i32) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Enable unfolding of nested documents.
    pub fn unfold(mut self) -> Self {
        self.opts.unfold = true;
        self
    }

    /// Build the GraphQL query string.
    fn build_query(&self) -> String
    where
        T::Filter: Serialize,
    {
        let type_name = T::to_class();
        let mut args = Vec::new();

        // Add filter if present
        if let Some(filter) = &self.filter {
            let filter_json = serde_json::to_string(filter).unwrap_or_default();
            args.push(format!("filter: {}", filter_json));
        }

        // Add limit if present
        if let Some(limit) = self.limit {
            args.push(format!("limit: {}", limit));
        }

        // Add offset if present
        if let Some(offset) = self.offset {
            args.push(format!("offset: {}", offset));
        }

        let args_str = if args.is_empty() {
            String::new()
        } else {
            format!("({})", args.join(", "))
        };

        format!(
            r#"query {{
  {}{} {{
    _id
  }}
}}"#,
            type_name, args_str
        )
    }

    /// Execute the filter query and return the matching models.
    ///
    /// This performs a two-phase query:
    /// 1. GraphQL query with filter to get matching IDs
    /// 2. Batch fetch of documents by ID
    pub async fn execute(self, spec: &BranchSpec) -> anyhow::Result<Vec<T>>
    where
        T::Filter: Serialize,
        C: MultiTypeFetch + Sync,
    {
        let result = self.execute_result(spec).await?;
        result.get::<T>()
    }

    /// Execute the filter query and return an OrmResult.
    ///
    /// Use this when you need access to multiple types or want to handle
    /// the result more flexibly.
    pub async fn execute_result(self, spec: &BranchSpec) -> anyhow::Result<OrmResult>
    where
        T::Filter: Serialize,
        C: MultiTypeFetch + Sync,
    {
        let type_name = T::to_class();

        // Phase 1: Execute GraphQL query to get IDs
        let query = self.build_query();
        let request = GraphQLRequest::new(&query);

        let response = self
            .client
            .client()
            .execute_graphql::<serde_json::Value>(&spec.db, spec.branch.as_deref(), request, None)
            .await?;

        // Check for errors
        if let Some(errors) = &response.errors {
            if !errors.is_empty() {
                let error_msgs: Vec<_> = errors.iter().map(|e| e.message.clone()).collect();
                return Err(anyhow::anyhow!("GraphQL errors: {:?}", error_msgs));
            }
        }

        let data = response
            .data
            .ok_or_else(|| anyhow::anyhow!("No GraphQL data returned"))?;

        // Extract IDs from the response
        let ids = extract_ids_from_response(&data, &type_name);

        if ids.is_empty() {
            return Ok(OrmResult::empty());
        }

        // Phase 2: Fetch documents by ID
        self.client.fetch_by_ids(ids, spec, self.opts).await
    }

    /// Execute the query and return a single result.
    ///
    /// Returns `None` if no results are found, or an error if multiple are found.
    pub async fn execute_one(self, spec: &BranchSpec) -> anyhow::Result<Option<T>>
    where
        T::Filter: Serialize,
        C: MultiTypeFetch + Sync,
    {
        let results = self.execute(spec).await?;
        match results.len() {
            0 => Ok(None),
            1 => Ok(Some(results.into_iter().next().unwrap())),
            n => Err(anyhow::anyhow!(
                "Expected at most 1 result, got {}",
                n
            )),
        }
    }
}

/// Extract IDs from a GraphQL filter query response.
fn extract_ids_from_response(data: &serde_json::Value, type_name: &str) -> Vec<String> {
    let mut ids = Vec::new();

    if let Some(array) = data.get(type_name).and_then(|v| v.as_array()) {
        for item in array {
            if let Some(id) = item.get("_id").and_then(|v| v.as_str()) {
                ids.push(id.to_string());
            }
        }
    }

    ids
}

/// Extension trait that adds filter query methods to models implementing TdbGQLModel.
pub trait FilterExt: OrmModel + TdbGQLModel + ToSchemaClass {
    /// Query using a filter.
    ///
    /// # Example
    /// ```ignore
    /// let filter = ProjectFilter {
    ///     name: Some(StringFilter { eq: Some("Test".into()), ..Default::default() }),
    ///     ..Default::default()
    /// };
    /// let projects = Project::filter(filter).execute(&spec).await?;
    /// ```
    fn filter(filter: Self::Filter) -> FilterQuery<Self, GlobalClient>
    where
        Self: Sized,
    {
        FilterQuery::new(filter)
    }

    /// Query all records (no filter).
    ///
    /// # Example
    /// ```ignore
    /// let all_projects = Project::all().limit(100).execute(&spec).await?;
    /// ```
    fn all() -> FilterQuery<Self, GlobalClient>
    where
        Self: Sized,
    {
        FilterQuery::all()
    }
}

// Blanket implementation for all models that implement TdbGQLModel
impl<T> FilterExt for T where T: OrmModel + TdbGQLModel + ToSchemaClass {}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full tests require generated filter types and a running TerminusDB instance
    // These are structural tests only

    #[test]
    fn test_extract_ids_from_response() {
        let response = serde_json::json!({
            "Project": [
                { "_id": "Project/1" },
                { "_id": "Project/2" },
                { "_id": "Project/3" }
            ]
        });

        let ids = extract_ids_from_response(&response, "Project");
        assert_eq!(ids, vec!["Project/1", "Project/2", "Project/3"]);
    }

    #[test]
    fn test_extract_ids_empty_response() {
        let response = serde_json::json!({
            "Project": []
        });

        let ids = extract_ids_from_response(&response, "Project");
        assert!(ids.is_empty());
    }
}
