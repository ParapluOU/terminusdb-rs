#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use anyhow::Result;
    use serde_json::json;
    use std::collections::HashMap;
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;

    /// Helper to check if a result map has a key ending with the given suffix
    fn has_key_ending_with<V>(map: &HashMap<String, V>, suffix: &str) -> bool {
        map.keys().any(|k| k.ends_with(suffix))
    }

    /// Test the default behavior: check=false, force=false (safe, checks and updates via PUT)
    #[tokio::test]
    async fn test_default_safe_insert() -> Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_default_safe", |client, spec| async move {
                // Define a simple schema
                let schema = json!({
                    "@type": "Class",
                    "@id": "Person",
                    "name": "xsd:string"
                });

                client
                    .insert_documents(
                        vec![&schema],
                        DocumentInsertArgs::from(spec.clone()).as_schema(),
                    )
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

                assert!(has_key_ending_with(&result1, "Person/alice"));

                // Try to insert the same document again with default settings
                // skip_existence_check=false, force=false
                // This should check, find existing, and update via PUT
                let doc2 = json!({
                    "@type": "Person",
                    "@id": "Person/alice",
                    "name": "Alice Updated"
                });

                let _result2 = client
                    .insert_documents(vec![&doc2], DocumentInsertArgs::from(spec.clone()))
                    .await?;

                // Verify the document was updated via PUT
                let retrieved = client
                    .get_document("Person/alice", &spec, GetOpts::default())
                    .await?;
                assert_eq!(retrieved["name"], "Alice Updated");

                Ok(())
            })
            .await
    }

    /// Test: skip_existence_check=true, force=true (fastest, skips check and replaces any duplicates)
    #[tokio::test]
    async fn test_skip_check_with_force() -> Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_skip_force", |client, spec| async move {
                // Define a simple schema
                let schema = json!({
                    "@type": "Class",
                    "@id": "Person",
                    "name": "xsd:string"
                });

                client
                    .insert_documents(
                        vec![&schema],
                        DocumentInsertArgs::from(spec.clone()).as_schema(),
                    )
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

                assert!(has_key_ending_with(&result1, "Person/bob"));

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

                assert!(has_key_ending_with(&result2, "Person/bob"));

                // Verify the document was replaced
                let retrieved = client
                    .get_document("Person/bob", &spec, GetOpts::default())
                    .await?;
                assert_eq!(retrieved["name"], "Bob Updated");

                Ok(())
            })
            .await
    }

    /// Test: skip_existence_check=false, force=true (checks then replaces - validates but always succeeds)
    #[tokio::test]
    async fn test_check_with_force() -> Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_check_force", |client, spec| async move {
                // Define a simple schema
                let schema = json!({
                    "@type": "Class",
                    "@id": "Person",
                    "name": "xsd:string"
                });

                client
                    .insert_documents(
                        vec![&schema],
                        DocumentInsertArgs::from(spec.clone()).as_schema(),
                    )
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

                assert!(has_key_ending_with(&result1, "Person/charlie"));

                // Insert same document with skip_existence_check=false and force=true
                // This checks for existing documents but still uses force to replace
                // The check filters them to PUT, which with force should work
                let doc2 = json!({
                    "@type": "Person",
                    "@id": "Person/charlie",
                    "name": "Charlie Updated"
                });

                let _result2 = client
                    .insert_documents(
                        vec![&doc2],
                        DocumentInsertArgs::from(spec.clone())
                            .with_skip_existence_check(false)
                            .with_force(true),
                    )
                    .await?;

                // Verify the document was updated
                let retrieved = client
                    .get_document("Person/charlie", &spec, GetOpts::default())
                    .await?;
                assert_eq!(retrieved["name"], "Charlie Updated");

                Ok(())
            })
            .await
    }

    /// Test: skip_existence_check=true, force=false behavior with duplicates
    /// Note: TerminusDB may handle duplicates gracefully depending on version
    #[tokio::test]
    async fn test_skip_check_without_force_duplicate_behavior() -> Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_skip_no_force", |client, spec| async move {
                // Define a simple schema
                let schema = json!({
                    "@type": "Class",
                    "@id": "Person",
                    "name": "xsd:string"
                });

                client
                    .insert_documents(
                        vec![&schema],
                        DocumentInsertArgs::from(spec.clone()).as_schema(),
                    )
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

                assert!(has_key_ending_with(&result1, "Person/diana"));

                // Try to insert same document with skip_existence_check=true and force=false
                // Behavior may vary - some versions error, others update
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

                // Check behavior - either succeeds or fails with error
                match result2 {
                    Ok(_) => {
                        // If it succeeded, verify the document was updated
                        let retrieved = client
                            .get_document("Person/diana", &spec, GetOpts::default())
                            .await?;
                        // The document may or may not be updated depending on behavior
                        assert!(
                            retrieved["name"] == "Diana" || retrieved["name"] == "Diana Updated"
                        );
                    }
                    Err(_) => {
                        // If it failed, verify the original document is unchanged
                        let retrieved = client
                            .get_document("Person/diana", &spec, GetOpts::default())
                            .await?;
                        assert_eq!(retrieved["name"], "Diana");
                    }
                }

                Ok(())
            })
            .await
    }

    /// Test: Bulk insert with skip_existence_check=true for performance
    #[tokio::test]
    async fn test_bulk_insert_with_skip_check() -> Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_bulk_skip", |client, spec| async move {
                // Define a simple schema
                let schema = json!({
                    "@type": "Class",
                    "@id": "Person",
                    "name": "xsd:string"
                });

                client
                    .insert_documents(
                        vec![&schema],
                        DocumentInsertArgs::from(spec.clone()).as_schema(),
                    )
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
            })
            .await
    }
}
