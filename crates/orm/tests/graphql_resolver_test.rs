//! Integration tests for GraphQL queries against TerminusDB.
//!
//! These tests use the embedded in-memory TerminusDB server.
//!
//! # Current Limitations
//!
//! **Important**: `EntityIDFor<T>` creates STRING properties in the schema,
//! not document links. This means:
//!
//! 1. Forward relations cannot traverse into the referenced entity
//!    (you can only read the ID as a string, not nested fields)
//! 2. Auto-generated reverse fields like `_<field>_of_<Type>` are NOT created
//!    by TerminusDB because there's no actual link in the schema
//! 3. Path queries (`_path_to_`) don't work for traversing string references
//!
//! To enable full relation support, fields would need to be defined as
//! linked documents (embedded or via `TdbLazy<T>`), not `EntityIDFor<T>`.

use serde::{Deserialize, Serialize};
use terminusdb_client::graphql::GraphQLRequest;
use terminusdb_client::DocumentInsertArgs;
use terminusdb_schema::{EntityIDFor, ToTDBInstance};
use terminusdb_schema_derive::TerminusDBModel;

// Required for the derive macro
use terminusdb_schema as terminusdb_schema;

use terminusdb_orm::prelude::GraphQLRelationQuery;
use terminusdb_test::test as db_test;

// ============================================================================
// Test Models
// ============================================================================

/// A user in the system
#[derive(Clone, Debug, Default, TerminusDBModel)]
pub struct Author {
    pub name: String,
    pub email: String,
}

/// A blog post that belongs to an author
/// Note: `author_id` is stored as a STRING in the schema, not as a document link!
#[derive(Clone, Debug, Default, TerminusDBModel)]
pub struct Article {
    pub title: String,
    pub content: String,
    /// The author of this article (stored as string ID, NOT a document link)
    pub author_id: EntityIDFor<Author>,
}

// ============================================================================
// Tests
// ============================================================================

/// Test basic GraphQL query to fetch an Author by ID.
#[db_test(db = "graphql_basic")]
async fn test_basic_graphql_query(client: _, spec: _) -> anyhow::Result<()> {
    // Insert schema
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<Author>(args).await?;

    // Insert test data
    let author = Author {
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    let args = DocumentInsertArgs::from(spec.clone());
    let result = client.save_instance(&author, args).await?;
    let author_id = result.root_id;
    println!("Inserted author: {}", author_id);

    // Query the author by ID
    let query = format!(
        r#"
        query {{
            Author(id: "{}") {{
                _id
                name
                email
            }}
        }}
    "#,
        author_id
    );

    println!("Executing query:\n{}", query);

    let request = GraphQLRequest::new(&query);
    let response = client
        .execute_graphql::<serde_json::Value>(&spec.db, Some("main"), request, None)
        .await?;

    // Check for errors
    if let Some(errors) = &response.errors {
        if !errors.is_empty() {
            for error in errors {
                eprintln!("GraphQL error: {}", error.message);
            }
            return Err(anyhow::anyhow!("GraphQL query failed: {:?}", errors));
        }
    }

    // Verify the response
    let data = response
        .data
        .ok_or_else(|| anyhow::anyhow!("No data in response"))?;
    println!("Response: {}", serde_json::to_string_pretty(&data)?);

    let authors = data
        .get("Author")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("Expected Author array"))?;

    assert_eq!(authors.len(), 1, "Expected 1 author");
    assert_eq!(
        authors[0].get("name").and_then(|v| v.as_str()),
        Some("Alice")
    );
    assert_eq!(
        authors[0].get("email").and_then(|v| v.as_str()),
        Some("alice@example.com")
    );

    println!("SUCCESS: Basic GraphQL query works!");
    Ok(())
}

/// Test that Article.author_id is stored as a string (not a document link).
/// This demonstrates the current limitation with `EntityIDFor<T>`.
#[db_test(db = "graphql_entity_id")]
async fn test_entity_id_stored_as_string(client: _, spec: _) -> anyhow::Result<()> {
    // Insert schemas
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<Author>(args).await?;
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<Article>(args).await?;

    // Insert test data
    let author = Author {
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
    };
    let args = DocumentInsertArgs::from(spec.clone());
    let result = client.save_instance(&author, args).await?;
    let author_id = result.root_id;

    let article = Article {
        title: "My First Post".to_string(),
        content: "Hello World!".to_string(),
        author_id: EntityIDFor::new(&author_id)?,
    };
    let args = DocumentInsertArgs::from(spec.clone());
    let result = client.save_instance(&article, args).await?;
    let article_id = result.root_id;
    println!("Inserted article: {}", article_id);

    // Query the article and its author_id field
    let query = format!(
        r#"
        query {{
            Article(id: "{}") {{
                _id
                title
                author_id
            }}
        }}
    "#,
        article_id
    );

    println!("Executing query:\n{}", query);

    let request = GraphQLRequest::new(&query);
    let response = client
        .execute_graphql::<serde_json::Value>(&spec.db, Some("main"), request, None)
        .await?;

    if let Some(errors) = &response.errors {
        if !errors.is_empty() {
            for error in errors {
                eprintln!("GraphQL error: {}", error.message);
            }
            return Err(anyhow::anyhow!("GraphQL query failed: {:?}", errors));
        }
    }

    let data = response
        .data
        .ok_or_else(|| anyhow::anyhow!("No data in response"))?;
    println!("Response: {}", serde_json::to_string_pretty(&data)?);

    let articles = data
        .get("Article")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("Expected Article array"))?;

    assert_eq!(articles.len(), 1, "Expected 1 article");

    // author_id should be a STRING containing the Author's ID
    let author_id_value = articles[0]
        .get("author_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Expected author_id to be a string"))?;

    println!("author_id value: {}", author_id_value);
    assert!(
        author_id_value.contains("Author/"),
        "author_id should be a string containing 'Author/'"
    );

    println!("SUCCESS: EntityIDFor stores as string!");
    Ok(())
}

/// Test the ORM's GraphQLRelationQuery builder generates correct syntax.
/// Note: This tests the query GENERATION, not that the query succeeds against TerminusDB.
#[test]
fn test_query_builder_syntax() {
    // Test single ID query
    let query = GraphQLRelationQuery::new("Author")
        .select("name")
        .select("email")
        .build_with_ids(&["Author/123".to_string()]);

    println!("Single ID query:\n{}", query);
    assert!(query.contains(r#"Author(id: "Author/123")"#));
    assert!(query.contains("name"));
    assert!(query.contains("email"));

    // Test query without IDs (fetches all)
    let query_all = GraphQLRelationQuery::new("Author").select("name").build();

    println!("All query:\n{}", query_all);
    assert!(query_all.contains("Author {"));
    assert!(!query_all.contains("id:"));
}

/// Test that forward relation on EntityIDFor fields is not supported
/// (because they're strings, not document links).
#[db_test(db = "graphql_forward_limit")]
async fn test_forward_relation_limitation(client: _, spec: _) -> anyhow::Result<()> {
    // Insert schemas
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<Author>(args).await?;
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<Article>(args).await?;

    // Insert test data
    let author = Author {
        name: "Charlie".to_string(),
        email: "charlie@example.com".to_string(),
    };
    let args = DocumentInsertArgs::from(spec.clone());
    let result = client.save_instance(&author, args).await?;
    let author_id = result.root_id;

    let article = Article {
        title: "Test Article".to_string(),
        content: "Testing forward relations".to_string(),
        author_id: EntityIDFor::new(&author_id)?,
    };
    let args = DocumentInsertArgs::from(spec.clone());
    let result = client.save_instance(&article, args).await?;
    let article_id = result.root_id;

    // Try to select nested fields on author_id - this should FAIL
    // because author_id is a String, not a linked document
    let query = format!(
        r#"
        query {{
            Article(id: "{}") {{
                _id
                title
                author_id {{
                    _id
                    name
                }}
            }}
        }}
    "#,
        article_id
    );

    println!("Executing forward relation query (should fail):\n{}", query);

    let request = GraphQLRequest::new(&query);
    let response = client
        .execute_graphql::<serde_json::Value>(&spec.db, Some("main"), request, None)
        .await?;

    // This query should have errors because author_id is a String, not a link
    if let Some(errors) = &response.errors {
        if !errors.is_empty() {
            println!("Expected error occurred:");
            for error in errors {
                println!("  - {}", error.message);
            }
            // Verify we got the expected error about subselection on String
            let has_expected_error = errors.iter().any(|e| {
                e.message.contains("must not have a selection")
                    || e.message.contains("has no subfields")
            });
            assert!(
                has_expected_error,
                "Expected error about String not having subfields"
            );
            println!("SUCCESS: Forward relation correctly fails for EntityIDFor fields!");
            return Ok(());
        }
    }

    // If we got here without errors, the test failed
    Err(anyhow::anyhow!(
        "Expected query to fail, but it succeeded"
    ))
}

/// Test that auto-generated reverse fields don't exist for EntityIDFor.
/// This is expected because EntityIDFor creates string fields, not links.
#[db_test(db = "graphql_reverse_limit")]
async fn test_reverse_field_limitation(client: _, spec: _) -> anyhow::Result<()> {
    // Insert schemas
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<Author>(args).await?;
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<Article>(args).await?;

    // Insert test data
    let author = Author {
        name: "Diana".to_string(),
        email: "diana@example.com".to_string(),
    };
    let args = DocumentInsertArgs::from(spec.clone());
    let result = client.save_instance(&author, args).await?;
    let author_id = result.root_id;

    let article = Article {
        title: "Test Reverse".to_string(),
        content: "Testing reverse fields".to_string(),
        author_id: EntityIDFor::new(&author_id)?,
    };
    let args = DocumentInsertArgs::from(spec.clone());
    let _ = client.save_instance(&article, args).await?;

    // Try to use auto-generated reverse field - this should FAIL
    // because EntityIDFor creates strings, not links
    let query = format!(
        r#"
        query {{
            Author(id: "{}") {{
                _id
                name
                _author_id_of_Article {{
                    _id
                    title
                }}
            }}
        }}
    "#,
        author_id
    );

    println!("Executing reverse field query (should fail):\n{}", query);

    let request = GraphQLRequest::new(&query);
    let response = client
        .execute_graphql::<serde_json::Value>(&spec.db, Some("main"), request, None)
        .await?;

    // This query should have errors because the field doesn't exist
    if let Some(errors) = &response.errors {
        if !errors.is_empty() {
            println!("Expected error occurred:");
            for error in errors {
                println!("  - {}", error.message);
            }
            // Verify we got the expected error about unknown field
            let has_expected_error = errors
                .iter()
                .any(|e| e.message.contains("Unknown field"));
            assert!(
                has_expected_error,
                "Expected error about unknown field '_author_id_of_Article'"
            );
            println!("SUCCESS: Reverse field correctly doesn't exist for EntityIDFor!");
            return Ok(());
        }
    }

    // If we got here without errors, the test failed
    Err(anyhow::anyhow!(
        "Expected query to fail, but it succeeded"
    ))
}
