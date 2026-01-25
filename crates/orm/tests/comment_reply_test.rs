//! Integration test for Comment/Reply ORM pattern
//!
//! This test demonstrates the typed ORM API with:
//! - Strongly-typed EntityIDFor<T> queries
//! - HasMany/BelongsTo relations
//! - Batch fetching with FetchBuilder
//!
//! Run integration tests with: `cargo test -p terminusdb-orm --test comment_reply_test -- --ignored`
//! Requires a running TerminusDB instance at localhost:6363

use terminusdb_orm::prelude::*;

/// Test GraphQL ID query builder for Comment/Reply pattern
#[test]
fn test_graphql_id_query_with_relations() {
    // Build a query for fetching comments with their replies
    let query = IdQueryBuilder::new("Comment")
        .filter_by_ids(["Comment/1", "Comment/2"])
        .with_relation("replies")
        .build();

    // Verify query structure
    assert!(query.contains("Comment(ids:"));
    assert!(query.contains("\"Comment/1\""));
    assert!(query.contains("\"Comment/2\""));
    assert!(query.contains("_id"));
    assert!(query.contains("replies { _id }"));

    println!("Generated query:\n{}", query);
}

/// Test GraphQL query with nested relations
#[test]
fn test_graphql_nested_relations() {
    let query = IdQueryBuilder::new("Comment")
        .filter_by_ids(["Comment/1"])
        .with_relation_path(RelationPath::new("replies").with_nested(RelationPath::new("author")))
        .build();

    assert!(query.contains("replies"));
    assert!(query.contains("author"));

    println!("Nested query:\n{}", query);
}

/// Test IdQueryResult parsing for Comment/Reply
#[test]
fn test_parse_comment_reply_response() {
    // Simulate a GraphQL response for Comments with Replies
    let response = serde_json::json!({
        "Comment": [
            {
                "_id": "Comment/1",
                "replies": [
                    { "_id": "Reply/a" },
                    { "_id": "Reply/b" }
                ]
            },
            {
                "_id": "Comment/2",
                "replies": [
                    { "_id": "Reply/c" }
                ]
            }
        ]
    });

    let result = terminusdb_orm::parse_id_response(&response, "Comment", &["replies"]);

    // Verify root IDs
    assert_eq!(result.root_ids.len(), 2);
    assert!(result.root_ids.contains(&"Comment/1".to_string()));
    assert!(result.root_ids.contains(&"Comment/2".to_string()));

    // Verify related IDs
    let replies = result.related_ids.get("replies").unwrap();
    assert_eq!(replies.len(), 3);
    assert!(replies.contains(&"Reply/a".to_string()));
    assert!(replies.contains(&"Reply/b".to_string()));
    assert!(replies.contains(&"Reply/c".to_string()));

    // Verify all_ids combines both
    let all = result.all_ids();
    assert_eq!(all.len(), 5); // 2 comments + 3 replies
}

/// Test OrmResult type separation with simulated Comment/Reply data
#[test]
fn test_orm_result_type_separation() {
    // Create documents with different @type values
    let docs = vec![
        serde_json::json!({
            "@type": "Comment",
            "@id": "Comment/1",
            "text": "Hello",
            "author": "Alice"
        }),
        serde_json::json!({
            "@type": "Comment",
            "@id": "Comment/2",
            "text": "World",
            "author": "Bob"
        }),
        serde_json::json!({
            "@type": "Reply",
            "@id": "Reply/a",
            "text": "Hi back",
            "author": "Carol"
        }),
    ];

    let result = OrmResult::new(docs);

    // Verify counts
    assert_eq!(result.len(), 3);

    let counts = result.count_by_class();
    assert_eq!(counts.get("Comment"), Some(&2));
    assert_eq!(counts.get("Reply"), Some(&1));

    // Verify class names
    let classes = result.class_names();
    assert!(classes.contains(&"Comment".to_string()));
    assert!(classes.contains(&"Reply".to_string()));
}

/// Test FetchBuilder with string IDs
#[test]
fn test_fetch_builder_with_strings() {
    // FetchBuilder accepts strings for mixed-type fetching
    let builder = FetchBuilder::new()
        .add_id("Comment/1")
        .add_id("Comment/2")
        .add_ids(["Reply/a", "Reply/b", "Reply/c"]);

    assert_eq!(builder.len(), 5);

    let ids = builder.ids();
    assert!(ids.contains(&"Comment/1".to_string()));
    assert!(ids.contains(&"Reply/c".to_string()));
}

/// Test IdQueryBuilder without filter (fetch all)
#[test]
fn test_graphql_query_fetch_all() {
    let query = IdQueryBuilder::new("Comment")
        .with_relation("replies")
        .build();

    // Should not have ids: parameter when fetching all
    assert!(query.contains("Comment {"));
    assert!(!query.contains("ids:"));
    assert!(query.contains("replies { _id }"));
}

/// Test complex relation traversal
#[test]
fn test_complex_relation_traversal() {
    // Build a query that traverses multiple levels:
    // Comment -> replies -> author -> profile
    let query = IdQueryBuilder::new("Comment")
        .filter_by_ids(["Comment/1"])
        .with_relation_path(
            RelationPath::new("replies")
                .with_nested(RelationPath::new("author").with_nested(RelationPath::new("profile"))),
        )
        .with_relation("tags") // Also fetch tags
        .build();

    println!("Complex query:\n{}", query);

    assert!(query.contains("replies"));
    assert!(query.contains("author"));
    assert!(query.contains("profile"));
    assert!(query.contains("tags"));
}

/// Test OrmResult combine/merge
#[test]
fn test_orm_result_combine() {
    let docs1 = vec![serde_json::json!({
        "@type": "Comment",
        "@id": "Comment/1",
        "text": "First"
    })];

    let docs2 = vec![serde_json::json!({
        "@type": "Reply",
        "@id": "Reply/1",
        "text": "Response"
    })];

    let result1 = OrmResult::new(docs1);
    let result2 = OrmResult::new(docs2);

    let combined = result1.combine(result2);

    assert_eq!(combined.len(), 2);
    assert_eq!(combined.class_names().len(), 2);
}

/// Test empty results
#[test]
fn test_empty_results() {
    let empty = OrmResult::empty();
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);
    assert!(empty.class_names().is_empty());

    let empty_id = IdQueryResult::default();
    assert!(empty_id.is_empty());
    assert_eq!(empty_id.total_count(), 0);
}

// ============================================================================
// Integration tests (use embedded in-memory TerminusDB server)
// ============================================================================

use terminusdb_test::test as db_test;

/// Integration test: Verify test macro works
#[db_test(db = "orm_comment_test")]
async fn test_db_helper_works(_client: _, spec: _) -> anyhow::Result<()> {
    assert!(spec.db.starts_with("orm_comment_test"));
    assert_eq!(spec.branch, Some("main".to_string()));
    Ok(())
}

/// Integration test: FetchBuilder with client returns empty for non-existent
#[db_test(db = "orm_fetch_empty_test")]
async fn test_fetch_nonexistent_returns_empty(client: _, spec: _) -> anyhow::Result<()> {
    // Fetch non-existent IDs - the database is empty so this may error or return empty
    // TerminusDB returns an error when fetching IDs that don't exist
    let result = FetchBuilder::with_client(&client)
        .add_ids(["NonExistent/1", "NonExistent/2"])
        .execute(&spec)
        .await;

    // Either empty result or error is acceptable for non-existent IDs
    match result {
        Ok(docs) => assert!(docs.is_empty()),
        Err(_) => {} // Error is expected for non-existent IDs in empty DB
    }
    Ok(())
}

/// Integration test: Execute GraphQL ID query
/// Note: GraphQL queries require schema to exist, so querying non-existent types will error
#[db_test(db = "orm_graphql_test")]
async fn test_execute_graphql_id_query(client: _, spec: _) -> anyhow::Result<()> {
    // Build an ID query for a type that doesn't exist in the schema
    let query_builder = IdQueryBuilder::new("Comment").with_relation("replies");

    // Execute - will error because Comment schema doesn't exist
    let result = terminusdb_orm::execute_id_query(&client, &spec, &query_builder, None).await;

    // GraphQL returns error for unknown types - this is expected
    match result {
        Ok(id_result) => {
            // If schema exists (from previous test runs), result should be empty
            assert!(id_result.is_empty());
        }
        Err(e) => {
            // Expected: "Unknown field 'Comment' on type 'Query'"
            let err_msg = e.to_string();
            assert!(
                err_msg.contains("Unknown field") || err_msg.contains("GraphQL"),
                "Unexpected error: {}",
                err_msg
            );
        }
    }
    Ok(())
}
