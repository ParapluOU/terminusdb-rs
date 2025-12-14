#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::{CommitId, *};
    use terminusdb_schema::*;
    use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

    /// Test model for specific version retrieval
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
    #[tdb(id_field = "id")]
    struct VersionedProduct {
        id: EntityIDFor<Self>,
        name: String,
        price: f64,
        version: i32,
    }

    #[tokio::test]
    async fn test_get_specific_instance_versions() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_specific_versions", |client, spec| async move {
                // Insert schema
                let args = DocumentInsertArgs::from(spec.clone());
                client
                    .insert_entity_schema::<VersionedProduct>(args)
                    .await
                    .ok();

                let fixed_id = &format!(
                    "product_{}",
                    chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
                );
                println!("=== Testing get_instance_versions with specific commit IDs ===");
                println!("Using ID: {}", fixed_id);

                // Create 5 versions
                let mut all_commit_ids: Vec<CommitId> = Vec::new();

                for i in 1..=5 {
                    let product = VersionedProduct {
                        id: EntityIDFor::new(fixed_id).unwrap(),
                        name: format!("Product V{}", i),
                        price: 10.0 * i as f64,
                        version: i,
                    };

                    let args = DocumentInsertArgs::from(spec.clone());
                    let result = if i == 1 {
                        client.create_instance(&product, args).await?
                    } else {
                        client.update_instance(&product, args).await?
                    };

                    let commit_id = result.extract_commit_id().expect("Should have commit ID");
                    println!("Created version {} in commit: {}", i, &commit_id);
                    all_commit_ids.push(commit_id);
                }

                // Test 1: Get specific versions (2nd, 4th, and 5th)
                println!("\n=== Test 1: Get versions 2, 4, and 5 ===");
                let selected_commits = vec![
                    all_commit_ids[1].clone(), // Version 2
                    all_commit_ids[3].clone(), // Version 4
                    all_commit_ids[4].clone(), // Version 5
                ];

                let mut deserializer = terminusdb_client::deserialize::DefaultTDBDeserializer;
                let versions = client
                    .get_instance_versions::<VersionedProduct>(
                        fixed_id,
                        &spec,
                        selected_commits.clone(),
                        &mut deserializer,
                    )
                    .await?;

                println!("Retrieved {} versions", versions.len());
                for (product, commit_id) in &versions {
                    println!(
                        "  {} (v{}, ${}) in commit {}",
                        product.name, product.version, product.price, commit_id
                    );
                }

                // Verify we got the right versions
                assert_eq!(versions.len(), 3, "Should have retrieved 3 versions");

                let version_numbers: Vec<i32> = versions.iter().map(|(p, _)| p.version).collect();
                assert!(version_numbers.contains(&2));
                assert!(version_numbers.contains(&4));
                assert!(version_numbers.contains(&5));
                assert!(!version_numbers.contains(&1));
                assert!(!version_numbers.contains(&3));

                // Test 2: Get just the first and last versions
                println!("\n=== Test 2: Get first and last versions ===");
                let first_last_commits = vec![
                    all_commit_ids[0].clone(), // Version 1
                    all_commit_ids[4].clone(), // Version 5
                ];

                let versions = client
                    .get_instance_versions::<VersionedProduct>(
                        fixed_id,
                        &spec,
                        first_last_commits,
                        &mut deserializer,
                    )
                    .await?;

                println!("Retrieved {} versions", versions.len());
                for (product, commit_id) in &versions {
                    println!(
                        "  {} (v{}, ${}) in commit {}",
                        product.name, product.version, product.price, commit_id
                    );
                }

                assert_eq!(versions.len(), 2, "Should have retrieved 2 versions");

                // Test 3: Empty commit list
                println!("\n=== Test 3: Empty commit list ===");
                let empty_commits: Vec<CommitId> = vec![];
                let versions = client
                    .get_instance_versions::<VersionedProduct>(
                        fixed_id,
                        &spec,
                        empty_commits,
                        &mut deserializer,
                    )
                    .await?;

                assert_eq!(
                    versions.len(),
                    0,
                    "Empty commit list should return no versions"
                );

                // Test 4: Compare with list_instance_versions
                println!("\n=== Test 4: Compare with list_instance_versions ===");
                let all_versions = client
                    .list_instance_versions::<VersionedProduct>(fixed_id, &spec, &mut deserializer)
                    .await?;

                println!(
                    "list_instance_versions returned {} versions",
                    all_versions.len()
                );
                assert_eq!(all_versions.len(), 5, "Should have all 5 versions");

                // Verify that get_instance_versions with all commit IDs returns the same result
                let all_versions_via_get = client
                    .get_instance_versions::<VersionedProduct>(
                        fixed_id,
                        &spec,
                        all_commit_ids.clone(),
                        &mut deserializer,
                    )
                    .await?;

                assert_eq!(
                    all_versions.len(),
                    all_versions_via_get.len(),
                    "Both methods should return the same number of versions"
                );

                Ok(())
            })
            .await
    }

    #[tokio::test]
    async fn test_get_non_existent_commits() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_nonexistent_commits", |client, spec| async move {
                // Insert schema
                let args = DocumentInsertArgs::from(spec.clone());
                client
                    .insert_entity_schema::<VersionedProduct>(args)
                    .await
                    .ok();

                let fixed_id = &format!(
                    "product_nonexistent_{}",
                    chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
                );

                // Create one version
                let product = VersionedProduct {
                    id: EntityIDFor::new(fixed_id).unwrap(),
                    name: "Test Product".to_string(),
                    price: 99.99,
                    version: 1,
                };

                let args = DocumentInsertArgs::from(spec.clone());
                let result = client.create_instance(&product, args).await?;
                let real_commit_id = result.extract_commit_id().expect("Should have commit ID");

                // Try to get versions with non-existent commit IDs
                let fake_commits = vec![
                    CommitId::new("fake_commit_id_123"),
                    CommitId::new("another_fake_commit_456"),
                    real_commit_id, // Include one real commit
                ];

                let mut deserializer = terminusdb_client::deserialize::DefaultTDBDeserializer;
                let versions = client
                    .get_instance_versions::<VersionedProduct>(
                        fixed_id,
                        &spec,
                        fake_commits,
                        &mut deserializer,
                    )
                    .await?;

                // Should only get the one real version
                assert_eq!(versions.len(), 1, "Should only retrieve the real version");
                assert_eq!(versions[0].0.name, "Test Product");

                Ok(())
            })
            .await
    }
}
