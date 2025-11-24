use anyhow::Result;
use terminusdb_client::*;
use serde_json::json;

/// Test that force=true skips existence checking and uses full_replace
#[tokio::test]
#[ignore] // requires running TerminusDB instance
async fn test_force_insert_with_duplicates() -> Result<()> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let db_name = "test_force_insert";
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

    // Try to insert the same document again with force=true
    // This should succeed and replace the existing document
    let doc2 = json!({
        "@type": "Person",
        "@id": "Person/alice",
        "name": "Alice Updated"
    });

    let result2 = client
        .insert_documents(
            vec![&doc2],
            DocumentInsertArgs::from(spec.clone()).with_force(true),
        )
        .await?;

    assert!(result2.contains_key("Person/alice"));

    // Verify the document was updated
    let retrieved = client.get_document("Person/alice", &spec, GetOpts::default()).await?;
    assert_eq!(retrieved["name"], "Alice Updated");

    Ok(())
}

/// Test that force=false (default) still checks for existing documents
#[tokio::test]
#[ignore] // requires running TerminusDB instance
async fn test_default_insert_checks_duplicates() -> Result<()> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let db_name = "test_default_insert";
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

    // Try to insert the same document again with force=false (default)
    // This should succeed because it automatically converts to PUT
    let doc2 = json!({
        "@type": "Person",
        "@id": "Person/bob",
        "name": "Bob Updated"
    });

    let result2 = client
        .insert_documents(
            vec![&doc2],
            DocumentInsertArgs::from(spec.clone()).with_force(false),
        )
        .await?;

    // Verify the document was updated via PUT
    let retrieved = client.get_document("Person/bob", &spec, GetOpts::default()).await?;
    assert_eq!(retrieved["name"], "Bob Updated");

    Ok(())
}
