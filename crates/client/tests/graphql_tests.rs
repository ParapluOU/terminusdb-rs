#![recursion_limit = "256"]
//! Integration tests for GraphQL functionality.
//!
//! Previously disabled behind a non-existent `__disabled_graphql_tests` feature
//! (they used an old `create_database` signature and needed an external server).
//! Now migrated onto `TerminusDBServer`: each test spins up the shared embedded
//! v12.1 server and seeds a fresh, uniquely-named database with a small `Widget`
//! schema, so they run in parallel with `cargo test` — no `#[ignore]`, no
//! external instance.

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use serde_json::json;
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::http::GraphQLRequest;
    use terminusdb_schema::{EntityIDFor, ToTDBInstance};
    use terminusdb_schema_derive::*;

    /// A minimal model so the seeded database has a non-trivial GraphQL schema to
    /// introspect and query.
    #[derive(Debug, Clone, TerminusDBModel)]
    #[tdb(id_field = "id")]
    struct Widget {
        id: EntityIDFor<Self>,
        name: String,
    }

    #[tokio::test]
    async fn test_introspection_query() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_db_schema::<(Widget,), _, _, _>("gql_introspect", |client, spec| async move {
                let schema = client.introspect_schema(&spec.db, None, None).await?;

                // Verify we got schema data
                assert!(schema.is_object());

                // Check for expected top-level schema field
                let schema_obj = schema.as_object().expect("introspection result is an object");
                assert!(schema_obj.contains_key("__schema"));

                // Check __schema has expected fields
                let inner_schema = &schema_obj["__schema"];
                assert!(inner_schema.get("queryType").is_some());
                assert!(inner_schema.get("types").is_some());
                assert!(inner_schema.get("directives").is_some());

                println!(
                    "Introspection successful. Found {} types.",
                    inner_schema["types"].as_array().map(|a| a.len()).unwrap_or(0)
                );
                Ok(())
            })
            .await
    }

    #[tokio::test]
    async fn test_simple_graphql_query() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_db_schema::<(Widget,), _, _, _>("gql_simple", |client, spec| async move {
                // NB: a root `{ __typename }` triggers a server-side panic in
                // TerminusDB's GraphQL (`concrete_type_name()`), so we query the
                // schema root instead — still a full request/response round-trip.
                let query = r#"
                    query {
                        __schema {
                            queryType { name }
                        }
                    }
                "#;

                let request = GraphQLRequest::new(query);
                let response = client
                    .execute_graphql::<serde_json::Value>(&spec.db, None, request, None)
                    .await?;

                // Check response structure
                assert!(response.data.is_some());
                assert!(response.errors.is_none() || response.errors.as_ref().unwrap().is_empty());

                println!("Simple query response: {:?}", response.data);
                Ok(())
            })
            .await
    }

    #[tokio::test]
    async fn test_graphql_query_with_variables() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_db_schema::<(Widget,), _, _, _>("gql_vars", |client, spec| async move {
                // Query with variables against a built-in scalar type.
                let query = r#"
                    query GetType($name: String!) {
                        __type(name: $name) {
                            name
                            kind
                            description
                        }
                    }
                "#;

                let variables = json!({ "name": "String" });

                let request = GraphQLRequest::with_variables(query, variables);
                let response = client
                    .execute_graphql::<serde_json::Value>(&spec.db, None, request, None)
                    .await?;

                if let Some(data) = &response.data {
                    println!("Type query response: {}", serde_json::to_string_pretty(data)?);
                }
                if let Some(errors) = &response.errors {
                    for error in errors {
                        eprintln!("GraphQL error: {}", error.message);
                    }
                }
                Ok(())
            })
            .await
    }

    #[tokio::test]
    async fn test_graphql_error_handling() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_db_schema::<(Widget,), _, _, _>("gql_error", |client, spec| async move {
                // Intentionally reference a field that does not exist.
                let query = r#"
                    query {
                        thisFieldDoesNotExist
                    }
                "#;

                let request = GraphQLRequest::new(query);
                let response = client
                    .execute_graphql::<serde_json::Value>(&spec.db, None, request, None)
                    .await?;

                // We expect errors for the unknown field.
                assert!(response.errors.is_some());
                let errors = response.errors.unwrap();
                assert!(!errors.is_empty());

                println!("Expected error received: {}", errors[0].message);
                Ok(())
            })
            .await
    }

    #[tokio::test]
    async fn test_raw_graphql_execution() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_db_schema::<(Widget,), _, _, _>("gql_raw", |client, spec| async move {
                let query = r#"
                    query {
                        __schema {
                            queryType {
                                name
                            }
                        }
                    }
                "#;

                let request = GraphQLRequest::new(query);
                let raw_response = client
                    .execute_graphql_raw(&spec.db, None, request, None)
                    .await?;

                // Verify raw response is valid JSON with the expected structure.
                assert!(raw_response.is_object());
                let obj = raw_response.as_object().expect("raw response is an object");
                assert!(obj.contains_key("data"));

                println!("Raw response: {}", serde_json::to_string_pretty(&raw_response)?);
                Ok(())
            })
            .await
    }

    #[tokio::test]
    async fn test_graphql_with_different_branch() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_db_schema::<(Widget,), _, _, _>("gql_branch", |client, spec| async move {
                // Root `__typename` panics server-side (see test_simple_graphql_query);
                // query the schema root, which works on any branch.
                let query = r#"
                    query {
                        __schema {
                            queryType { name }
                        }
                    }
                "#;

                let request = GraphQLRequest::new(query);

                // Explicit "main" branch should behave like the default.
                let response = client
                    .execute_graphql::<serde_json::Value>(
                        &spec.db,
                        Some("main"),
                        request.clone(),
                        None,
                    )
                    .await?;
                assert!(response.data.is_some());

                // A non-existent branch may error or return empty — either is fine.
                match client
                    .execute_graphql::<serde_json::Value>(
                        &spec.db,
                        Some("non-existent-branch"),
                        request,
                        None,
                    )
                    .await
                {
                    Ok(response) => println!("Response from non-existent branch: {:?}", response),
                    Err(e) => println!("Expected error for non-existent branch: {}", e),
                }
                Ok(())
            })
            .await
    }

    #[tokio::test]
    async fn test_full_introspection_parsing() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_db_schema::<(Widget,), _, _, _>("gql_full_introspect", |client, spec| async move {
                // A full introspection query wrapped in the JSON envelope the
                // GraphQL HTTP API accepts (query + operationName).
                let query = r#"{
        "query": "\n    query IntrospectionQuery {\n      __schema {\n        \n        queryType { name }\n        mutationType { name }\n        subscriptionType { name }\n        types {\n          ...FullType\n        }\n        directives {\n          name\n          description\n          \n          locations\n          args {\n            ...InputValue\n          }\n        }\n      }\n    }\n\n    fragment FullType on __Type {\n      kind\n      name\n      description\n      \n      fields(includeDeprecated: true) {\n        name\n        description\n        args {\n          ...InputValue\n        }\n        type {\n          ...TypeRef\n        }\n        isDeprecated\n        deprecationReason\n      }\n      inputFields {\n        ...InputValue\n      }\n      interfaces {\n        ...TypeRef\n      }\n      enumValues(includeDeprecated: true) {\n        name\n        description\n        isDeprecated\n        deprecationReason\n      }\n      possibleTypes {\n        ...TypeRef\n      }\n    }\n\n    fragment InputValue on __InputValue {\n      name\n      description\n      type { ...TypeRef }\n      defaultValue\n      \n      \n    }\n\n    fragment TypeRef on __Type {\n      kind\n      name\n      ofType {\n        kind\n        name\n        ofType {\n          kind\n          name\n          ofType {\n            kind\n            name\n            ofType {\n              kind\n              name\n              ofType {\n                kind\n                name\n                ofType {\n                  kind\n                  name\n                  ofType {\n                    kind\n                    name\n                  }\n                }\n              }\n            }\n          }\n        }\n      }\n    }\n  ",
        "operationName": "IntrospectionQuery"
    }"#;

                // Parse the JSON envelope to extract the query and operation name.
                let parsed: serde_json::Value = serde_json::from_str(query)?;
                let query_str = parsed["query"].as_str().expect("query field is a string");
                let operation_name = parsed["operationName"].as_str().map(|s| s.to_string());

                let request = GraphQLRequest {
                    query: query_str.to_string(),
                    variables: None,
                    operation_name,
                };

                let response = client
                    .execute_graphql::<serde_json::Value>(&spec.db, None, request, None)
                    .await?;

                assert!(response.data.is_some());
                if let Some(errors) = &response.errors {
                    for error in errors {
                        eprintln!("GraphQL error: {}", error.message);
                    }
                }

                if let Some(data) = &response.data {
                    let pretty = serde_json::to_string_pretty(data)?;
                    println!("Full introspection result preview (first 1000 chars):");
                    println!("{}", &pretty.chars().take(1000).collect::<String>());
                    println!("... (truncated)");
                }
                Ok(())
            })
            .await
    }
}
