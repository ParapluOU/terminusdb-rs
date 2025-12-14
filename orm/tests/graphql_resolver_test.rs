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
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::graphql::GraphQLRequest;
use terminusdb_client::{BranchSpec, DocumentInsertArgs, TerminusDBHttpClient};
use terminusdb_schema::{EntityIDFor, ToTDBInstance};
use terminusdb_schema_derive::TerminusDBModel;

// Required for the derive macro
use terminusdb_schema as terminusdb_schema;

use terminusdb_orm::prelude::GraphQLRelationQuery;

// ============================================================================
// Test Models
// ============================================================================

/// A user in the system
#[derive(Clone, Debug, Default, Serialize, Deserialize, TerminusDBModel)]
pub struct Author {
    pub name: String,
    pub email: String,
}

/// A blog post that belongs to an author
/// Note: `author_id` is stored as a STRING in the schema, not as a document link!
#[derive(Clone, Debug, Default, Serialize, Deserialize, TerminusDBModel)]
pub struct Article {
    pub title: String,
    pub content: String,
    /// The author of this article (stored as string ID, NOT a document link)
    pub author_id: EntityIDFor<Author>,
}

// ============================================================================
// Test Helpers
// ============================================================================

async fn setup_test_db(prefix: &str) -> anyhow::Result<(TerminusDBHttpClient, String, BranchSpec)> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    let server = TerminusDBServer::test_instance().await?;
    let client = server.client().await?;
    let db_name = format!("{}_{}", prefix, COUNTER.fetch_add(1, Ordering::SeqCst));

    // Delete if exists, then create fresh
    let _ = client.delete_database(&db_name).await;
    client.ensure_database(&db_name).await?;

    let spec = BranchSpec {
        db: db_name.clone(),
        branch: Some("main".to_string()),
        ref_commit: None,
    };

    Ok((client, db_name, spec))
}

async fn insert_schema<T: terminusdb_schema::ToTDBSchema>(
    client: &TerminusDBHttpClient,
    spec: &BranchSpec,
) -> anyhow::Result<()> {
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<T>(args).await
}

async fn insert_instance<T: terminusdb_client::TerminusDBModel>(
    client: &TerminusDBHttpClient,
    spec: &BranchSpec,
    instance: &T,
) -> anyhow::Result<String> {
    let args = DocumentInsertArgs::from(spec.clone());
    let result = client.save_instance(instance, args).await?;
    Ok(result.root_id)
}

async fn cleanup_db(client: &TerminusDBHttpClient, db_name: &str) {
    let _ = client.delete_database(db_name).await;
}

// ============================================================================
// Tests
// ============================================================================

/// Test basic GraphQL query to fetch an Author by ID.
#[tokio::test]
async fn test_basic_graphql_query() -> anyhow::Result<()> {
    let (client, db_name, spec) = setup_test_db("graphql_basic").await?;

    // Insert schema
    insert_schema::<Author>(&client, &spec).await?;

    // Insert test data
    let author = Author {
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
    };
    let author_id = insert_instance(&client, &spec, &author).await?;
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
        .execute_graphql::<serde_json::Value>(&db_name, Some("main"), request, None)
        .await?;

    // Check for errors
    if let Some(errors) = &response.errors {
        if !errors.is_empty() {
            for error in errors {
                eprintln!("GraphQL error: {}", error.message);
            }
            cleanup_db(&client, &db_name).await;
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

    cleanup_db(&client, &db_name).await;
    Ok(())
}

/// Test that Article.author_id is stored as a string (not a document link).
/// This demonstrates the current limitation with `EntityIDFor<T>`.
#[tokio::test]
async fn test_entity_id_stored_as_string() -> anyhow::Result<()> {
    let (client, db_name, spec) = setup_test_db("graphql_entity_id").await?;

    // Insert schemas
    insert_schema::<Author>(&client, &spec).await?;
    insert_schema::<Article>(&client, &spec).await?;

    // Insert test data
    let author = Author {
        name: "Bob".to_string(),
        email: "bob@example.com".to_string(),
    };
    let author_id = insert_instance(&client, &spec, &author).await?;

    let article = Article {
        title: "My First Post".to_string(),
        content: "Hello World!".to_string(),
        author_id: EntityIDFor::new(&author_id)?,
    };
    let article_id = insert_instance(&client, &spec, &article).await?;
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
        .execute_graphql::<serde_json::Value>(&db_name, Some("main"), request, None)
        .await?;

    if let Some(errors) = &response.errors {
        if !errors.is_empty() {
            for error in errors {
                eprintln!("GraphQL error: {}", error.message);
            }
            cleanup_db(&client, &db_name).await;
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

    cleanup_db(&client, &db_name).await;
    Ok(())
}

/// Test the ORM's GraphQLRelationQuery builder generates correct syntax.
/// Note: This tests the query GENERATION, not that the query succeeds against TerminusDB.
#[tokio::test]
async fn test_query_builder_syntax() {
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
#[tokio::test]
async fn test_forward_relation_limitation() -> anyhow::Result<()> {
    let (client, db_name, spec) = setup_test_db("graphql_forward_limit").await?;

    // Insert schemas
    insert_schema::<Author>(&client, &spec).await?;
    insert_schema::<Article>(&client, &spec).await?;

    // Insert test data
    let author = Author {
        name: "Charlie".to_string(),
        email: "charlie@example.com".to_string(),
    };
    let author_id = insert_instance(&client, &spec, &author).await?;

    let article = Article {
        title: "Test Article".to_string(),
        content: "Testing forward relations".to_string(),
        author_id: EntityIDFor::new(&author_id)?,
    };
    let article_id = insert_instance(&client, &spec, &article).await?;

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
        .execute_graphql::<serde_json::Value>(&db_name, Some("main"), request, None)
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
            cleanup_db(&client, &db_name).await;
            return Ok(());
        }
    }

    // If we got here without errors, the test failed
    cleanup_db(&client, &db_name).await;
    Err(anyhow::anyhow!(
        "Expected query to fail, but it succeeded"
    ))
}

/// Test that auto-generated reverse fields don't exist for EntityIDFor.
/// This is expected because EntityIDFor creates string fields, not links.
#[tokio::test]
async fn test_reverse_field_limitation() -> anyhow::Result<()> {
    let (client, db_name, spec) = setup_test_db("graphql_reverse_limit").await?;

    // Insert schemas
    insert_schema::<Author>(&client, &spec).await?;
    insert_schema::<Article>(&client, &spec).await?;

    // Insert test data
    let author = Author {
        name: "Diana".to_string(),
        email: "diana@example.com".to_string(),
    };
    let author_id = insert_instance(&client, &spec, &author).await?;

    let article = Article {
        title: "Test Reverse".to_string(),
        content: "Testing reverse fields".to_string(),
        author_id: EntityIDFor::new(&author_id)?,
    };
    let _ = insert_instance(&client, &spec, &article).await?;

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
        .execute_graphql::<serde_json::Value>(&db_name, Some("main"), request, None)
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
            cleanup_db(&client, &db_name).await;
            return Ok(());
        }
    }

    // If we got here without errors, the test failed
    cleanup_db(&client, &db_name).await;
    Err(anyhow::anyhow!(
        "Expected query to fail, but it succeeded"
    ))
}
