//! GraphQL operations for TerminusDB
//!
//! This module provides support for executing GraphQL queries against TerminusDB's GraphQL endpoint.
//! It includes utilities for introspection, query execution, and response handling.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::{TerminusDBAdapterError, TerminusDBHttpClient};
use std::time::Instant;

/// A GraphQL request following the standard GraphQL over HTTP specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLRequest {
    /// The GraphQL query string
    pub query: String,
    /// Optional variables for the query
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<Value>,
    /// Optional operation name when query contains multiple operations
    #[serde(rename = "operationName", skip_serializing_if = "Option::is_none")]
    pub operation_name: Option<String>,
}

impl GraphQLRequest {
    /// Create a new GraphQL request with just a query
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            variables: None,
            operation_name: None,
        }
    }

    /// Create a new GraphQL request with query and variables
    pub fn with_variables(query: impl Into<String>, variables: Value) -> Self {
        Self {
            query: query.into(),
            variables: Some(variables),
            operation_name: None,
        }
    }
}

/// A GraphQL response following the standard GraphQL specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLResponse<T> {
    /// The data returned by the query (can be null on errors)
    pub data: Option<T>,
    /// Any errors that occurred during query execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<GraphQLError>>,
    /// Optional extensions (server-specific)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<HashMap<String, Value>>,
}

/// A GraphQL error following the standard specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLError {
    /// The error message
    pub message: String,
    /// Locations in the query where the error occurred
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locations: Option<Vec<GraphQLLocation>>,
    /// Path to the field that caused the error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<Vec<GraphQLPathSegment>>,
    /// Additional error information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<HashMap<String, Value>>,
}

/// Location in a GraphQL query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLLocation {
    /// Line number (1-indexed)
    pub line: u32,
    /// Column number (1-indexed)
    pub column: u32,
}

/// A segment in a GraphQL error path (can be a field name or array index)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GraphQLPathSegment {
    /// Field name
    Field(String),
    /// Array index
    Index(usize),
}

/// The standard introspection query for GraphQL schemas
pub const INTROSPECTION_QUERY: &str = r#"
    query IntrospectionQuery {
      __schema {
        queryType { name }
        mutationType { name }
        subscriptionType { name }
        types {
          ...FullType
        }
        directives {
          name
          description
          locations
          args {
            ...InputValue
          }
        }
      }
    }

    fragment FullType on __Type {
      kind
      name
      description
      fields(includeDeprecated: true) {
        name
        description
        args {
          ...InputValue
        }
        type {
          ...TypeRef
        }
        isDeprecated
        deprecationReason
      }
      inputFields {
        ...InputValue
      }
      interfaces {
        ...TypeRef
      }
      enumValues(includeDeprecated: true) {
        name
        description
        isDeprecated
        deprecationReason
      }
      possibleTypes {
        ...TypeRef
      }
    }

    fragment InputValue on __InputValue {
      name
      description
      type { ...TypeRef }
      defaultValue
    }

    fragment TypeRef on __Type {
      kind
      name
      ofType {
        kind
        name
        ofType {
          kind
          name
          ofType {
            kind
            name
            ofType {
              kind
              name
              ofType {
                kind
                name
                ofType {
                  kind
                  name
                  ofType {
                    kind
                    name
                  }
                }
              }
            }
          }
        }
      }
    }
  "#;

#[cfg(not(target_arch = "wasm32"))]
impl TerminusDBHttpClient {
    /// Execute a GraphQL query against the specified database and branch
    ///
    /// # Arguments
    /// * `database` - The database name
    /// * `branch` - The branch name (defaults to "main" if None)
    /// * `request` - The GraphQL request to execute
    ///
    /// # Returns
    /// A GraphQL response with the specified type T for the data field
    pub async fn execute_graphql<T: serde::de::DeserializeOwned>(
        &self,
        database: &str,
        branch: Option<&str>,
        request: GraphQLRequest,
    ) -> Result<GraphQLResponse<T>, TerminusDBAdapterError> {
        use crate::debug::{OperationEntry, OperationType, QueryLogEntry};

        let start_time = Instant::now();
        let branch = branch.unwrap_or("main");
        let url = self.build_graphql_url(database, branch);

        // Create operation entry with the GraphQL query stored in context
        let mut operation = OperationEntry::new(
            OperationType::GraphQL,
            format!("/graphql/{}/{}/local/branch/{}", self.org, database, branch)
        )
        .with_context(
            Some(database.to_string()),
            Some(branch.to_string())
        )
        .with_additional_context(request.query.clone());

        let response = self
            .http
            .post(url)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| TerminusDBAdapterError::HTTP(e))?;

        let duration_ms = start_time.elapsed().as_millis() as u64;

        let result = if response.status().is_success() {
            response
                .json::<GraphQLResponse<T>>()
                .await
                .map_err(|e| TerminusDBAdapterError::HTTP(e))
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to get error text".to_string());
            Err(TerminusDBAdapterError::Other(format!(
                "GraphQL server error: {}",
                error_text
            )))
        };

        // Update operation entry based on result
        match &result {
            Ok(res) => {
                // Try to count results if data is present
                let result_count = res.data.as_ref().map(|_| 1);
                operation = operation.success(result_count, duration_ms);

                // Log to query log if enabled
                let logger_opt = self.query_logger.read().ok().and_then(|guard| guard.clone());
                if let Some(logger) = logger_opt {
                    let log_entry = QueryLogEntry {
                        timestamp: chrono::Utc::now(),
                        operation_type: "graphql".to_string(),
                        database: Some(database.to_string()),
                        branch: Some(branch.to_string()),
                        endpoint: operation.endpoint.clone(),
                        details: serde_json::json!({
                            "query": request.query,
                            "variables": request.variables,
                            "operation_name": request.operation_name
                        }),
                        success: true,
                        result_count,
                        duration_ms,
                        error: None,
                    };
                    let _ = logger.log(log_entry).await;
                }
            }
            Err(e) => {
                operation = operation.failure(e.to_string(), duration_ms);

                // Log to query log if enabled
                let logger_opt = self.query_logger.read().ok().and_then(|guard| guard.clone());
                if let Some(logger) = logger_opt {
                    let log_entry = QueryLogEntry {
                        timestamp: chrono::Utc::now(),
                        operation_type: "graphql".to_string(),
                        database: Some(database.to_string()),
                        branch: Some(branch.to_string()),
                        endpoint: operation.endpoint.clone(),
                        details: serde_json::json!({
                            "query": request.query,
                            "variables": request.variables,
                            "operation_name": request.operation_name
                        }),
                        success: false,
                        result_count: None,
                        duration_ms,
                        error: Some(e.to_string()),
                    };
                    let _ = logger.log(log_entry).await;
                }
            }
        }

        // Add to operation log
        self.operation_log.push(operation);

        result
    }

    /// Execute a GraphQL query and return the raw JSON response
    ///
    /// This is useful when you don't want to deserialize the response
    /// or need maximum flexibility in handling the response.
    ///
    /// Note: This method automatically tracks query duration and logs to the operation log
    /// via the underlying `execute_graphql` call.
    pub async fn execute_graphql_raw(
        &self,
        database: &str,
        branch: Option<&str>,
        request: GraphQLRequest,
    ) -> Result<Value, TerminusDBAdapterError> {
        self.execute_graphql::<Value>(database, branch, request)
            .await
            .map(|response| {
                serde_json::json!({
                    "data": response.data,
                    "errors": response.errors,
                    "extensions": response.extensions,
                })
            })
    }

    /// Get the GraphQL schema for a database using introspection
    ///
    /// # Arguments
    /// * `database` - The database name
    /// * `branch` - The branch name (defaults to "main" if None)
    ///
    /// # Returns
    /// The introspection query result containing the full schema
    #[pseudonym::alias(get_graphql_schema)]
    pub async fn introspect_schema(
        &self,
        database: &str,
        branch: Option<&str>,
    ) -> Result<Value, TerminusDBAdapterError> {
        let request = GraphQLRequest {
            query: INTROSPECTION_QUERY.to_string(),
            variables: None,
            operation_name: Some("IntrospectionQuery".to_string()),
        };

        let response = self
            .execute_graphql::<Value>(database, branch, request)
            .await?;

        if let Some(errors) = response.errors {
            if !errors.is_empty() {
                return Err(TerminusDBAdapterError::Other(format!(
                    "GraphQL errors: {:?}",
                    errors
                )));
            }
        }

        response.data.ok_or_else(|| {
            TerminusDBAdapterError::Other("No data in introspection response".to_string())
        })
    }

    /// Build the GraphQL endpoint URL for a database
    fn build_graphql_url(&self, database: &str, branch: &str) -> String {
        format!(
            "{}/graphql/{}/{}/local/branch/{}",
            self.endpoint.as_str().trim_end_matches('/'),
            self.org,
            database,
            branch
        )
    }
}
