#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::*;
    use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

    /// Test model for semantic API testing
    #[derive(
        Debug, Clone, PartialEq, Serialize, Deserialize, Default, TerminusDBModel, FromTDBInstance,
    )]
    #[tdb(id_field = "id")]
    struct SemanticTestModel {
        id: EntityIDFor<Self>,
        name: String,
        value: i32,
    }

    #[tokio::test]
    async fn test_create_instance_generates_commit() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_create_commit", |client, spec| async move {
                // Insert schema
                let args = DocumentInsertArgs::from(spec.clone());
                client
                    .insert_entity_schema::<SemanticTestModel>(args.clone())
                    .await
                    .ok();

                // Create a new instance using create_instance
                let model = SemanticTestModel {
                    id: EntityIDFor::new("semantic_test_001").unwrap(),
                    name: "Test Create".to_string(),
                    value: 42,
                };

                let args = DocumentInsertArgs::from(spec.clone());
                let result = client.create_instance(&model, args).await?;

                // Verify it was created
                assert!(!result.is_empty());

                // Verify commit ID is present
                assert!(
                    result.commit_id.is_some(),
                    "create_instance should generate a commit with ID"
                );

                // Try to create again - should fail
                let args = DocumentInsertArgs::from(spec.clone());
                let result2 = client.create_instance(&model, args).await;
                assert!(
                    result2.is_err(),
                    "create_instance should fail if instance already exists"
                );

                Ok(())
            })
            .await
    }

    #[tokio::test]
    async fn test_update_instance_requires_existing() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_update_requires", |client, spec| async move {
                // Insert schema
                let args = DocumentInsertArgs::from(spec.clone());
                client
                    .insert_entity_schema::<SemanticTestModel>(args.clone())
                    .await
                    .ok();

                // Try to update non-existent instance - should fail
                let model = SemanticTestModel {
                    id: EntityIDFor::new("semantic_test_update_001").unwrap(),
                    name: "Test Update".to_string(),
                    value: 100,
                };

                let args = DocumentInsertArgs::from(spec.clone());
                let result = client.update_instance(&model, args).await;
                assert!(
                    result.is_err(),
                    "update_instance should fail if instance doesn't exist"
                );

                // First create it
                let args = DocumentInsertArgs::from(spec.clone());
                client.create_instance(&model, args).await?;

                // Now update should work
                let updated_model = SemanticTestModel {
                    id: EntityIDFor::new("semantic_test_update_001").unwrap(),
                    name: "Test Update Modified".to_string(),
                    value: 200,
                };

                let args = DocumentInsertArgs::from(spec.clone());
                let update_result = client.update_instance(&updated_model, args).await?;

                assert!(!update_result.is_empty());
                assert!(
                    update_result.commit_id.is_some(),
                    "update_instance should generate a commit with ID"
                );

                Ok(())
            })
            .await
    }

    #[tokio::test]
    async fn test_save_instance_creates_or_updates() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_save_creates", |client, spec| async move {
                // Insert schema
                let args = DocumentInsertArgs::from(spec.clone());
                client
                    .insert_entity_schema::<SemanticTestModel>(args.clone())
                    .await
                    .ok();

                // First save - should create
                let model = SemanticTestModel {
                    id: EntityIDFor::new("semantic_test_save_001").unwrap(),
                    name: "Test Save".to_string(),
                    value: 300,
                };

                let args = DocumentInsertArgs::from(spec.clone());
                let result1 = client.save_instance(&model, args).await?;

                assert!(!result1.is_empty());
                assert!(
                    result1.commit_id.is_some(),
                    "save_instance should generate a commit with ID"
                );

                // Verify it exists
                let mut deserializer = deserialize::DefaultTDBDeserializer;
                let retrieved = client
                    .get_instance::<SemanticTestModel>(
                        "semantic_test_save_001",
                        &spec,
                        &mut deserializer,
                    )
                    .await?;
                assert_eq!(retrieved.name, "Test Save");
                assert_eq!(retrieved.value, 300);

                // Second save with updated data - should update
                let updated_model = SemanticTestModel {
                    id: EntityIDFor::new("semantic_test_save_001").unwrap(),
                    name: "Test Save Updated".to_string(),
                    value: 400,
                };

                let args = DocumentInsertArgs::from(spec.clone());
                let result2 = client.save_instance(&updated_model, args).await?;

                assert!(!result2.is_empty());
                assert!(
                    result2.commit_id.is_some(),
                    "save_instance update should generate a commit with ID"
                );

                // Verify update worked
                let retrieved2 = client
                    .get_instance::<SemanticTestModel>(
                        "semantic_test_save_001",
                        &spec,
                        &mut deserializer,
                    )
                    .await?;
                assert_eq!(retrieved2.name, "Test Save Updated");
                assert_eq!(retrieved2.value, 400);

                Ok(())
            })
            .await
    }

    #[tokio::test]
    async fn test_semantic_methods_generate_proper_version_history() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_version_history", |client, spec| async move {
                // Insert schema
                let args = DocumentInsertArgs::from(spec.clone());
                client
                    .insert_entity_schema::<SemanticTestModel>(args.clone())
                    .await
                    .ok();

                let fixed_id = "semantic_version_test_001";

                // Step 1: Create with create_instance
                let v1 = SemanticTestModel {
                    id: EntityIDFor::new(fixed_id).unwrap(),
                    name: "Version 1".to_string(),
                    value: 1,
                };

                let args = DocumentInsertArgs::from(spec.clone());
                let create_result = client.create_instance(&v1, args).await?;
                let commit1 = create_result
                    .extract_commit_id()
                    .expect("Should have commit ID");
                println!("Created instance in commit: {}", commit1);

                // Step 2: Update with update_instance
                let v2 = SemanticTestModel {
                    id: EntityIDFor::new(fixed_id).unwrap(),
                    name: "Version 2".to_string(),
                    value: 2,
                };

                let args = DocumentInsertArgs::from(spec.clone());
                let update_result = client.update_instance(&v2, args).await?;
                let commit2 = update_result
                    .extract_commit_id()
                    .expect("Should have commit ID");
                println!("Updated instance in commit: {}", commit2);

                // Step 3: Update again with save_instance
                let v3 = SemanticTestModel {
                    id: EntityIDFor::new(fixed_id).unwrap(),
                    name: "Version 3".to_string(),
                    value: 3,
                };

                let args = DocumentInsertArgs::from(spec.clone());
                let save_result = client.save_instance(&v3, args).await?;
                let commit3 = save_result
                    .extract_commit_id()
                    .expect("Should have commit ID");
                println!("Saved instance in commit: {}", commit3);

                // Verify all commits are different
                assert_ne!(
                    commit1, commit2,
                    "Each operation should generate a unique commit"
                );
                assert_ne!(
                    commit2, commit3,
                    "Each operation should generate a unique commit"
                );
                assert_ne!(
                    commit1, commit3,
                    "Each operation should generate a unique commit"
                );

                // Get version history
                let history = client
                    .get_instance_history::<SemanticTestModel>(fixed_id, &spec, None)
                    .await?;
                println!("Found {} commits in history", history.len());

                // Should have at least 3 commits
                assert!(
                    history.len() >= 3,
                    "Should have at least 3 commits in history"
                );

                // Get all versions
                let mut deserializer = deserialize::DefaultTDBDeserializer;
                let versions = client
                    .list_instance_versions::<SemanticTestModel>(fixed_id, &spec, &mut deserializer)
                    .await?;

                println!("Retrieved {} versions", versions.len());
                for (i, (model, commit)) in versions.iter().enumerate() {
                    println!(
                        "Version {}: {} (value={}) in commit {}",
                        i + 1,
                        model.name,
                        model.value,
                        commit
                    );
                }

                // Should have all 3 versions
                assert!(versions.len() >= 3, "Should have at least 3 versions");

                Ok(())
            })
            .await
    }

    #[tokio::test]
    async fn test_replace_instance_is_alias_for_update() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_replace_alias", |client, spec| async move {
                // Insert schema
                let args = DocumentInsertArgs::from(spec.clone());
                client
                    .insert_entity_schema::<SemanticTestModel>(args.clone())
                    .await
                    .ok();

                // Create instance first
                let model = SemanticTestModel {
                    id: EntityIDFor::new("semantic_test_replace_001").unwrap(),
                    name: "Test Replace".to_string(),
                    value: 500,
                };

                let args = DocumentInsertArgs::from(spec.clone());
                client.create_instance(&model, args).await?;

                // Replace should work like update
                let replaced_model = SemanticTestModel {
                    id: EntityIDFor::new("semantic_test_replace_001").unwrap(),
                    name: "Test Replace Modified".to_string(),
                    value: 600,
                };

                let args = DocumentInsertArgs::from(spec.clone());
                let replace_result = client.replace_instance(&replaced_model, args).await?;

                assert!(!replace_result.is_empty());
                assert!(
                    replace_result.commit_id.is_some(),
                    "replace_instance should generate a commit with ID"
                );

                // Verify the update worked
                let mut deserializer = deserialize::DefaultTDBDeserializer;
                let retrieved = client
                    .get_instance::<SemanticTestModel>(
                        "semantic_test_replace_001",
                        &spec,
                        &mut deserializer,
                    )
                    .await?;
                assert_eq!(retrieved.name, "Test Replace Modified");
                assert_eq!(retrieved.value, 600);

                Ok(())
            })
            .await
    }
}
