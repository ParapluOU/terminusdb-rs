//! Relation resolution for ORM queries.
//!
//! This module handles the execution of relation queries, transforming
//! the declarative `.with::<T>()` calls into actual database queries
//! and combining results.
//!
//! # GraphQL-Based Resolution
//!
//! All relation resolution uses TerminusDB's GraphQL API, which supports:
//!
//! - **Forward relations**: Direct field traversal (e.g., `post { author { _id name } }`)
//! - **Reverse relations**: Path queries with `_path_to_` prefix (e.g., `User { _path_to_Post { ... } }`)
//!
//! # Resolution Strategies
//!
//! 1. **Single Query** (default) - GraphQL join query:
//!    - Generate a single query with nested selections for all relations
//!    - Most efficient for small-to-medium result sets
//!
//! 2. **Batch Loading** - Two-phase approach:
//!    - Phase 1: Load primary entities with their forward relation IDs
//!    - Phase 2: Batch load related entities by collected IDs
//!    - Better for large result sets to avoid GraphQL response size limits
//!
//! 3. **Lazy Loading** - On-demand loading (future)
//!    - Load relations only when accessed

use std::any::TypeId;
use std::collections::HashMap;

use crate::query::{RelationDirection, RelationSpec};
use crate::result::OrmResult;

/// Strategy for loading relations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoadStrategy {
    /// Load relations in batches (default).
    /// Two-phase: fetch primary, then batch fetch related.
    #[default]
    Batch,

    /// Generate a single query with all relations.
    /// Uses GraphQL-style nested selection.
    SingleQuery,

    /// Don't load relations automatically.
    /// Use lazy loading when relations are accessed.
    Lazy,
}

/// Plan for resolving a single relation.
#[derive(Debug, Clone)]
pub struct RelationPlan {
    /// The relation specification from the query.
    pub spec: RelationSpec,

    /// How to resolve this relation.
    pub resolution: RelationResolution,
}

/// How a relation will be resolved.
#[derive(Debug, Clone)]
pub enum RelationResolution {
    /// Forward: Load target entities by IDs extracted from source field.
    ///
    /// Example: Post.author_id -> User
    /// 1. Extract author_id values from loaded Posts
    /// 2. Batch load Users by those IDs
    Forward {
        /// Field on source containing target ID(s)
        source_field: String,
        /// Whether the field is a collection (Vec)
        is_collection: bool,
    },

    /// Reverse: Query target entities that reference source.
    ///
    /// Example: User <- Post.author_id
    /// 1. Get IDs of loaded Users
    /// 2. Query Posts where author_id IN (user_ids)
    Reverse {
        /// Field on target that references source
        target_field: Option<String>,
    },
}

/// Execution plan for a complete query with relations.
#[derive(Debug, Clone)]
pub struct QueryPlan {
    /// IDs of primary entities to fetch.
    pub primary_ids: Vec<String>,

    /// Type name of primary entities.
    pub primary_type: String,

    /// Plans for each relation to load.
    pub relation_plans: Vec<RelationPlan>,

    /// Overall loading strategy.
    pub strategy: LoadStrategy,
}

impl QueryPlan {
    /// Create a new query plan.
    pub fn new(primary_ids: Vec<String>, primary_type: String) -> Self {
        Self {
            primary_ids,
            primary_type,
            relation_plans: Vec::new(),
            strategy: LoadStrategy::default(),
        }
    }

    /// Add a relation to the plan.
    pub fn add_relation(&mut self, spec: RelationSpec) {
        let resolution = match &spec.direction {
            RelationDirection::Forward { field_name } => RelationResolution::Forward {
                source_field: field_name.clone(),
                is_collection: false, // TODO: detect from field type
            },
            RelationDirection::Reverse { via_field } => RelationResolution::Reverse {
                target_field: via_field.clone(),
            },
        };

        self.relation_plans.push(RelationPlan { spec, resolution });
    }

    /// Set the loading strategy.
    pub fn with_strategy(mut self, strategy: LoadStrategy) -> Self {
        self.strategy = strategy;
        self
    }
}

/// Result of relation resolution, with entities organized by type and relation.
#[derive(Debug, Clone)]
pub struct ResolvedRelations {
    /// All loaded entities, combined.
    pub result: OrmResult,

    /// Mapping from (parent_type, child_type) to the field that connects them.
    pub relation_fields: HashMap<(TypeId, TypeId), String>,
}

impl ResolvedRelations {
    /// Create from an OrmResult.
    pub fn new(result: OrmResult) -> Self {
        Self {
            result,
            relation_fields: HashMap::new(),
        }
    }

    /// Get the underlying result.
    pub fn into_result(self) -> OrmResult {
        self.result
    }
}

/// Trait for types that can resolve relations.
///
/// This is implemented by client providers that can execute queries.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait::async_trait]
pub trait RelationResolver: Send + Sync {
    /// Execute a query plan and return resolved relations.
    async fn resolve(
        &self,
        plan: QueryPlan,
        spec: &terminusdb_client::BranchSpec,
    ) -> anyhow::Result<ResolvedRelations>;

    /// Resolve a single forward relation.
    ///
    /// Given parent entity IDs and a field name, load the referenced entities.
    async fn resolve_forward(
        &self,
        parent_ids: Vec<String>,
        field_name: &str,
        target_type: &str,
        spec: &terminusdb_client::BranchSpec,
    ) -> anyhow::Result<OrmResult>;

    /// Resolve a single reverse relation.
    ///
    /// Find entities of target_type where field_name references any of parent_ids.
    async fn resolve_reverse(
        &self,
        parent_ids: Vec<String>,
        field_name: Option<&str>,
        target_type: &str,
        spec: &terminusdb_client::BranchSpec,
    ) -> anyhow::Result<OrmResult>;
}

// ============================================================================
// Batch Resolution Implementation (Stub)
// ============================================================================

/// Batch resolver that executes relations in two phases.
#[derive(Debug, Clone)]
pub struct BatchResolver<C> {
    client: C,
}

impl<C> BatchResolver<C> {
    /// Create a new batch resolver with the given client.
    pub fn new(client: C) -> Self {
        Self { client }
    }
}

// ============================================================================
// GraphQL Query Generation
// ============================================================================

/// Builder for GraphQL relation queries.
///
/// TerminusDB's GraphQL supports two patterns for reverse relations:
///
/// ## 1. Auto-generated reverse fields (preferred for simple cases)
///
/// For every forward relation field `source_type.field_name -> target_type`,
/// TerminusDB auto-generates a reverse field on the target:
///
/// ```text
/// target_type._<field_name>_of_<SourceType>
/// ```
///
/// Example: If `Standard.workflow -> Workflow`, then:
/// ```graphql
/// Workflow {
///   _workflow_of_Standard { _id name }  # No args needed!
/// }
/// ```
///
/// ## 2. Path queries (for complex traversals)
///
/// For multi-hop or complex patterns, use `_path_to_<Type>(path: "...")`:
///
/// ```graphql
/// User {
///   _path_to_Post(path: "<author_id|<reviewer_id") { _id }
/// }
/// ```
///
/// Path syntax:
/// - `<field` - Backward traversal (find nodes linking TO current via field)
/// - `field` - Forward traversal
/// - `A,B` - Sequence
/// - `A|B` - Choice
/// - `field+`, `field*`, `field{n,m}` - Repetition
#[derive(Debug, Clone)]
pub struct GraphQLRelationQuery {
    /// The primary type being queried.
    pub primary_type: String,
    /// Fields to select on the primary type.
    pub primary_fields: Vec<String>,
    /// Nested relation selections.
    pub relation_selections: Vec<RelationSelection>,
}

/// A nested selection for a relation.
#[derive(Debug, Clone)]
pub struct RelationSelection {
    /// The GraphQL field name to use.
    /// - Forward: field name (e.g., "author")
    /// - Reverse via auto-generated: "_<fieldname>_of_<SourceType>" (e.g., "_author_id_of_Post")
    /// - Reverse via path: "_path_to_<Type>" (e.g., "_path_to_Post")
    pub field_name: String,
    /// Fields to select on the related type.
    pub nested_fields: Vec<String>,
    /// Whether this is a reverse relation.
    pub is_reverse: bool,
    /// The path expression for `_path_to_` queries (e.g., "<author_id" for backward traversal).
    /// Only used when `field_name` starts with `_path_to_`.
    pub path_expression: Option<String>,
    /// Nested relation selections under this relation.
    pub children: Vec<RelationSelection>,
}

impl GraphQLRelationQuery {
    /// Create a new query for a primary type.
    pub fn new(primary_type: impl Into<String>) -> Self {
        Self {
            primary_type: primary_type.into(),
            primary_fields: vec!["_id".to_string(), "_type".to_string()],
            relation_selections: Vec::new(),
        }
    }

    /// Add a field to select on the primary type.
    pub fn select(mut self, field: impl Into<String>) -> Self {
        self.primary_fields.push(field.into());
        self
    }

    /// Add a forward relation selection.
    ///
    /// Forward relations traverse a field that contains an ID reference.
    /// Example: `Post.author` where `author` is an `EntityIDFor<User>`.
    pub fn with_forward(mut self, field_name: impl Into<String>, target_type: &str) -> Self {
        self.relation_selections.push(RelationSelection {
            field_name: field_name.into(),
            nested_fields: vec![
                "_id".to_string(),
                "_type".to_string(),
            ],
            is_reverse: false,
            path_expression: None,
            children: Vec::new(),
        });
        // Suppress unused variable warning - target_type used for documentation
        let _ = target_type;
        self
    }

    /// Add a reverse relation selection using the auto-generated field.
    ///
    /// TerminusDB automatically generates reverse lookup fields on target types.
    /// For a field `source_type.field_name -> target_type`, the target gets:
    /// `target_type._<field_name>_of_<SourceType>`
    ///
    /// This method uses that auto-generated field, which requires NO arguments.
    ///
    /// # Arguments
    /// * `source_type` - The type that has the forward relation field
    /// * `via_field` - The field name on source_type that references the target
    ///
    /// # Example
    /// ```ignore
    /// // If Post.author_id -> User, find all Posts referencing this User
    /// GraphQLRelationQuery::new("User")
    ///     .with_reverse_via("Post", "author_id")
    /// ```
    ///
    /// Generates:
    /// ```graphql
    /// User {
    ///   _author_id_of_Post { _id _type }
    /// }
    /// ```
    pub fn with_reverse_via(
        mut self,
        source_type: impl Into<String>,
        via_field: impl Into<String>,
    ) -> Self {
        let source = source_type.into();
        let field = via_field.into();
        // Auto-generated reverse field pattern: _<fieldname>_of_<SourceType>
        self.relation_selections.push(RelationSelection {
            field_name: format!("_{}_of_{}", field, source),
            nested_fields: vec![
                "_id".to_string(),
                "_type".to_string(),
            ],
            is_reverse: true,
            path_expression: None, // No path needed for auto-generated fields
            children: Vec::new(),
        });
        self
    }

    /// Add a reverse relation with a custom path expression.
    ///
    /// Use this for complex path queries that need the full power of
    /// TerminusDB's path regular expression syntax. This uses the
    /// `_path_to_<Type>(path: "...")` syntax which requires a path argument.
    ///
    /// # Path Syntax
    /// - `<field` - Backward traversal (find nodes linking TO current)
    /// - `field` - Forward traversal
    /// - `A,B` - Sequence
    /// - `A|B` - Choice
    /// - `field+` - One or more
    /// - `field*` - Zero or more
    /// - `field{n,m}` - Between n and m times
    ///
    /// # Example
    /// ```ignore
    /// // Find Posts linked via author_id OR reviewer_id
    /// .with_reverse_path("Post", "<author_id|<reviewer_id")
    ///
    /// // Find ancestors through self-referential relation
    /// .with_reverse_path("Person", "(parent)+")
    /// ```
    pub fn with_reverse_path(
        mut self,
        target_type: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        let target = target_type.into();
        self.relation_selections.push(RelationSelection {
            field_name: format!("_path_to_{}", target),
            nested_fields: vec![
                "_id".to_string(),
                "_type".to_string(),
            ],
            is_reverse: true,
            path_expression: Some(path.into()),
            children: Vec::new(),
        });
        self
    }

    /// Build the GraphQL query string.
    pub fn build(&self) -> String {
        let mut query = String::new();
        query.push_str("query {\n");
        query.push_str(&format!("  {} {{\n", self.primary_type));

        // Primary fields
        for field in &self.primary_fields {
            query.push_str(&format!("    {}\n", field));
        }

        // Relation selections
        for selection in &self.relation_selections {
            self.write_selection(&mut query, selection, 4);
        }

        query.push_str("  }\n");
        query.push_str("}\n");
        query
    }

    /// Build the GraphQL query with an ID filter.
    ///
    /// Note: TerminusDB GraphQL uses `id` parameter for single ID lookup.
    /// For multiple IDs, you may need to use a different approach or run
    /// multiple queries.
    pub fn build_with_ids(&self, ids: &[String]) -> String {
        let mut query = String::new();
        query.push_str("query {\n");

        // TerminusDB uses 'id' parameter for filtering, not '_ids'
        // For now, we support single ID lookup
        if ids.len() == 1 {
            query.push_str(&format!(
                "  {}(id: \"{}\") {{\n",
                self.primary_type,
                ids[0]
            ));
        } else {
            // For multiple IDs, we query all and filter client-side
            // (TerminusDB GraphQL doesn't have a built-in _ids filter)
            query.push_str(&format!("  {} {{\n", self.primary_type));
        }

        // Primary fields
        for field in &self.primary_fields {
            query.push_str(&format!("    {}\n", field));
        }

        // Relation selections
        for selection in &self.relation_selections {
            self.write_selection(&mut query, selection, 4);
        }

        query.push_str("  }\n");
        query.push_str("}\n");
        query
    }

    /// Write a single relation selection to the query string.
    fn write_selection(&self, query: &mut String, selection: &RelationSelection, indent: usize) {
        let indent_str = " ".repeat(indent);

        if selection.is_reverse {
            // Reverse relation: _path_to_Type(path: "<field") { ... } or auto-generated field
            if let Some(path) = &selection.path_expression {
                query.push_str(&format!(
                    "{}{}(path: \"{}\") {{\n",
                    indent_str, selection.field_name, path
                ));
            } else {
                // Auto-generated reverse field (e.g., _writer_of_BlogPost)
                query.push_str(&format!("{}{} {{\n", indent_str, selection.field_name));
            }
        } else {
            // Forward relation: just the field name
            query.push_str(&format!("{}{} {{\n", indent_str, selection.field_name));
        }

        // Nested fields (_id, _type, etc.)
        let nested_indent = " ".repeat(indent + 2);
        for nested_field in &selection.nested_fields {
            query.push_str(&format!("{}{}\n", nested_indent, nested_field));
        }

        // Recursively write child relation selections
        for child in &selection.children {
            self.write_selection(query, child, indent + 2);
        }

        query.push_str(&format!("{}}}\n", indent_str));
    }
}

/// Generate a GraphQL query for fetching entities with their relations.
///
/// This creates a single GraphQL query that:
/// - Fetches primary entities by ID
/// - Includes nested selections for all forward relations
/// - Uses `_<fieldname>_of_<SourceType>` for reverse relations (auto-generated by TerminusDB)
///
/// # Panics
/// Panics if a reverse relation doesn't specify a `target_field`. TerminusDB's
/// auto-generated reverse fields require knowing which field to look up.
pub fn generate_graphql_query(
    primary_type: &str,
    primary_ids: &[String],
    relations: &[RelationPlan],
) -> String {
    let mut builder = GraphQLRelationQuery::new(primary_type);

    for plan in relations {
        match &plan.resolution {
            RelationResolution::Forward { source_field, .. } => {
                builder = builder.with_forward(source_field, &plan.spec.target_type_name);
            }
            RelationResolution::Reverse { target_field } => {
                let field = target_field.as_ref().unwrap_or_else(|| {
                    panic!(
                        "Reverse relation to {} requires a target_field. \
                         TerminusDB generates reverse fields as _<fieldname>_of_<SourceType>. \
                         Use .with_via::<{}, SomeField>() instead of .with::<{}>().",
                        plan.spec.target_type_name,
                        plan.spec.target_type_name,
                        plan.spec.target_type_name
                    )
                });
                // Use auto-generated reverse field: _<fieldname>_of_<SourceType>
                // Here, plan.spec.target_type_name is the source type (type with the FK field)
                // and primary_type is the target type being queried
                builder = builder.with_reverse_via(plan.spec.target_type_name.clone(), field);
            }
        }
    }

    builder.build_with_ids(primary_ids)
}

// ============================================================================
// RelationSpec-based GraphQL Generation
// ============================================================================

/// Build a GraphQL query from a tree of `RelationSpec`s.
///
/// This function converts the ORM's `RelationSpec` tree (produced by `.with()` and
/// `.with_nested()` calls) into a GraphQL query that fetches only `_id` fields.
///
/// # Arguments
/// * `primary_type` - The root type being queried (e.g., "Writer")
/// * `primary_ids` - The IDs of the root entities to query
/// * `relations` - The relation specs to include (with nested children)
///
/// # Example
/// ```ignore
/// let query = build_graphql_from_relation_specs(
///     "Writer",
///     &["Writer/123".to_string()],
///     &[
///         RelationSpec {
///             target_type_name: "BlogPost",
///             direction: RelationDirection::Reverse { via_field: Some("writer".to_string()) },
///             children: vec![],
///             ..
///         },
///     ],
/// );
/// ```
///
/// Generates:
/// ```graphql
/// query {
///   Writer(id: "Writer/123") {
///     _id
///     _writer_of_BlogPost { _id }
///   }
/// }
/// ```
pub fn build_graphql_from_relation_specs(
    primary_type: &str,
    primary_ids: &[String],
    relations: &[RelationSpec],
) -> String {
    let mut query = String::new();
    query.push_str("query {\n");

    // Primary type with optional ID filter
    if primary_ids.len() == 1 {
        query.push_str(&format!(
            "  {}(id: \"{}\") {{\n",
            primary_type, primary_ids[0]
        ));
    } else {
        query.push_str(&format!("  {} {{\n", primary_type));
    }

    // Always include _id on primary type
    query.push_str("    _id\n");

    // Write relation selections
    for rel in relations {
        write_relation_spec(&mut query, rel, primary_type, 4);
    }

    query.push_str("  }\n");
    query.push_str("}\n");
    query
}

/// Recursively write a RelationSpec to a GraphQL query string.
pub(crate) fn write_relation_spec(
    query: &mut String,
    rel: &RelationSpec,
    parent_type: &str,
    indent: usize,
) {
    use crate::query::RelationDirection;

    let indent_str = " ".repeat(indent);

    // Determine the GraphQL field name based on direction
    let field_name = match &rel.direction {
        RelationDirection::Forward { field_name } => {
            // Forward relation: use the field name directly
            field_name.clone()
        }
        RelationDirection::Reverse { via_field } => {
            // Reverse relation: use auto-generated field pattern
            // _<fieldname>_of_<SourceType>
            // where SourceType is target_type_name (the type WITH the TdbLazy field)
            // and fieldname is the field on SourceType pointing to parent_type
            let field = via_field.as_ref().map(|f| f.as_str()).unwrap_or_else(|| {
                // Default to lowercase parent type name as the field name
                // e.g., if parent is "Writer", assume field is "writer"
                // This is a common convention for TdbLazy<Writer> fields
                leak_lowercase(parent_type)
            });
            format!("_{}_of_{}", field, rel.target_type_name)
        }
    };

    // Build arguments string for filter, limit, offset, orderBy
    let mut args = Vec::new();
    if let Some(filter) = &rel.filter_gql {
        args.push(format!("filter: {}", filter));
    }
    if let Some(limit) = rel.limit {
        args.push(format!("limit: {}", limit));
    }
    if let Some(offset) = rel.offset {
        args.push(format!("offset: {}", offset));
    }
    if let Some(order_by) = &rel.order_by_gql {
        args.push(format!("orderBy: {}", order_by));
    }

    let args_str = if args.is_empty() {
        String::new()
    } else {
        format!("({})", args.join(", "))
    };

    // Write the field selection with optional arguments
    query.push_str(&format!("{}{}{} {{\n", indent_str, field_name, args_str));

    // Always include _id
    let nested_indent = " ".repeat(indent + 2);
    query.push_str(&format!("{}_id\n", nested_indent));

    // Recursively write child relations
    for child in &rel.children {
        write_relation_spec(query, child, &rel.target_type_name, indent + 2);
    }

    query.push_str(&format!("{}}}\n", indent_str));
}

/// Leak a lowercase version of a string for static lifetime.
/// Used for default field name inference.
fn leak_lowercase(s: &str) -> &'static str {
    // Convert to lowercase and leak to get static lifetime
    // This is acceptable for type names which are limited in number
    let lowercase = s.to_lowercase();
    Box::leak(lowercase.into_boxed_str())
}

/// Recursively extract all `_id` values from a JSON response.
///
/// This walks the entire JSON tree and collects all `_id` string values,
/// deduplicating them in the order they're found.
pub fn extract_ids_recursive(json: &serde_json::Value, ids: &mut Vec<String>) {
    use serde_json::Value;

    match json {
        Value::Object(map) => {
            // Check for _id field
            if let Some(Value::String(id)) = map.get("_id") {
                if !ids.contains(id) {
                    ids.push(id.clone());
                }
            }
            // Recurse into all values
            for value in map.values() {
                extract_ids_recursive(value, ids);
            }
        }
        Value::Array(arr) => {
            for item in arr {
                extract_ids_recursive(item, ids);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_plan_creation() {
        let plan = QueryPlan::new(
            vec!["User/1".to_string(), "User/2".to_string()],
            "User".to_string(),
        );

        assert_eq!(plan.primary_ids.len(), 2);
        assert_eq!(plan.strategy, LoadStrategy::Batch);
        assert!(plan.relation_plans.is_empty());
    }

    #[test]
    fn test_load_strategy_default() {
        assert_eq!(LoadStrategy::default(), LoadStrategy::Batch);
    }

    #[test]
    fn test_graphql_query_builder_basic() {
        let query = GraphQLRelationQuery::new("User")
            .select("name")
            .select("email")
            .build();

        assert!(query.contains("User"));
        assert!(query.contains("_id"));
        assert!(query.contains("name"));
        assert!(query.contains("email"));
    }

    #[test]
    fn test_graphql_query_with_forward_relation() {
        let query = GraphQLRelationQuery::new("Post")
            .select("title")
            .with_forward("author", "User")
            .build();

        assert!(query.contains("Post"));
        assert!(query.contains("author {"));
        assert!(query.contains("_id"));
    }

    #[test]
    fn test_graphql_query_with_reverse_relation() {
        // Reverse relations use auto-generated fields: _<fieldname>_of_<SourceType>
        let query = GraphQLRelationQuery::new("User")
            .with_reverse_via("Post", "author_id")
            .build();

        assert!(query.contains("User"));
        // Auto-generated reverse field pattern (no path arg needed)
        assert!(query.contains("_author_id_of_Post {"));
    }

    #[test]
    fn test_graphql_query_with_single_id() {
        let query = GraphQLRelationQuery::new("User")
            .build_with_ids(&["User/1".to_string()]);

        // Single ID uses the id: parameter
        assert!(query.contains("User(id: \"User/1\")"));
    }

    #[test]
    fn test_graphql_query_with_multiple_ids() {
        let query = GraphQLRelationQuery::new("User")
            .build_with_ids(&["User/1".to_string(), "User/2".to_string()]);

        // Multiple IDs queries all (no filter) - client-side filtering needed
        assert!(query.contains("User {"));
        assert!(!query.contains("id:"));
    }

    #[test]
    fn test_graphql_query_complex() {
        let query = GraphQLRelationQuery::new("User")
            .select("username")
            .with_forward("profile", "Profile")
            .with_reverse_via("Post", "author_id")
            .with_reverse_via("Comment", "commenter_id")
            .build_with_ids(&["User/alice".to_string()]);

        // Primary type with single ID filter
        assert!(query.contains("User(id: \"User/alice\")"));
        // Forward relation
        assert!(query.contains("profile {"));
        // Reverse relations using auto-generated fields
        assert!(query.contains("_author_id_of_Post {"));
        assert!(query.contains("_commenter_id_of_Comment {"));
    }

    #[test]
    fn test_graphql_query_with_custom_path() {
        // Test complex path expressions using _path_to_
        let query = GraphQLRelationQuery::new("User")
            .with_reverse_path("Post", "<author_id|<reviewer_id")
            .build();

        // Should use _path_to_ with path argument for complex patterns
        assert!(query.contains("_path_to_Post(path: \"<author_id|<reviewer_id\")"));
    }

    #[test]
    fn test_generate_graphql_query_from_plans() {
        use crate::query::RelationDirection;
        use std::any::TypeId;

        // We use () as a placeholder type - the actual type doesn't matter for query generation
        let plans = vec![
            RelationPlan {
                spec: RelationSpec {
                    target_type_id: TypeId::of::<()>(),
                    target_type_name: "Post".to_string(),
                    direction: RelationDirection::Reverse {
                        via_field: Some("author_id".to_string()),
                    },
                    children: Vec::new(),
                    filter_gql: None,
                    limit: None,
                    offset: None,
                    order_by_gql: None,
                },
                resolution: RelationResolution::Reverse {
                    target_field: Some("author_id".to_string()),
                },
            },
            RelationPlan {
                spec: RelationSpec {
                    target_type_id: TypeId::of::<()>(),
                    target_type_name: "Profile".to_string(),
                    direction: RelationDirection::Forward {
                        field_name: "profile_id".to_string(),
                    },
                    children: Vec::new(),
                    filter_gql: None,
                    limit: None,
                    offset: None,
                    order_by_gql: None,
                },
                resolution: RelationResolution::Forward {
                    source_field: "profile_id".to_string(),
                    is_collection: false,
                },
            },
        ];

        let query = generate_graphql_query(
            "User",
            &["User/1".to_string()],
            &plans,
        );

        // Single ID uses id: parameter
        assert!(query.contains("User(id: \"User/1\")"));
        // Reverse relation uses auto-generated field: _<fieldname>_of_<SourceType>
        assert!(query.contains("_author_id_of_Post {"));
        // Forward relation uses field name directly
        assert!(query.contains("profile_id {"));
    }

    #[test]
    fn test_build_graphql_from_relation_specs_simple() {
        use crate::query::RelationDirection;
        use std::any::TypeId;

        let relations = vec![RelationSpec {
            target_type_id: TypeId::of::<()>(),
            target_type_name: "BlogPost".to_string(),
            direction: RelationDirection::Reverse {
                via_field: Some("writer".to_string()),
            },
            children: Vec::new(),
            filter_gql: None,
            limit: None,
            offset: None,
            order_by_gql: None,
        }];

        let query = build_graphql_from_relation_specs("Writer", &["Writer/123".to_string()], &relations);

        assert!(query.contains("Writer(id: \"Writer/123\")"));
        assert!(query.contains("_writer_of_BlogPost {"));
        assert!(query.contains("_id"));
    }

    #[test]
    fn test_build_graphql_from_relation_specs_nested() {
        use crate::query::RelationDirection;
        use std::any::TypeId;

        // Writer -> BlogPost -> Comment (nested)
        let relations = vec![
            RelationSpec {
                target_type_id: TypeId::of::<()>(),
                target_type_name: "BlogPost".to_string(),
                direction: RelationDirection::Reverse {
                    via_field: Some("writer".to_string()),
                },
                children: Vec::new(),
                filter_gql: None,
                limit: None,
                offset: None,
                order_by_gql: None,
            },
            RelationSpec {
                target_type_id: TypeId::of::<()>(),
                target_type_name: "Comment".to_string(),
                direction: RelationDirection::Reverse {
                    via_field: Some("writer".to_string()),
                },
                children: vec![RelationSpec {
                    target_type_id: TypeId::of::<()>(),
                    target_type_name: "Reply".to_string(),
                    direction: RelationDirection::Reverse {
                        via_field: Some("comment".to_string()),
                    },
                    children: Vec::new(),
                    filter_gql: None,
                    limit: None,
                    offset: None,
                    order_by_gql: None,
                }],
                filter_gql: None,
                limit: None,
                offset: None,
                order_by_gql: None,
            },
        ];

        let query = build_graphql_from_relation_specs("Writer", &["Writer/123".to_string()], &relations);

        println!("Generated query:\n{}", query);

        // Check structure
        assert!(query.contains("Writer(id: \"Writer/123\")"));
        assert!(query.contains("_writer_of_BlogPost {"));
        assert!(query.contains("_writer_of_Comment {"));
        assert!(query.contains("_comment_of_Reply {"));
    }

    #[test]
    fn test_extract_ids_recursive() {
        use serde_json::json;

        let response = json!({
            "Writer": [{
                "_id": "Writer/1",
                "_writer_of_BlogPost": [
                    { "_id": "BlogPost/1" },
                    { "_id": "BlogPost/2" }
                ],
                "_writer_of_Comment": [{
                    "_id": "Comment/1",
                    "_comment_of_Reply": [
                        { "_id": "Reply/1" },
                        { "_id": "Reply/2" }
                    ]
                }]
            }]
        });

        let mut ids = Vec::new();
        extract_ids_recursive(&response, &mut ids);

        assert_eq!(ids.len(), 6);
        assert!(ids.contains(&"Writer/1".to_string()));
        assert!(ids.contains(&"BlogPost/1".to_string()));
        assert!(ids.contains(&"BlogPost/2".to_string()));
        assert!(ids.contains(&"Comment/1".to_string()));
        assert!(ids.contains(&"Reply/1".to_string()));
        assert!(ids.contains(&"Reply/2".to_string()));
    }

    #[test]
    fn test_build_graphql_from_relation_specs_with_options() {
        use crate::query::RelationDirection;
        use std::any::TypeId;

        // Test that filter, limit, offset, and orderBy are correctly emitted
        let relations = vec![RelationSpec {
            target_type_id: TypeId::of::<()>(),
            target_type_name: "Ticket".to_string(),
            direction: RelationDirection::Reverse {
                via_field: Some("project".to_string()),
            },
            children: Vec::new(),
            filter_gql: Some("{status: {eq: \"open\"}}".to_string()),
            limit: Some(10),
            offset: Some(5),
            order_by_gql: Some("{created_at: Desc}".to_string()),
        }];

        let query =
            build_graphql_from_relation_specs("Project", &["Project/123".to_string()], &relations);

        println!("Generated query with options:\n{}", query);

        // Check that all options are present
        assert!(query.contains("Project(id: \"Project/123\")"));
        assert!(
            query.contains("_project_of_Ticket("),
            "Should have opening paren for args"
        );
        assert!(
            query.contains("filter: {status: {eq: \"open\"}}"),
            "Should have filter arg"
        );
        assert!(query.contains("limit: 10"), "Should have limit arg");
        assert!(query.contains("offset: 5"), "Should have offset arg");
        assert!(
            query.contains("orderBy: {created_at: Desc}"),
            "Should have orderBy arg"
        );
    }

    #[test]
    fn test_build_graphql_from_relation_specs_with_partial_options() {
        use crate::query::RelationDirection;
        use std::any::TypeId;

        // Test with only limit (no filter, offset, or orderBy)
        let relations = vec![RelationSpec {
            target_type_id: TypeId::of::<()>(),
            target_type_name: "Comment".to_string(),
            direction: RelationDirection::Reverse {
                via_field: Some("post".to_string()),
            },
            children: Vec::new(),
            filter_gql: None,
            limit: Some(5),
            offset: None,
            order_by_gql: None,
        }];

        let query =
            build_graphql_from_relation_specs("Post", &["Post/456".to_string()], &relations);

        println!("Generated query with partial options:\n{}", query);

        // Check that only limit is present
        assert!(query.contains("_post_of_Comment(limit: 5)"));
        assert!(!query.contains("filter:"));
        assert!(!query.contains("offset:"));
        assert!(!query.contains("orderBy:"));
    }
}
