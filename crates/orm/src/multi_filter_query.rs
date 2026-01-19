//! Multi-type filter query builder for ORM operations.
//!
//! Provides a query API that combines multiple entity types with their filters
//! into a single GraphQL query. This enables efficient querying of related
//! data in one round-trip instead of sequential queries.
//!
//! # Example
//! ```ignore
//! use terminusdb_orm::prelude::*;
//!
//! // Build a single query for multiple types with different filters
//! let result = MultiFilterQuery::new()
//!     .query::<Project>(project_filter)
//!     .query::<Ticket>(ticket_filter)
//!     .query::<TicketEdge>(edge_filter)
//!     .query::<Milestone>(milestone_filter)
//!     .execute(&spec)
//!     .await?;
//!
//! // Extract typed results
//! let projects: Vec<Project> = result.get::<Project>()?;
//! let tickets: Vec<Ticket> = result.get::<Ticket>()?;
//! ```
//!
//! # Motivation
//!
//! The `ticket_tree` query previously made 10 sequential database calls:
//! ```text
//! fetch_all_tickets(db)           // ALL tickets
//! fetch_all_projects(db)          // ALL projects
//! fetch_all_companies(db)         // ALL companies
//! ... etc
//! ```
//!
//! With `MultiFilterQuery`, we generate:
//! ```graphql
//! query {
//!   Project(filter: { company: { _id: { eq: "Company/acme" } } }) { _id }
//!   Ticket(filter: { project: { company: { _id: { eq: "Company/acme" } } } }) { _id }
//!   TicketEdge(filter: { from_ticket: { project: { company: { _id: ... } } } }) { _id }
//!   Milestone(filter: { project: { company: { _id: ... } } }) { _id }
//!   Label(filter: { project: { company: { _id: ... } } }) { _id }
//! }
//! ```
//!
//! One GraphQL query → one batch fetch by IDs.

use std::marker::PhantomData;

use serde::Serialize;
use terminusdb_client::{graphql::GraphQLRequest, BranchSpec, GetOpts};
use terminusdb_gql::TdbGQLModel;
use terminusdb_schema::ToSchemaClass;

use crate::{ClientProvider, GlobalClient, MultiTypeFetch, OrmModel, OrmResult};

/// A single type query entry within a multi-filter query.
struct TypeQuery {
    /// The GraphQL type name
    type_name: String,
    /// Filter serialized to GraphQL object syntax (if any)
    filter_gql: Option<String>,
    /// Limit on results for this type
    limit: Option<i32>,
    /// Offset for pagination for this type
    offset: Option<i32>,
}

impl TypeQuery {
    /// Build the GraphQL fragment for this type query.
    fn build_fragment(&self) -> String {
        let mut args = Vec::new();

        if let Some(filter) = &self.filter_gql {
            args.push(format!("filter: {}", filter));
        }
        if let Some(limit) = self.limit {
            args.push(format!("limit: {}", limit));
        }
        if let Some(offset) = self.offset {
            args.push(format!("offset: {}", offset));
        }

        let args_str = if args.is_empty() {
            String::new()
        } else {
            format!("({})", args.join(", "))
        };

        format!("  {}{} {{ _id }}", self.type_name, args_str)
    }
}

/// Builder for a single type within a multi-filter query.
///
/// Allows setting type-specific options like limit and offset.
pub struct TypeQueryBuilder<'a, T, C>
where
    T: OrmModel + TdbGQLModel + ToSchemaClass,
    C: ClientProvider,
{
    parent: &'a mut MultiFilterQuery<C>,
    type_name: String,
    filter: Option<T::Filter>,
    limit: Option<i32>,
    offset: Option<i32>,
    _phantom: PhantomData<T>,
}

impl<'a, T, C> TypeQueryBuilder<'a, T, C>
where
    T: OrmModel + TdbGQLModel + ToSchemaClass,
    T::Filter: Serialize,
    C: ClientProvider,
{
    /// Set a limit on results for this type.
    pub fn limit(mut self, limit: i32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set an offset for pagination for this type.
    pub fn offset(mut self, offset: i32) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Finalize this type query and return to the parent builder.
    pub fn done(self) -> &'a mut MultiFilterQuery<C> {
        let filter_gql = self.filter.map(|f| {
            let filter_json = serde_json::to_value(&f).unwrap_or_default();
            json_to_graphql(&filter_json)
        });

        self.parent.queries.push(TypeQuery {
            type_name: self.type_name,
            filter_gql,
            limit: self.limit,
            offset: self.offset,
        });

        self.parent
    }
}

/// A query builder for loading multiple model types in a single GraphQL query.
///
/// This is the main entry point for multi-type filtered queries. Each type
/// can have its own filter, limit, and offset.
///
/// # Example
/// ```ignore
/// let result = MultiFilterQuery::new()
///     .query::<Project>(project_filter)
///     .query::<Ticket>(ticket_filter)
///     .query_all::<TicketEdge>() // No filter, fetch all
///     .execute(&spec)
///     .await?;
/// ```
pub struct MultiFilterQuery<C = GlobalClient>
where
    C: ClientProvider,
{
    /// The queries to execute
    queries: Vec<TypeQuery>,
    /// Get options for document fetching
    opts: GetOpts,
    /// Client to use
    client: C,
}

impl MultiFilterQuery<GlobalClient> {
    /// Create a new multi-filter query using the global client.
    pub fn new() -> Self {
        Self {
            queries: Vec::new(),
            opts: GetOpts::default(),
            client: GlobalClient,
        }
    }
}

impl Default for MultiFilterQuery<GlobalClient> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: ClientProvider> MultiFilterQuery<C> {
    /// Create a new multi-filter query with a specific client.
    pub fn with_client(client: C) -> Self {
        Self {
            queries: Vec::new(),
            opts: GetOpts::default(),
            client,
        }
    }

    /// Add a type query with the given filter.
    ///
    /// # Example
    /// ```ignore
    /// let mut query = MultiFilterQuery::new();
    /// query.query::<Project>(project_filter);
    /// ```
    pub fn query<T>(&mut self, filter: T::Filter) -> &mut Self
    where
        T: OrmModel + TdbGQLModel + ToSchemaClass,
        T::Filter: Serialize,
    {
        let type_name = T::to_class();
        let filter_json = serde_json::to_value(&filter).unwrap_or_default();
        let filter_gql = json_to_graphql(&filter_json);

        self.queries.push(TypeQuery {
            type_name,
            filter_gql: Some(filter_gql),
            limit: None,
            offset: None,
        });

        self
    }

    /// Add a type query that fetches all records (no filter).
    ///
    /// # Example
    /// ```ignore
    /// let mut query = MultiFilterQuery::new();
    /// query.query_all::<Label>();
    /// ```
    pub fn query_all<T>(&mut self) -> &mut Self
    where
        T: OrmModel + TdbGQLModel + ToSchemaClass,
    {
        self.queries.push(TypeQuery {
            type_name: T::to_class(),
            filter_gql: None,
            limit: None,
            offset: None,
        });

        self
    }

    /// Add a type query with an explicit filter type (no TdbGQLModel needed).
    ///
    /// Use this when the model doesn't implement TdbGQLModel, such as when
    /// filter types are generated in a separate crate due to orphan rules.
    ///
    /// # Example
    /// ```ignore
    /// use paraplu_services::filters::ProjectFilter;
    ///
    /// let mut query = MultiFilterQuery::new();
    /// query.query_with_filter::<Project, _>(project_filter);
    /// ```
    pub fn query_with_filter<T, F>(&mut self, filter: F) -> &mut Self
    where
        T: OrmModel + ToSchemaClass,
        F: Serialize,
    {
        let type_name = T::to_class();
        let filter_json = serde_json::to_value(&filter).unwrap_or_default();
        let filter_gql = json_to_graphql(&filter_json);

        self.queries.push(TypeQuery {
            type_name,
            filter_gql: Some(filter_gql),
            limit: None,
            offset: None,
        });

        self
    }

    /// Add a type query that fetches all records (no TdbGQLModel needed).
    ///
    /// # Example
    /// ```ignore
    /// let mut query = MultiFilterQuery::new();
    /// query.query_all_typed::<Project>();
    /// ```
    pub fn query_all_typed<T>(&mut self) -> &mut Self
    where
        T: OrmModel + ToSchemaClass,
    {
        self.queries.push(TypeQuery {
            type_name: T::to_class(),
            filter_gql: None,
            limit: None,
            offset: None,
        });

        self
    }

    /// Add a type query with detailed options using a builder.
    ///
    /// # Example
    /// ```ignore
    /// let mut query = MultiFilterQuery::new();
    /// query
    ///     .query_builder::<Project>(project_filter)
    ///     .limit(100)
    ///     .offset(0)
    ///     .done();
    /// ```
    pub fn query_builder<T>(&mut self, filter: T::Filter) -> TypeQueryBuilder<'_, T, C>
    where
        T: OrmModel + TdbGQLModel + ToSchemaClass,
        T::Filter: Serialize,
    {
        TypeQueryBuilder {
            parent: self,
            type_name: T::to_class(),
            filter: Some(filter),
            limit: None,
            offset: None,
            _phantom: PhantomData,
        }
    }

    /// Add a type query for all records with detailed options.
    pub fn query_all_builder<T>(&mut self) -> TypeQueryBuilder<'_, T, C>
    where
        T: OrmModel + TdbGQLModel + ToSchemaClass,
        T::Filter: Serialize,
    {
        TypeQueryBuilder {
            parent: self,
            type_name: T::to_class(),
            filter: None,
            limit: None,
            offset: None,
            _phantom: PhantomData,
        }
    }

    /// Enable unfolding of nested documents in the fetch phase.
    pub fn unfold(mut self) -> Self {
        self.opts.unfold = true;
        self
    }

    /// Set the get options for document fetching.
    pub fn opts(mut self, opts: GetOpts) -> Self {
        self.opts = opts;
        self
    }

    /// Build the combined GraphQL query string.
    fn build_query(&self) -> String {
        if self.queries.is_empty() {
            return "query { __typename }".to_string();
        }

        let fragments: Vec<String> = self.queries.iter().map(|q| q.build_fragment()).collect();

        format!("query {{\n{}\n}}", fragments.join("\n"))
    }

    /// Execute the multi-type query and return all matching models.
    ///
    /// This performs a two-phase query:
    /// 1. Single GraphQL query with all filters to get matching IDs per type
    /// 2. Single batch fetch of all documents by ID
    pub async fn execute(self, spec: &BranchSpec) -> anyhow::Result<OrmResult>
    where
        C: MultiTypeFetch + Sync,
    {
        if self.queries.is_empty() {
            return Ok(OrmResult::empty());
        }

        // Phase 1: Execute combined GraphQL query
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

        // Extract all IDs from all type queries
        let mut all_ids: Vec<String> = Vec::new();
        for type_query in &self.queries {
            let ids = extract_ids_from_response(&data, &type_query.type_name);
            all_ids.extend(ids);
        }

        if all_ids.is_empty() {
            return Ok(OrmResult::empty());
        }

        // Phase 2: Single batch fetch of all documents
        self.client.fetch_by_ids(all_ids, spec, self.opts).await
    }
}

/// Convert a JSON value to GraphQL object literal syntax.
///
/// GraphQL object literals don't quote keys, but JSON does.
/// This converts `{"name": {"eq": "test"}}` to `{name: {eq: "test"}}`.
fn json_to_graphql(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => {
            format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
        }
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(json_to_graphql).collect();
            format!("[{}]", items.join(", "))
        }
        serde_json::Value::Object(obj) => {
            let fields: Vec<String> = obj
                .iter()
                .map(|(k, v)| format!("{}: {}", k, json_to_graphql(v)))
                .collect();
            format!("{{{}}}", fields.join(", "))
        }
    }
}

/// Extract IDs from a GraphQL filter query response for a specific type.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_query_fragment_no_args() {
        let query = TypeQuery {
            type_name: "Project".to_string(),
            filter_gql: None,
            limit: None,
            offset: None,
        };

        let fragment = query.build_fragment();
        assert_eq!(fragment, "  Project { _id }");
    }

    #[test]
    fn test_type_query_fragment_with_filter() {
        let query = TypeQuery {
            type_name: "Project".to_string(),
            filter_gql: Some("{name: {eq: \"Test\"}}".to_string()),
            limit: None,
            offset: None,
        };

        let fragment = query.build_fragment();
        assert_eq!(
            fragment,
            "  Project(filter: {name: {eq: \"Test\"}}) { _id }"
        );
    }

    #[test]
    fn test_type_query_fragment_with_all_args() {
        let query = TypeQuery {
            type_name: "Ticket".to_string(),
            filter_gql: Some("{status: {eq: \"open\"}}".to_string()),
            limit: Some(100),
            offset: Some(50),
        };

        let fragment = query.build_fragment();
        assert!(fragment.contains("filter: {status: {eq: \"open\"}}"));
        assert!(fragment.contains("limit: 100"));
        assert!(fragment.contains("offset: 50"));
    }

    #[test]
    fn test_multi_filter_query_build_empty() {
        let query: MultiFilterQuery<GlobalClient> = MultiFilterQuery::new();
        let gql = query.build_query();
        assert_eq!(gql, "query { __typename }");
    }

    #[test]
    fn test_extract_ids_from_response() {
        let response = serde_json::json!({
            "Project": [
                { "_id": "Project/1" },
                { "_id": "Project/2" }
            ],
            "Ticket": [
                { "_id": "Ticket/a" },
                { "_id": "Ticket/b" },
                { "_id": "Ticket/c" }
            ]
        });

        let project_ids = extract_ids_from_response(&response, "Project");
        assert_eq!(project_ids, vec!["Project/1", "Project/2"]);

        let ticket_ids = extract_ids_from_response(&response, "Ticket");
        assert_eq!(ticket_ids, vec!["Ticket/a", "Ticket/b", "Ticket/c"]);
    }

    #[test]
    fn test_extract_ids_missing_type() {
        let response = serde_json::json!({
            "Project": [{ "_id": "Project/1" }]
        });

        let ids = extract_ids_from_response(&response, "NonExistent");
        assert!(ids.is_empty());
    }

    #[test]
    fn test_json_to_graphql_nested() {
        let json = serde_json::json!({
            "company": {
                "_id": {
                    "eq": "Company/acme"
                }
            }
        });

        let gql = json_to_graphql(&json);
        assert!(gql.contains("company:"));
        assert!(gql.contains("_id:"));
        assert!(gql.contains("eq: \"Company/acme\""));
    }
}
