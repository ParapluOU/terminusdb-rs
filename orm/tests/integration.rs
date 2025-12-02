//! Integration tests for terminusdb-orm
//!
//! These tests require a running TerminusDB instance at localhost:6363.
//! Run with: `cargo test -p terminusdb-orm --test integration -- --ignored`

use terminusdb_orm::prelude::*;
use terminusdb_orm::testing::TestDb;

/// Test that ORM client initialization works
#[tokio::test]
#[ignore = "Requires running TerminusDB instance"]
async fn test_orm_client_init() {
    let client = TerminusDBHttpClient::local_node().await;

    // Initialize should succeed (note: may fail if already initialized in another test)
    let _ = OrmClient::init(client);

    // Client should be initialized
    assert!(OrmClient::is_initialized());
}

/// Test basic fetch operations with TestDb helper
#[tokio::test]
#[ignore = "Requires running TerminusDB instance"]
async fn test_fetch_by_ids() {
    // Create a test database
    let test_db = TestDb::new("orm_fetch_test").await.unwrap();
    let client = test_db.client();
    let spec = test_db.spec();

    // Fetch non-existent IDs - TerminusDB may return error or empty
    let result = FetchBuilder::with_client(client)
        .add_ids(["NonExistent/1"])
        .execute(&spec)
        .await;

    // Either empty result or error is acceptable for non-existent IDs
    match result {
        Ok(docs) => assert!(docs.is_empty()),
        Err(_) => {} // Error is expected for non-existent IDs
    }
}

/// Test TestDb helper creation and cleanup
#[tokio::test]
#[ignore = "Requires running TerminusDB instance"]
async fn test_test_db_helper() {
    let test_db = TestDb::new("orm_helper_test").await.unwrap();

    assert!(test_db.db_name().starts_with("orm_helper_test_"));
    assert_eq!(test_db.org(), "admin");

    let spec = test_db.spec();
    assert_eq!(spec.branch, Some("main".to_string()));
}

/// Test GraphQL ID query building
#[test]
fn test_graphql_id_query_building() {
    let query = IdQueryBuilder::new("Comment")
        .filter_by_ids(["Comment/1", "Comment/2"])
        .with_relation("replies")
        .build();

    // Verify the query structure
    assert!(query.contains("Comment(ids:"));
    assert!(query.contains("\"Comment/1\""));
    assert!(query.contains("\"Comment/2\""));
    assert!(query.contains("_id"));
    assert!(query.contains("replies { _id }"));
}

/// Test GraphQL query without filter
#[test]
fn test_graphql_query_no_filter() {
    let query = IdQueryBuilder::new("Comment")
        .with_relation("replies")
        .build();

    // Should not have "ids:" parameter
    assert!(query.contains("Comment {"));
    assert!(!query.contains("ids:"));
    assert!(query.contains("replies { _id }"));
}

/// Test GraphQL response parsing
#[test]
fn test_graphql_response_parsing() {
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
                "replies": []
            }
        ]
    });

    let result = terminusdb_orm::parse_id_response(&response, "Comment", &["replies"]);

    // Verify parsed IDs
    assert_eq!(result.root_ids.len(), 2);
    assert!(result.root_ids.contains(&"Comment/1".to_string()));
    assert!(result.root_ids.contains(&"Comment/2".to_string()));

    let replies = result.related_ids.get("replies").unwrap();
    assert_eq!(replies.len(), 2);
    assert!(replies.contains(&"Reply/1".to_string()));
    assert!(replies.contains(&"Reply/2".to_string()));
}

/// Test IdQueryResult all_ids collection
#[test]
fn test_id_query_result_all_ids() {
    let response = serde_json::json!({
        "Parent": [
            { "_id": "Parent/1", "children": [{ "_id": "Child/1" }] },
            { "_id": "Parent/2", "children": [{ "_id": "Child/2" }, { "_id": "Child/3" }] }
        ]
    });

    let result = terminusdb_orm::parse_id_response(&response, "Parent", &["children"]);

    let all_ids = result.all_ids();
    assert_eq!(all_ids.len(), 5); // 2 parents + 3 children
}

/// Test OrmResult basic operations
#[test]
fn test_orm_result_basic() {
    let docs = vec![
        serde_json::json!({ "@type": "TypeA", "@id": "TypeA/1" }),
        serde_json::json!({ "@type": "TypeB", "@id": "TypeB/1" }),
        serde_json::json!({ "@type": "TypeA", "@id": "TypeA/2" }),
    ];

    let result = OrmResult::new(docs);

    // Check length
    assert_eq!(result.len(), 3);
    assert!(!result.is_empty());

    // Check class names
    let names = result.class_names();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"TypeA".to_string()));
    assert!(names.contains(&"TypeB".to_string()));

    // Check counts
    let counts = result.count_by_class();
    assert_eq!(counts.get("TypeA"), Some(&2));
    assert_eq!(counts.get("TypeB"), Some(&1));
}

/// Test OrmResult merge
#[test]
fn test_orm_result_merge() {
    let docs1 = vec![serde_json::json!({ "@type": "A", "@id": "A/1" })];
    let docs2 = vec![serde_json::json!({ "@type": "B", "@id": "B/1" })];

    let result1 = OrmResult::new(docs1);
    let result2 = OrmResult::new(docs2);

    let combined = result1.combine(result2);
    assert_eq!(combined.len(), 2);
}

/// Test FetchBuilder construction
#[test]
fn test_fetch_builder() {
    let builder = FetchBuilder::new()
        .add_id("Type/1")
        .add_ids(["Type/2", "Type/3"]);

    assert_eq!(builder.len(), 3);
    assert!(!builder.is_empty());

    let ids = builder.ids();
    assert!(ids.contains(&"Type/1".to_string()));
    assert!(ids.contains(&"Type/2".to_string()));
    assert!(ids.contains(&"Type/3".to_string()));
}

/// Test FetchBuilder with unfold
#[test]
fn test_fetch_builder_with_unfold() {
    let builder = FetchBuilder::new()
        .add_id("Type/1")
        .unfold();

    assert_eq!(builder.len(), 1);
}

/// Test RelationPath construction
#[test]
fn test_relation_path() {
    let path = RelationPath::new("parent")
        .with_nested(RelationPath::new("grandparent"));

    let selection = path.build_selection();
    assert!(selection.contains("parent"));
    assert!(selection.contains("grandparent"));
    assert!(selection.contains("_id"));
}

/// Test IdQueryBuilder for_type (compile-time check)
#[test]
fn test_id_query_builder_type_inference() {
    // This tests that the builder can be created without explicit type parameters
    let _builder = IdQueryBuilder::new("SomeType")
        .filter_by_ids(["SomeType/1"])
        .with_relation("related");
}

/// Test empty results handling
#[test]
fn test_empty_results() {
    let empty_result = OrmResult::empty();
    assert!(empty_result.is_empty());
    assert_eq!(empty_result.len(), 0);
    assert!(empty_result.class_names().is_empty());

    let empty_id_result = IdQueryResult::default();
    assert!(empty_id_result.is_empty());
    assert_eq!(empty_id_result.total_count(), 0);
}
