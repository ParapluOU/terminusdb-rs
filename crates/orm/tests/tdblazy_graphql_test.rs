//! Integration tests for TdbLazy-based GraphQL relations.
//!
//! TdbLazy<T> creates actual document links in the schema (not strings),
//! which enables TerminusDB's auto-generated reverse fields.

use serde::{Deserialize, Serialize};
use terminusdb_client::graphql::GraphQLRequest;
use terminusdb_client::DocumentInsertArgs;
use terminusdb_schema::{TdbLazy, ToTDBInstance};
use terminusdb_schema_derive::TerminusDBModel;

use terminusdb_schema as terminusdb_schema;

use terminusdb_test::test as db_test;

// ============================================================================
// Test Models using TdbLazy (creates actual document links)
// ============================================================================

#[derive(Clone, Debug, Default, TerminusDBModel)]
pub struct Writer {
    pub name: String,
}

/// A blog post with TdbLazy link to Writer (creates document link, not string!)
#[derive(Clone, Debug, TerminusDBModel)]
pub struct BlogPost {
    pub title: String,
    /// This creates an actual document link in the schema
    pub writer: TdbLazy<Writer>,
}

// ============================================================================
// Tests
// ============================================================================

/// Test that TdbLazy fields create proper document links in the schema.
#[db_test(db = "tdblazy_schema_test")]
async fn test_tdblazy_schema_creates_link(client: _, spec: _) -> anyhow::Result<()> {
    // Insert schemas
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<Writer>(args).await?;

    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<BlogPost>(args).await?;

    // Query the GraphQL schema to check the field type
    let introspection = r#"
        query {
            __type(name: "BlogPost") {
                fields {
                    name
                    type {
                        name
                        kind
                        ofType {
                            name
                            kind
                        }
                    }
                }
            }
        }
    "#;

    let request = GraphQLRequest::new(introspection);
    let response = client
        .execute_graphql::<serde_json::Value>(&spec.db, Some("main"), request, None)
        .await?;

    if let Some(errors) = &response.errors {
        if !errors.is_empty() {
            return Err(anyhow::anyhow!("GraphQL introspection failed: {:?}", errors));
        }
    }

    let data = response.data.ok_or_else(|| anyhow::anyhow!("No data"))?;
    println!("Schema introspection: {}", serde_json::to_string_pretty(&data)?);

    // Check if 'writer' field type is Writer (linked document) not String
    let fields = data
        .pointer("/__type/fields")
        .and_then(|f| f.as_array())
        .ok_or_else(|| anyhow::anyhow!("No fields"))?;

    let writer_field = fields
        .iter()
        .find(|f| f.get("name").and_then(|n| n.as_str()) == Some("writer"))
        .ok_or_else(|| anyhow::anyhow!("No writer field"))?;

    // For NON_NULL types, name is null and actual type is in ofType
    let field_type = writer_field
        .pointer("/type/name")
        .and_then(|t| t.as_str())
        .or_else(|| writer_field.pointer("/type/ofType/name").and_then(|t| t.as_str()));

    println!("writer field type: {:?}", field_type);

    // Should be "Writer" not "String"
    assert_eq!(
        field_type,
        Some("Writer"),
        "Expected writer field to be type Writer, got {:?}",
        field_type
    );

    println!("SUCCESS: TdbLazy creates document link in schema!");
    Ok(())
}

/// Test that forward relation traversal works with TdbLazy.
#[db_test(db = "tdblazy_forward_test")]
async fn test_tdblazy_forward_relation(client: _, spec: _) -> anyhow::Result<()> {
    // Insert schemas
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<Writer>(args).await?;

    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<BlogPost>(args).await?;

    // Insert writer
    let writer = Writer {
        name: "Alice".to_string(),
    };
    let args = DocumentInsertArgs::from(spec.clone());
    let writer_result = client.save_instance(&writer, args).await?;
    let writer_id = writer_result.root_id;
    println!("Inserted writer: {}", writer_id);

    // Insert blog post with TdbLazy link
    let post = BlogPost {
        title: "My First Post".to_string(),
        writer: TdbLazy::new_id(&writer_id)?,
    };
    let args = DocumentInsertArgs::from(spec.clone());
    let post_result = client.save_instance(&post, args).await?;
    let post_id = post_result.root_id;
    println!("Inserted post: {}", post_id);

    // Query with forward relation traversal - this should work with TdbLazy!
    let query = format!(
        r#"
        query {{
            BlogPost(id: "{}") {{
                _id
                title
                writer {{
                    _id
                    name
                }}
            }}
        }}
    "#,
        post_id
    );

    println!("Executing forward relation query:\n{}", query);

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

    let data = response.data.ok_or_else(|| anyhow::anyhow!("No data"))?;
    println!("Response: {}", serde_json::to_string_pretty(&data)?);

    // Verify forward relation worked
    let posts = data.get("BlogPost").and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("Expected BlogPost array"))?;

    let writer_data = posts[0].get("writer")
        .ok_or_else(|| anyhow::anyhow!("Expected writer field"))?;

    let writer_name = writer_data.get("name").and_then(|n| n.as_str());
    assert_eq!(writer_name, Some("Alice"), "Expected writer name to be Alice");

    println!("SUCCESS: Forward relation traversal works with TdbLazy!");
    Ok(())
}

/// Test that auto-generated reverse fields work with TdbLazy.
#[db_test(db = "tdblazy_reverse_test")]
async fn test_tdblazy_reverse_relation(client: _, spec: _) -> anyhow::Result<()> {
    // Insert schemas
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<Writer>(args).await?;

    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<BlogPost>(args).await?;

    // Insert writer
    let writer = Writer { name: "Bob".to_string() };
    let args = DocumentInsertArgs::from(spec.clone());
    let writer_result = client.save_instance(&writer, args).await?;
    let writer_id = writer_result.root_id;

    // Insert multiple blog posts
    for title in ["Post 1", "Post 2", "Post 3"] {
        let post = BlogPost {
            title: title.to_string(),
            writer: TdbLazy::new_id(&writer_id)?,
        };
        let args = DocumentInsertArgs::from(spec.clone());
        client.save_instance(&post, args).await?;
    }

    // Query with reverse relation - should work because TdbLazy creates links!
    // Pattern: _<fieldname>_of_<SourceType>
    let query = format!(
        r#"
        query {{
            Writer(id: "{}") {{
                _id
                name
                _writer_of_BlogPost {{
                    _id
                    title
                }}
            }}
        }}
    "#,
        writer_id
    );

    println!("Executing reverse relation query:\n{}", query);

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

    let data = response.data.ok_or_else(|| anyhow::anyhow!("No data"))?;
    println!("Response: {}", serde_json::to_string_pretty(&data)?);

    // Verify reverse relation worked
    let writers = data.get("Writer").and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("Expected Writer array"))?;

    let posts = writers[0].get("_writer_of_BlogPost").and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("Expected _writer_of_BlogPost array"))?;

    assert_eq!(posts.len(), 3, "Expected 3 posts via reverse relation");

    println!("SUCCESS: Reverse relation works with TdbLazy!");
    Ok(())
}

// ============================================================================
// ORM Relation Loading Tests
// ============================================================================

use terminusdb_orm::prelude::*;

// ReverseRelation impls are now auto-generated by the TerminusDBModel derive macro
// for TdbLazy<T> fields! No manual implementation needed.

/// Test that ORM `.with::<T>()` relation loading works with TdbLazy.
///
/// This tests the two-phase loading:
/// 1. ONE GraphQL query to collect all related IDs
/// 2. ONE batch fetch of all documents
#[db_test(db = "orm_with_relation_test")]
async fn test_orm_with_relation_loads_children(client: _, spec: _) -> anyhow::Result<()> {
    // Insert schemas
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<Writer>(args).await?;

    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<BlogPost>(args).await?;

    // Insert writer
    let writer = Writer { name: "Alice".to_string() };
    let args = DocumentInsertArgs::from(spec.clone());
    let writer_result = client.save_instance(&writer, args).await?;
    let writer_id = writer_result.root_id.clone();
    println!("Inserted writer: {}", writer_id);

    // Insert multiple blog posts linked to the writer
    for title in ["Post 1", "Post 2", "Post 3"] {
        let post = BlogPost {
            title: title.to_string(),
            writer: TdbLazy::new_id(&writer_id)?,
        };
        let args = DocumentInsertArgs::from(spec.clone());
        let result = client.save_instance(&post, args).await?;
        println!("Inserted post: {} -> {}", title, result.root_id);
    }

    // Now test the ORM relation loading!
    // This should:
    // 1. Generate GraphQL: Writer(id: "...") { _id, _writer_of_BlogPost { _id } }
    // 2. Execute GraphQL to get all IDs
    // 3. Batch fetch Writer + 3 BlogPosts

    // Use with_via to specify the exact field name for the GraphQL query
    // The field name "writer" on BlogPost points to Writer
    let result = Writer::find_by_string(&writer_id)
        .with_via::<BlogPost, BlogPostFields::Writer>()
        .with_client(&client)
        .execute(&spec)
        .await?;

    // Verify both types are present in the result
    let writers = result.get::<Writer>()?;
    let posts = result.get::<BlogPost>()?;

    println!("Writers found: {:?}", writers);
    println!("Posts found: {:?}", posts);

    assert_eq!(writers.len(), 1, "Expected 1 writer");
    assert_eq!(posts.len(), 3, "Expected 3 blog posts");
    assert_eq!(writers[0].name, "Alice");

    println!("SUCCESS: ORM .with() relation loading works!");
    Ok(())
}
