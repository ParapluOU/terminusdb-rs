use anyhow::Result;
use terminusdb_client::*;
use serde_json::json;

/// Test the default behavior: check=false, force=false (safe, checks and updates via PUT)
#[tokio::test]
#[ignore] // requires running TerminusDB instance
async fn test_default_safe_insert() -> Result<()> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let db_name = "test_default_safe";
    let spec = BranchSpec::from(db_name);

    // Reset database
    client.reset_database(&spec.db).await?;

    // Define a simple schema
    let schema = json!({
        "@type": "Class",
        "@id": "Person",
        "name": "xsd:string"
    });

    client
        .insert_documents(vec![&schema], DocumentInsertArgs::from(spec.clone()).as_schema())
        .await?;

    // Insert first document
    let doc1 = json!({
        "@type": "Person",
        "@id": "Person/alice",
        "name": "Alice"
    });

    let result1 = client
        .insert_documents(vec![&doc1], DocumentInsertArgs::from(spec.clone()))
        .await?;

    assert!(result1.contains_key("Person/alice"));

    // Try to insert the same document again with default settings
    // skip_existence_check=false, force=false
    // This should check, find existing, and update via PUT
    let doc2 = json!({
        "@type": "Person",
        "@id": "Person/alice",
        "name": "Alice Updated"
    });

    let result2 = client
        .insert_documents(vec![&doc2], DocumentInsertArgs::from(spec.clone()))
        .await?;

    // Verify the document was updated via PUT
    let retrieved = client.get_document("Person/alice", &spec, GetOpts::default()).await?;
    assert_eq!(retrieved["name"], "Alice Updated");

    Ok(())
}

/// Test: skip_existence_check=true, force=true (fastest, skips check and replaces any duplicates)
#[tokio::test]
#[ignore] // requires running TerminusDB instance
async fn test_skip_check_with_force() -> Result<()> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let db_name = "test_skip_force";
    let spec = BranchSpec::from(db_name);

    // Reset database
    client.reset_database(&spec.db).await?;

    // Define a simple schema
    let schema = json!({
        "@type": "Class",
        "@id": "Person",
        "name": "xsd:string"
    });

    client
        .insert_documents(vec![&schema], DocumentInsertArgs::from(spec.clone()).as_schema())
        .await?;

    // Insert first document
    let doc1 = json!({
        "@type": "Person",
        "@id": "Person/bob",
        "name": "Bob"
    });

    let result1 = client
        .insert_documents(vec![&doc1], DocumentInsertArgs::from(spec.clone()))
        .await?;

    assert!(result1.contains_key("Person/bob"));

    // Insert same document with skip_existence_check=true and force=true
    // This is the fastest combination - no check, uses full_replace
    let doc2 = json!({
        "@type": "Person",
        "@id": "Person/bob",
        "name": "Bob Updated"
    });

    let result2 = client
        .insert_documents(
            vec![&doc2],
            DocumentInsertArgs::from(spec.clone())
                .with_skip_existence_check(true)
                .with_force(true),
        )
        .await?;

    assert!(result2.contains_key("Person/bob"));

    // Verify the document was replaced
    let retrieved = client.get_document("Person/bob", &spec, GetOpts::default()).await?;
    assert_eq!(retrieved["name"], "Bob Updated");

    Ok(())
}

/// Test: skip_existence_check=false, force=true (checks then replaces - validates but always succeeds)
#[tokio::test]
#[ignore] // requires running TerminusDB instance
async fn test_check_with_force() -> Result<()> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let db_name = "test_check_force";
    let spec = BranchSpec::from(db_name);

    // Reset database
    client.reset_database(&spec.db).await?;

    // Define a simple schema
    let schema = json!({
        "@type": "Class",
        "@id": "Person",
        "name": "xsd:string"
    });

    client
        .insert_documents(vec![&schema], DocumentInsertArgs::from(spec.clone()).as_schema())
        .await?;

    // Insert first document
    let doc1 = json!({
        "@type": "Person",
        "@id": "Person/charlie",
        "name": "Charlie"
    });

    let result1 = client
        .insert_documents(vec![&doc1], DocumentInsertArgs::from(spec.clone()))
        .await?;

    assert!(result1.contains_key("Person/charlie"));

    // Insert same document with skip_existence_check=false and force=true
    // This checks for existing documents but still uses force to replace
    // The check filters them to PUT, which with force should work
    let doc2 = json!({
        "@type": "Person",
        "@id": "Person/charlie",
        "name": "Charlie Updated"
    });

    let result2 = client
        .insert_documents(
            vec![&doc2],
            DocumentInsertArgs::from(spec.clone())
                .with_skip_existence_check(false)
                .with_force(true),
        )
        .await?;

    // Verify the document was updated
    let retrieved = client.get_document("Person/charlie", &spec, GetOpts::default()).await?;
    assert_eq!(retrieved["name"], "Charlie Updated");

    Ok(())
}

/// Test: skip_existence_check=true, force=false (fast but may error on duplicates)
/// This test verifies that skipping the check without force will cause an error on duplicate
#[tokio::test]
#[ignore] // requires running TerminusDB instance
async fn test_skip_check_without_force_errors_on_duplicate() -> Result<()> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let db_name = "test_skip_no_force";
    let spec = BranchSpec::from(db_name);

    // Reset database
    client.reset_database(&spec.db).await?;

    // Define a simple schema
    let schema = json!({
        "@type": "Class",
        "@id": "Person",
        "name": "xsd:string"
    });

    client
        .insert_documents(vec![&schema], DocumentInsertArgs::from(spec.clone()).as_schema())
        .await?;

    // Insert first document
    let doc1 = json!({
        "@type": "Person",
        "@id": "Person/diana",
        "name": "Diana"
    });

    let result1 = client
        .insert_documents(vec![&doc1], DocumentInsertArgs::from(spec.clone()))
        .await?;

    assert!(result1.contains_key("Person/diana"));

    // Try to insert same document with skip_existence_check=true and force=false
    // This should fail because we skip the check and don't use full_replace
    let doc2 = json!({
        "@type": "Person",
        "@id": "Person/diana",
        "name": "Diana Updated"
    });

    let result2 = client
        .insert_documents(
            vec![&doc2],
            DocumentInsertArgs::from(spec.clone())
                .with_skip_existence_check(true)
                .with_force(false),
        )
        .await;

    // This should fail with a duplicate ID error
    assert!(result2.is_err(), "Expected error due to duplicate ID without force");

    // Verify the original document is unchanged
    let retrieved = client.get_document("Person/diana", &spec, GetOpts::default()).await?;
    assert_eq!(retrieved["name"], "Diana");

    Ok(())
}

/// Test: Bulk insert with skip_existence_check=true for performance
#[tokio::test]
#[ignore] // requires running TerminusDB instance
async fn test_bulk_insert_with_skip_check() -> Result<()> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let db_name = "test_bulk_skip";
    let spec = BranchSpec::from(db_name);

    // Reset database
    client.reset_database(&spec.db).await?;

    // Define a simple schema
    let schema = json!({
        "@type": "Class",
        "@id": "Person",
        "name": "xsd:string"
    });

    client
        .insert_documents(vec![&schema], DocumentInsertArgs::from(spec.clone()).as_schema())
        .await?;

    // Insert many documents at once with skip_existence_check=true for performance
    let docs: Vec<_> = (0..100)
        .map(|i| {
            json!({
                "@type": "Person",
                "@id": format!("Person/user{}", i),
                "name": format!("User {}", i)
            })
        })
        .collect();

    let doc_refs: Vec<_> = docs.iter().collect();

    let result = client
        .insert_documents(
            doc_refs,
            DocumentInsertArgs::from(spec.clone())
                .with_skip_existence_check(true)
                .with_force(true),
        )
        .await?;

    assert_eq!(result.len(), 100);

    Ok(())
}
