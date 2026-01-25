//! GraphQL query builder for ID-only relation traversal.
//!
//! Builds GraphQL queries that efficiently fetch only the IDs of related entities,
//! which can then be batch-loaded via `get_documents`.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use terminusdb_client::{graphql::GraphQLRequest, BranchSpec, TerminusDBHttpClient};
use terminusdb_schema::ToSchemaClass;

/// Builds a GraphQL query for fetching related entity IDs.
///
/// # Example
/// ```ignore
/// let query = IdQueryBuilder::new("Comment")
///     .filter_by_ids(&["Comment/1", "Comment/2"])
///     .with_relation("replies")
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct IdQueryBuilder {
    /// The root type to query
    root_type: String,
    /// IDs to filter by (optional)
    filter_ids: Vec<String>,
    /// Relation fields to traverse for nested IDs
    relations: Vec<RelationPath>,
}

/// A path to a related entity for ID extraction.
#[derive(Debug, Clone)]
pub struct RelationPath {
    /// The field name on the parent type
    pub field_name: String,
    /// Nested relations (for deep traversal)
    pub nested: Vec<RelationPath>,
}

impl RelationPath {
    /// Create a new relation path.
    pub fn new(field_name: impl Into<String>) -> Self {
        Self {
            field_name: field_name.into(),
            nested: Vec::new(),
        }
    }

    /// Add a nested relation to traverse.
    pub fn with_nested(mut self, relation: RelationPath) -> Self {
        self.nested.push(relation);
        self
    }

    /// Build the GraphQL selection for this path.
    pub fn build_selection(&self) -> String {
        if self.nested.is_empty() {
            format!("{} {{ _id }}", self.field_name)
        } else {
            let nested_selections: Vec<String> =
                self.nested.iter().map(|r| r.build_selection()).collect();
            format!(
                "{} {{ _id {} }}",
                self.field_name,
                nested_selections.join(" ")
            )
        }
    }
}

impl IdQueryBuilder {
    /// Create a new ID query builder for the given root type.
    pub fn new(root_type: impl Into<String>) -> Self {
        Self {
            root_type: root_type.into(),
            filter_ids: Vec::new(),
            relations: Vec::new(),
        }
    }

    /// Create a new ID query builder using the class name from a type.
    pub fn for_type<T: ToSchemaClass>() -> Self {
        Self::new(T::to_class())
    }

    /// Filter by specific IDs.
    pub fn filter_by_ids(mut self, ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.filter_ids = ids.into_iter().map(|id| id.into()).collect();
        self
    }

    /// Add a relation to traverse for nested IDs.
    pub fn with_relation(mut self, field_name: impl Into<String>) -> Self {
        self.relations.push(RelationPath::new(field_name));
        self
    }

    /// Add a complex relation path.
    pub fn with_relation_path(mut self, path: RelationPath) -> Self {
        self.relations.push(path);
        self
    }

    /// Build the GraphQL query string.
    pub fn build(&self) -> String {
        let mut query = String::new();
        query.push_str("query {\n");

        // Build the root query with optional ID filter
        if self.filter_ids.is_empty() {
            query.push_str(&format!("  {} {{\n", self.root_type));
        } else {
            let ids_json = serde_json::to_string(&self.filter_ids).unwrap_or_default();
            query.push_str(&format!("  {}(ids: {}) {{\n", self.root_type, ids_json));
        }

        // Always include the root ID
        query.push_str("    _id\n");

        // Add relation selections
        for relation in &self.relations {
            query.push_str("    ");
            query.push_str(&relation.build_selection());
            query.push('\n');
        }

        query.push_str("  }\n");
        query.push_str("}\n");

        query
    }

    /// Build a GraphQL request from this query.
    pub fn build_request(&self) -> GraphQLRequest {
        GraphQLRequest::new(self.build())
    }
}

/// Result of executing an ID query - contains all collected IDs organized by type.
#[derive(Debug, Clone, Default)]
pub struct IdQueryResult {
    /// Root entity IDs
    pub root_ids: Vec<String>,
    /// Related entity IDs, keyed by relation field name
    pub related_ids: HashMap<String, Vec<String>>,
}

impl IdQueryResult {
    /// Get all unique IDs across all types.
    pub fn all_ids(&self) -> Vec<String> {
        let mut ids: HashSet<String> = self.root_ids.iter().cloned().collect();
        for related in self.related_ids.values() {
            ids.extend(related.iter().cloned());
        }
        ids.into_iter().collect()
    }

    /// Check if there are any IDs.
    pub fn is_empty(&self) -> bool {
        self.root_ids.is_empty() && self.related_ids.values().all(|v| v.is_empty())
    }

    /// Get the total count of all IDs.
    pub fn total_count(&self) -> usize {
        self.root_ids.len() + self.related_ids.values().map(|v| v.len()).sum::<usize>()
    }
}

/// Parse a GraphQL response to extract IDs.
///
/// The response is expected to have the structure:
/// ```json
/// {
///   "TypeName": [
///     { "_id": "Type/1", "relation": [{ "_id": "Related/1" }] }
///   ]
/// }
/// ```
pub fn parse_id_response(
    response: &serde_json::Value,
    root_type: &str,
    relation_fields: &[&str],
) -> IdQueryResult {
    let mut result = IdQueryResult::default();

    // Initialize related_ids with empty vectors for each field
    for field in relation_fields {
        result.related_ids.insert(field.to_string(), Vec::new());
    }

    // Get the root array
    let Some(root_array) = response.get(root_type).and_then(|v| v.as_array()) else {
        return result;
    };

    // Extract IDs from each root object
    for obj in root_array {
        // Extract root ID
        if let Some(id) = obj.get("_id").and_then(|v| v.as_str()) {
            result.root_ids.push(id.to_string());
        }

        // Extract related IDs
        for field in relation_fields {
            if let Some(related) = obj.get(*field) {
                extract_ids_from_value(related, result.related_ids.get_mut(*field).unwrap());
            }
        }
    }

    result
}

/// Recursively extract _id values from a JSON value.
fn extract_ids_from_value(value: &serde_json::Value, ids: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(obj) => {
            if let Some(id) = obj.get("_id").and_then(|v| v.as_str()) {
                ids.push(id.to_string());
            }
            // Also check for nested objects
            for (_, v) in obj.iter() {
                extract_ids_from_value(v, ids);
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                extract_ids_from_value(item, ids);
            }
        }
        _ => {}
    }
}

/// Execute an ID query and parse the results.
#[cfg(not(target_arch = "wasm32"))]
pub async fn execute_id_query(
    client: &TerminusDBHttpClient,
    spec: &BranchSpec,
    builder: &IdQueryBuilder,
    timeout: Option<Duration>,
) -> anyhow::Result<IdQueryResult> {
    let request = builder.build_request();
    let relation_fields: Vec<&str> = builder
        .relations
        .iter()
        .map(|r| r.field_name.as_str())
        .collect();

    let branch = spec.branch.as_deref().unwrap_or("main");
    let response = client
        .execute_graphql::<serde_json::Value>(&spec.db, Some(branch), request, timeout)
        .await?;

    // Check for errors
    if let Some(errors) = response.errors {
        if !errors.is_empty() {
            return Err(anyhow::anyhow!(
                "GraphQL errors: {:?}",
                errors.iter().map(|e| &e.message).collect::<Vec<_>>()
            ));
        }
    }

    let data = response.data.unwrap_or(serde_json::Value::Null);
    Ok(parse_id_response(
        &data,
        &builder.root_type,
        &relation_fields,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_query() {
        let query = IdQueryBuilder::new("Comment")
            .filter_by_ids(["Comment/1", "Comment/2"])
            .build();

        assert!(query.contains("Comment(ids:"));
        assert!(query.contains("_id"));
    }

    #[test]
    fn test_query_with_relation() {
        let query = IdQueryBuilder::new("Comment")
            .filter_by_ids(["Comment/1"])
            .with_relation("replies")
            .build();

        assert!(query.contains("Comment(ids:"));
        assert!(query.contains("replies { _id }"));
    }

    #[test]
    fn test_query_no_filter() {
        let query = IdQueryBuilder::new("Comment").build();

        assert!(query.contains("Comment {"));
        assert!(!query.contains("ids:"));
    }

    #[test]
    fn test_parse_response() {
        let response = serde_json::json!({
            "Comment": [
                {
                    "_id": "Comment/1",
                    "replies": [
                        { "_id": "Reply/1" },
                        { "_id": "Reply/2" }
                    ]
                },
                {
                    "_id": "Comment/2",
                    "replies": [
                        { "_id": "Reply/3" }
                    ]
                }
            ]
        });

        let result = parse_id_response(&response, "Comment", &["replies"]);

        assert_eq!(result.root_ids, vec!["Comment/1", "Comment/2"]);
        assert_eq!(
            result.related_ids.get("replies").unwrap(),
            &vec!["Reply/1", "Reply/2", "Reply/3"]
        );
    }

    #[test]
    fn test_all_ids() {
        let mut result = IdQueryResult::default();
        result.root_ids = vec!["A/1".to_string(), "A/2".to_string()];
        result
            .related_ids
            .insert("rel".to_string(), vec!["B/1".to_string()]);

        let all = result.all_ids();
        assert_eq!(all.len(), 3);
        assert!(all.contains(&"A/1".to_string()));
        assert!(all.contains(&"A/2".to_string()));
        assert!(all.contains(&"B/1".to_string()));
    }
}
