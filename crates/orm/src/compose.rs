//! Composable multi-model query builder.
//!
//! This module provides the ability to combine multiple independent `ModelQuery`
//! instances into a single GraphQL request, with full support for nested relations.
//!
//! # Example
//! ```ignore
//! use terminusdb_orm::prelude::*;
//!
//! // Build independent queries with nested relations
//! let project_query = Project::query(ProjectFilter { status: Some(eq("active")), ..Default::default() })
//!     .limit(10)
//!     .order_by(ProjectOrdering { name: Some(Ordering::Asc), ..Default::default() })
//!     .with_opts::<Ticket>(RelationOpts::new().filter(TicketFilter { status: Some(eq("open")), ..Default::default() }).limit(5));
//!
//! let label_query = Label::all();
//!
//! // Compose and execute as a single GraphQL request
//! let result: ComposedResult = Orm::and(project_query, label_query)
//!     .execute(&spec)
//!     .await?;
//!
//! // Access combined results
//! let projects: Vec<Project> = result.get::<Project>()?;
//! let labels: Vec<Label> = result.get::<Label>()?;
//!
//! // Or access per-part results
//! let project_part = result.part(0)?;
//! let label_part = result.part(1)?;
//! ```

use terminusdb_client::{graphql::GraphQLRequest, BranchSpec, GetOpts};
use terminusdb_schema::ToSchemaClass;

use crate::query::{IntoQueryPart, QueryEntry, RelationSpec};
use crate::resolver::write_relation_spec;
use crate::result::OrmResult;
use crate::{ClientProvider, GlobalClient, MultiTypeFetch, OrmModel};

/// Result from a composed multi-model query.
///
/// Supports both combined and per-part result access.
#[derive(Debug)]
pub struct ComposedResult {
    /// Combined result containing all documents from all query parts.
    combined: OrmResult,
    /// Per-part results (indexed by query order).
    parts: Vec<OrmResult>,
}

impl ComposedResult {
    /// Create a new ComposedResult.
    pub(crate) fn new(combined: OrmResult, parts: Vec<OrmResult>) -> Self {
        Self { combined, parts }
    }

    /// Create an empty ComposedResult.
    pub fn empty() -> Self {
        Self {
            combined: OrmResult::empty(),
            parts: Vec::new(),
        }
    }

    /// Get all documents of type T from all query parts combined.
    ///
    /// # Example
    /// ```ignore
    /// let all_projects: Vec<Project> = result.get::<Project>()?;
    /// ```
    pub fn get<T: OrmModel + ToSchemaClass>(&self) -> anyhow::Result<Vec<T>> {
        self.combined.get::<T>()
    }

    /// Access results from a specific query part.
    ///
    /// This is useful when the same model type appears in multiple queries
    /// and you need to distinguish between them.
    ///
    /// # Example
    /// ```ignore
    /// let active_projects = result.part(0)?.get::<Project>()?;
    /// let archived_projects = result.part(1)?.get::<Project>()?;
    /// ```
    pub fn part(&self, index: usize) -> anyhow::Result<&OrmResult> {
        self.parts.get(index).ok_or_else(|| {
            anyhow::anyhow!(
                "Part index {} out of bounds (have {} parts)",
                index,
                self.parts.len()
            )
        })
    }

    /// Number of query parts.
    pub fn num_parts(&self) -> usize {
        self.parts.len()
    }

    /// Get the combined result directly.
    pub fn combined(&self) -> &OrmResult {
        &self.combined
    }
}

/// Namespace for query composition functions.
///
/// # Example
/// ```ignore
/// // Combine two queries
/// let result = Orm::and(project_query, label_query).execute(&spec).await?;
///
/// // Or start a builder and add queries
/// let result = Orm::combine()
///     .add(project_query)
///     .add(label_query)
///     .execute(&spec)
///     .await?;
/// ```
pub struct Orm;

impl Orm {
    /// Combine two queries into a composed query.
    ///
    /// # Example
    /// ```ignore
    /// let result = Orm::and(
    ///     Project::query(filter).with::<Ticket>(),
    ///     Label::all()
    /// ).execute(&spec).await?;
    /// ```
    pub fn and<Q1, Q2>(q1: Q1, q2: Q2) -> ComposedQuery<GlobalClient>
    where
        Q1: IntoQueryPart,
        Q2: IntoQueryPart,
    {
        ComposedQuery::new().add(q1).add(q2)
    }

    /// Start a composed query builder.
    ///
    /// # Example
    /// ```ignore
    /// let result = Orm::combine()
    ///     .add(Project::query(filter))
    ///     .add(Ticket::all())
    ///     .add(Label::all())
    ///     .execute(&spec)
    ///     .await?;
    /// ```
    pub fn combine() -> ComposedQuery<GlobalClient> {
        ComposedQuery::new()
    }
}

/// A composed multi-model query builder.
///
/// Created via `Orm::and()` or `Orm::combine()`.
pub struct ComposedQuery<C = GlobalClient>
where
    C: ClientProvider,
{
    /// The query entries to execute.
    entries: Vec<QueryEntry>,
    /// Get options for document fetching.
    opts: GetOpts,
    /// Client to use.
    client: C,
}

impl ComposedQuery<GlobalClient> {
    /// Create a new empty composed query.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            opts: GetOpts::default(),
            client: GlobalClient,
        }
    }
}

impl Default for ComposedQuery<GlobalClient> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: ClientProvider> ComposedQuery<C> {
    /// Create a new composed query with a specific client.
    pub fn new_with_client(client: C) -> Self {
        Self {
            entries: Vec::new(),
            opts: GetOpts::default(),
            client,
        }
    }

    /// Use a specific client instead of the global one.
    ///
    /// # Example
    /// ```ignore
    /// let result = Orm::and(project_query, label_query)
    ///     .with_client(&*db.client)
    ///     .execute(&spec)
    ///     .await?;
    /// ```
    pub fn with_client<C2: ClientProvider>(self, client: C2) -> ComposedQuery<C2> {
        ComposedQuery {
            entries: self.entries,
            opts: self.opts,
            client,
        }
    }

    /// Add a query to the composition.
    ///
    /// # Example
    /// ```ignore
    /// let query = Orm::combine()
    ///     .add(Project::query(filter))
    ///     .add(Label::all());
    /// ```
    #[allow(clippy::should_implement_trait)]
    pub fn add<Q: IntoQueryPart>(mut self, query: Q) -> Self {
        self.entries.push(query.into_query_entry());
        self
    }

    /// Chainable and() for fluent API.
    ///
    /// # Example
    /// ```ignore
    /// let query = Orm::and(project_query, ticket_query)
    ///     .and(label_query);
    /// ```
    pub fn and<Q: IntoQueryPart>(self, query: Q) -> Self {
        self.add(query)
    }

    /// Enable unfolding of nested documents.
    pub fn unfold(mut self) -> Self {
        self.opts.unfold = true;
        self
    }

    /// Set a timeout for document fetching operations.
    ///
    /// # Example
    /// ```ignore
    /// let result = Orm::combine()
    ///     .add(Project::query(filter))
    ///     .with_timeout(std::time::Duration::from_secs(60))
    ///     .with_client(&client)
    ///     .execute(&spec)
    ///     .await?;
    /// ```
    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.opts.timeout = Some(timeout);
        self
    }

    /// Set the get options for document fetching.
    pub fn opts(mut self, opts: GetOpts) -> Self {
        self.opts = opts;
        self
    }

    /// Get the number of query entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if no queries have been added.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Build the combined GraphQL query string.
    ///
    /// This generates a query with all model types and their nested relations:
    /// ```graphql
    /// query {
    ///   Project(filter: {...}, limit: 10) {
    ///     _id
    ///     _project_of_Ticket(limit: 5) { _id }
    ///   }
    ///   Label { _id }
    /// }
    /// ```
    pub fn build_query(&self) -> String {
        if self.entries.is_empty() {
            return "query { __typename }".to_string();
        }

        let fragments: Vec<String> = self
            .entries
            .iter()
            .map(build_query_entry_fragment)
            .collect();

        format!("query {{\n{}\n}}", fragments.join("\n"))
    }

    /// Execute the composed query and return ComposedResult.
    ///
    /// This performs a two-phase query:
    /// 1. Single GraphQL query with all entries to get matching IDs per type
    /// 2. Batch fetch of all documents by ID, separated by query part
    pub async fn execute(self, spec: &BranchSpec) -> anyhow::Result<ComposedResult>
    where
        C: MultiTypeFetch + Sync,
    {
        if self.entries.is_empty() {
            return Ok(ComposedResult::empty());
        }

        // Phase 1: Execute combined GraphQL query
        let query = self.build_query();

        // Debug: print the generated GraphQL query
        eprintln!("[ComposedQuery DEBUG] Executing GraphQL:\n{}", query);

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

        // Phase 2: Extract IDs per entry and fetch documents
        let mut parts = Vec::with_capacity(self.entries.len());
        let mut all_ids = Vec::new();

        for entry in &self.entries {
            // Extract IDs for this entry (root + nested relations)
            let entry_ids = extract_ids_from_entry(&data, &entry.type_name, &entry.relations);

            all_ids.extend(entry_ids.clone());

            // Fetch documents for this part
            if entry_ids.is_empty() {
                parts.push(OrmResult::empty());
            } else {
                let part_result = self
                    .client
                    .fetch_by_ids(entry_ids, spec, self.opts.clone())
                    .await?;
                parts.push(part_result);
            }
        }

        // Fetch combined (all documents)
        let combined = if all_ids.is_empty() {
            OrmResult::empty()
        } else {
            // Deduplicate IDs (in case same entity appears in multiple parts)
            all_ids.sort();
            all_ids.dedup();

            // Always unfold for composed queries
            let mut fetch_opts = self.opts;
            fetch_opts.unfold = true;

            self.client.fetch_by_ids(all_ids, spec, fetch_opts).await?
        };

        Ok(ComposedResult::new(combined, parts))
    }
}

/// Build a GraphQL fragment for a single query entry.
fn build_query_entry_fragment(entry: &QueryEntry) -> String {
    let mut args = Vec::new();

    if let Some(filter) = &entry.filter_gql {
        args.push(format!("filter: {}", filter));
    }
    if let Some(order_by) = &entry.order_by_gql {
        args.push(format!("orderBy: {}", order_by));
    }
    if let Some(limit) = entry.limit {
        args.push(format!("limit: {}", limit));
    }
    if let Some(offset) = entry.offset {
        args.push(format!("offset: {}", offset));
    }

    let args_str = if args.is_empty() {
        String::new()
    } else {
        format!("({})", args.join(", "))
    };

    let mut fragment = format!("  {}{} {{\n    _id\n", entry.type_name, args_str);

    // Add nested relations
    for rel in &entry.relations {
        write_relation_spec(&mut fragment, rel, &entry.type_name, 4);
    }

    fragment.push_str("  }");
    fragment
}

/// Extract IDs from a GraphQL response for a specific entry.
fn extract_ids_from_entry(
    data: &serde_json::Value,
    type_name: &str,
    relations: &[RelationSpec],
) -> Vec<String> {
    let mut ids = Vec::new();

    if let Some(array) = data.get(type_name).and_then(|v| v.as_array()) {
        for item in array {
            // Extract root ID
            if let Some(id) = item.get("_id").and_then(|v| v.as_str()) {
                ids.push(id.to_string());
            }

            // Extract nested relation IDs recursively
            extract_relation_ids_recursive(item, relations, &mut ids);
        }
    }

    ids
}

/// Recursively extract IDs from nested relations in a response item.
fn extract_relation_ids_recursive(
    item: &serde_json::Value,
    relations: &[RelationSpec],
    ids: &mut Vec<String>,
) {
    for rel in relations {
        // Determine the field name in the response
        let field_name = match &rel.direction {
            crate::query::RelationDirection::Forward { field_name } => field_name.clone(),
            crate::query::RelationDirection::Reverse { via_field } => {
                if let Some(field) = via_field {
                    format!("_{}_of_{}", field, rel.target_type_name)
                } else {
                    // Fallback: try common patterns
                    // This is a simplified heuristic - in practice we'd need the parent type name
                    format!("_of_{}", rel.target_type_name)
                }
            }
        };

        // Try to find the field in the item
        if let Some(rel_data) = item.get(&field_name) {
            if let Some(rel_array) = rel_data.as_array() {
                for rel_item in rel_array {
                    if let Some(id) = rel_item.get("_id").and_then(|v| v.as_str()) {
                        ids.push(id.to_string());
                    }

                    // Recurse for nested relations
                    extract_relation_ids_recursive(rel_item, &rel.children, ids);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_query_entry_fragment_no_args() {
        let entry = QueryEntry {
            type_name: "Project".to_string(),
            filter_gql: None,
            limit: None,
            offset: None,
            order_by_gql: None,
            relations: Vec::new(),
        };

        let fragment = build_query_entry_fragment(&entry);
        assert_eq!(fragment, "  Project {\n    _id\n  }");
    }

    #[test]
    fn test_build_query_entry_fragment_with_filter() {
        let entry = QueryEntry {
            type_name: "Project".to_string(),
            filter_gql: Some("{status: {eq: \"active\"}}".to_string()),
            limit: None,
            offset: None,
            order_by_gql: None,
            relations: Vec::new(),
        };

        let fragment = build_query_entry_fragment(&entry);
        assert!(fragment.contains("filter: {status: {eq: \"active\"}}"));
        assert!(fragment.contains("Project("));
    }

    #[test]
    fn test_build_query_entry_fragment_with_all_args() {
        let entry = QueryEntry {
            type_name: "Ticket".to_string(),
            filter_gql: Some("{status: {eq: \"open\"}}".to_string()),
            limit: Some(100),
            offset: Some(50),
            order_by_gql: Some("{created_at: Desc}".to_string()),
            relations: Vec::new(),
        };

        let fragment = build_query_entry_fragment(&entry);
        assert!(fragment.contains("filter: {status: {eq: \"open\"}}"));
        assert!(fragment.contains("orderBy: {created_at: Desc}"));
        assert!(fragment.contains("limit: 100"));
        assert!(fragment.contains("offset: 50"));
    }

    #[test]
    fn test_build_query_entry_fragment_with_nested_relations() {
        use crate::query::RelationDirection;
        use std::any::TypeId;

        let entry = QueryEntry {
            type_name: "Project".to_string(),
            filter_gql: Some("{status: {eq: \"active\"}}".to_string()),
            limit: Some(10),
            offset: None,
            order_by_gql: Some("{name: Asc}".to_string()),
            relations: vec![RelationSpec {
                target_type_id: TypeId::of::<()>(), // Placeholder
                target_type_name: "Ticket".to_string(),
                direction: RelationDirection::Reverse {
                    via_field: Some("project".to_string()),
                },
                children: Vec::new(),
                filter_gql: Some("{status: {eq: \"open\"}}".to_string()),
                limit: Some(5),
                offset: None,
                order_by_gql: Some("{created_at: Desc}".to_string()),
            }],
        };

        let fragment = build_query_entry_fragment(&entry);

        // Should have root Project args
        assert!(fragment.contains("Project(filter: {status: {eq: \"active\"}}"));
        assert!(fragment.contains("orderBy: {name: Asc}"));
        assert!(fragment.contains("limit: 10"));

        // Should have nested Ticket relation
        assert!(fragment.contains("_project_of_Ticket"));
        assert!(fragment.contains("filter: {status: {eq: \"open\"}}"));
        assert!(fragment.contains("limit: 5"));
        assert!(fragment.contains("orderBy: {created_at: Desc}"));
    }

    #[test]
    fn test_composed_query_build_empty() {
        let query: ComposedQuery<GlobalClient> = ComposedQuery::new();
        let gql = query.build_query();
        assert_eq!(gql, "query { __typename }");
    }

    #[test]
    fn test_composed_query_build_multiple_entries() {
        let mut query: ComposedQuery<GlobalClient> = ComposedQuery::new();

        // Manually add entries (normally done via add())
        query.entries.push(QueryEntry {
            type_name: "Project".to_string(),
            filter_gql: Some("{status: {eq: \"active\"}}".to_string()),
            limit: Some(10),
            offset: None,
            order_by_gql: None,
            relations: Vec::new(),
        });

        query.entries.push(QueryEntry {
            type_name: "Label".to_string(),
            filter_gql: None,
            limit: None,
            offset: None,
            order_by_gql: None,
            relations: Vec::new(),
        });

        let gql = query.build_query();

        // Should have both entries
        assert!(gql.contains("Project(filter: {status: {eq: \"active\"}}, limit: 10)"));
        assert!(gql.contains("Label {"));
        assert!(gql.starts_with("query {"));
    }

    #[test]
    fn test_extract_ids_from_entry_simple() {
        let response = serde_json::json!({
            "Project": [
                { "_id": "Project/1" },
                { "_id": "Project/2" },
                { "_id": "Project/3" }
            ]
        });

        let ids = extract_ids_from_entry(&response, "Project", &[]);
        assert_eq!(ids, vec!["Project/1", "Project/2", "Project/3"]);
    }

    #[test]
    fn test_extract_ids_from_entry_missing_type() {
        let response = serde_json::json!({
            "Project": [{ "_id": "Project/1" }]
        });

        let ids = extract_ids_from_entry(&response, "NonExistent", &[]);
        assert!(ids.is_empty());
    }

    #[test]
    fn test_composed_result_empty() {
        let result = ComposedResult::empty();
        assert_eq!(result.num_parts(), 0);
    }

    // Integration-style test showing expected GraphQL output
    #[test]
    fn test_full_composed_query_graphql_output() {
        use crate::query::RelationDirection;
        use std::any::TypeId;

        let mut query: ComposedQuery<GlobalClient> = ComposedQuery::new();

        // Project with nested Ticket relation
        query.entries.push(QueryEntry {
            type_name: "Project".to_string(),
            filter_gql: Some("{status: {eq: \"active\"}}".to_string()),
            limit: Some(10),
            offset: None,
            order_by_gql: Some("{name: Asc}".to_string()),
            relations: vec![RelationSpec {
                target_type_id: TypeId::of::<()>(),
                target_type_name: "Ticket".to_string(),
                direction: RelationDirection::Reverse {
                    via_field: Some("project".to_string()),
                },
                children: Vec::new(),
                filter_gql: Some("{status: {eq: \"open\"}}".to_string()),
                limit: Some(5),
                offset: None,
                order_by_gql: Some("{created_at: Desc}".to_string()),
            }],
        });

        // Ticket with no filter
        query.entries.push(QueryEntry {
            type_name: "Ticket".to_string(),
            filter_gql: Some("{status: {eq: \"open\"}}".to_string()),
            limit: Some(100),
            offset: None,
            order_by_gql: None,
            relations: Vec::new(),
        });

        // Label with no args
        query.entries.push(QueryEntry {
            type_name: "Label".to_string(),
            filter_gql: None,
            limit: None,
            offset: None,
            order_by_gql: None,
            relations: Vec::new(),
        });

        let gql = query.build_query();

        // Expected output structure:
        // query {
        //   Project(filter: {status: {eq: "active"}}, orderBy: {name: Asc}, limit: 10) {
        //     _id
        //     _project_of_Ticket(filter: {status: {eq: "open"}}, orderBy: {created_at: Desc}, limit: 5) {
        //       _id
        //     }
        //   }
        //   Ticket(filter: {status: {eq: "open"}}, limit: 100) {
        //     _id
        //   }
        //   Label {
        //     _id
        //   }
        // }

        println!("Generated GraphQL:\n{}", gql);

        // Verify structure
        assert!(gql.starts_with("query {"));
        assert!(gql.ends_with("}"));

        // Verify Project entry
        assert!(gql.contains("Project(filter: {status: {eq: \"active\"}}"));
        assert!(gql.contains("orderBy: {name: Asc}"));
        assert!(gql.contains("limit: 10"));

        // Verify nested Ticket relation in Project
        assert!(gql.contains("_project_of_Ticket("));
        assert!(gql.contains("filter: {status: {eq: \"open\"}}"));
        assert!(gql.contains("orderBy: {created_at: Desc}"));
        assert!(gql.contains("limit: 5"));

        // Verify standalone Ticket entry
        assert!(gql.contains("Ticket(filter: {status: {eq: \"open\"}}, limit: 100)"));

        // Verify Label entry (no args)
        assert!(gql.contains("Label {"));
    }
}
