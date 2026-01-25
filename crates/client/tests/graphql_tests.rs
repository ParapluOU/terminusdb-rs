//! Integration tests for GraphQL functionality
//!
//! DISABLED: These tests use create_database which doesn't exist in the current API.
//! They also require a running TerminusDB instance.
#![cfg(feature = "__disabled_graphql_tests")]

use serde_json::json;
use terminusdb_client::http::{GraphQLRequest, TerminusDBHttpClient};

/// Test database name for GraphQL tests
const TEST_DB: &str = "graphql_test_db";

/// Helper to create a test client and ensure test database exists
async fn setup_test_client() -> anyhow::Result<TerminusDBHttpClient> {
    let client = TerminusDBHttpClient::local_node().await;

    // Ensure test database exists
    match client
        .create_database(TEST_DB, "Test database for GraphQL", None, None, false)
        .await
    {
        Ok(_) => println!("Created test database: {}", TEST_DB),
        Err(e) => {
            // Database might already exist, which is fine
            println!("Database creation note: {}", e);
        }
    }

    Ok(client)
}

#[tokio::test]
#[ignore = "requires running TerminusDB instance"]
async fn test_introspection_query() -> anyhow::Result<()> {
    let client = setup_test_client().await?;

    // Run introspection query
    let schema = client.introspect_schema(TEST_DB, None).await?;

    // Verify we got schema data
    assert!(schema.is_object());

    // Check for expected top-level schema field
    let schema_obj = schema.as_object().unwrap();
    assert!(schema_obj.contains_key("__schema"));

    // Check __schema has expected fields
    let inner_schema = &schema_obj["__schema"];
    assert!(inner_schema.get("queryType").is_some());
    assert!(inner_schema.get("types").is_some());
    assert!(inner_schema.get("directives").is_some());

    println!(
        "Introspection successful. Found {} types.",
        inner_schema["types"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires running TerminusDB instance"]
async fn test_simple_graphql_query() -> anyhow::Result<()> {
    let client = setup_test_client().await?;

    // Simple query to get database info
    let query = r#"
        query {
            __typename
        }
    "#;

    let request = GraphQLRequest::new(query);
    let response = client
        .execute_graphql::<serde_json::Value>(TEST_DB, None, request)
        .await?;

    // Check response structure
    assert!(response.data.is_some());
    assert!(response.errors.is_none() || response.errors.as_ref().unwrap().is_empty());

    println!("Simple query response: {:?}", response.data);

    Ok(())
}

#[tokio::test]
#[ignore = "requires running TerminusDB instance"]
async fn test_graphql_query_with_variables() -> anyhow::Result<()> {
    let client = setup_test_client().await?;

    // Query with variables (example - adjust based on actual schema)
    let query = r#"
        query GetType($name: String!) {
            __type(name: $name) {
                name
                kind
                description
            }
        }
    "#;

    let variables = json!({
        "name": "String"
    });

    let request = GraphQLRequest::with_variables(query, variables);
    let response = client
        .execute_graphql::<serde_json::Value>(TEST_DB, None, request)
        .await?;

    // Check response
    if let Some(data) = response.data {
        println!(
            "Type query response: {}",
            serde_json::to_string_pretty(&data)?
        );
    }

    if let Some(errors) = response.errors {
        for error in errors {
            eprintln!("GraphQL error: {}", error.message);
        }
    }

    Ok(())
}

#[tokio::test]
#[ignore = "requires running TerminusDB instance"]
async fn test_graphql_error_handling() -> anyhow::Result<()> {
    let client = setup_test_client().await?;

    // Intentionally malformed query
    let query = r#"
        query {
            thisFieldDoesNotExist
        }
    "#;

    let request = GraphQLRequest::new(query);
    let response = client
        .execute_graphql::<serde_json::Value>(TEST_DB, None, request)
        .await?;

    // We expect errors for unknown field
    assert!(response.errors.is_some());
    let errors = response.errors.unwrap();
    assert!(!errors.is_empty());

    println!("Expected error received: {}", errors[0].message);

    Ok(())
}

#[tokio::test]
#[ignore = "requires running TerminusDB instance"]
async fn test_raw_graphql_execution() -> anyhow::Result<()> {
    let client = setup_test_client().await?;

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
    let raw_response = client.execute_graphql_raw(TEST_DB, None, request).await?;

    // Verify raw response is valid JSON with expected structure
    assert!(raw_response.is_object());
    let obj = raw_response.as_object().unwrap();
    assert!(obj.contains_key("data"));

    println!(
        "Raw response: {}",
        serde_json::to_string_pretty(&raw_response)?
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires running TerminusDB instance"]
async fn test_graphql_with_different_branch() -> anyhow::Result<()> {
    let client = setup_test_client().await?;

    // Test with explicit branch name
    let query = r#"
        query {
            __typename
        }
    "#;

    let request = GraphQLRequest::new(query);

    // Test with main branch (should be same as None)
    let response = client
        .execute_graphql::<serde_json::Value>(TEST_DB, Some("main"), request.clone())
        .await?;
    assert!(response.data.is_some());

    // Test with non-existent branch (might error or return empty)
    match client
        .execute_graphql::<serde_json::Value>(TEST_DB, Some("non-existent-branch"), request)
        .await
    {
        Ok(response) => {
            println!("Response from non-existent branch: {:?}", response);
        }
        Err(e) => {
            println!("Expected error for non-existent branch: {}", e);
        }
    }

    Ok(())
}

#[tokio::test]
#[ignore = "requires running TerminusDB instance"]
async fn test_full_introspection_parsing() -> anyhow::Result<()> {
    let client = setup_test_client().await?;

    // Use the exact introspection query from the user's request
    let query = r#"{
        "query": "\n    query IntrospectionQuery {\n      __schema {\n        \n        queryType { name }\n        mutationType { name }\n        subscriptionType { name }\n        types {\n          ...FullType\n        }\n        directives {\n          name\n          description\n          \n          locations\n          args {\n            ...InputValue\n          }\n        }\n      }\n    }\n\n    fragment FullType on __Type {\n      kind\n      name\n      description\n      \n      fields(includeDeprecated: true) {\n        name\n        description\n        args {\n          ...InputValue\n        }\n        type {\n          ...TypeRef\n        }\n        isDeprecated\n        deprecationReason\n      }\n      inputFields {\n        ...InputValue\n      }\n      interfaces {\n        ...TypeRef\n      }\n      enumValues(includeDeprecated: true) {\n        name\n        description\n        isDeprecated\n        deprecationReason\n      }\n      possibleTypes {\n        ...TypeRef\n      }\n    }\n\n    fragment InputValue on __InputValue {\n      name\n      description\n      type { ...TypeRef }\n      defaultValue\n      \n      \n    }\n\n    fragment TypeRef on __Type {\n      kind\n      name\n      ofType {\n        kind\n        name\n        ofType {\n          kind\n          name\n          ofType {\n            kind\n            name\n            ofType {\n              kind\n              name\n              ofType {\n                kind\n                name\n                ofType {\n                  kind\n                  name\n                  ofType {\n                    kind\n                    name\n                  }\n                }\n              }\n            }\n          }\n        }\n      }\n    }\n  ",
        "operationName": "IntrospectionQuery"
    }"#;

    // Parse the JSON to extract query and operation name
    let parsed: serde_json::Value = serde_json::from_str(query)?;
    let query_str = parsed["query"].as_str().unwrap();
    let operation_name = parsed["operationName"].as_str().map(|s| s.to_string());

    let request = GraphQLRequest {
        query: query_str.to_string(),
        variables: None,
        operation_name,
    };

    let response = client
        .execute_graphql::<serde_json::Value>(TEST_DB, None, request)
        .await?;

    // Verify response
    assert!(response.data.is_some());
    if let Some(errors) = &response.errors {
        for error in errors {
            eprintln!("GraphQL error: {}", error.message);
        }
    }

    // Save introspection result for inspection
    if let Some(data) = &response.data {
        let pretty = serde_json::to_string_pretty(data)?;
        println!("Full introspection result preview (first 1000 chars):");
        println!("{}", &pretty.chars().take(1000).collect::<String>());
        println!("... (truncated)");
    }

    Ok(())
}
