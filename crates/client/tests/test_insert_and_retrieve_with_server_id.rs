#![cfg(not(target_arch = "wasm32"))]
#![cfg(test)]

use anyhow::Result;
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::TerminusDBModel;

// Model with ServerIDFor using lexical key
#[derive(Clone, Debug, Default, TerminusDBModel)]
#[tdb(key = "lexical", key_fields = "email", id_field = "id")]
pub struct LexicalUser {
    pub id: ServerIDFor<Self>,
    pub email: String,
    pub name: String,
}

// Model with ServerIDFor using value_hash key
#[derive(Clone, Debug, Default, TerminusDBModel)]
#[tdb(key = "value_hash", id_field = "id")]
pub struct HashDocument {
    pub id: ServerIDFor<Self>,
    pub content: String,
    pub version: i32,
}

// Model with ServerIDFor using random key (for comparison)
#[derive(Clone, Debug, Default, TerminusDBModel)]
#[tdb(key = "random", id_field = "id")]
pub struct RandomEntity {
    pub id: ServerIDFor<Self>,
    pub name: String,
    pub value: f64,
}

#[tokio::test]
async fn test_insert_and_retrieve_lexical_key() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_lexical", |client, spec| async move {
            // Insert schema (using single-element tuple)
            client
                .insert_schemas::<(LexicalUser,)>(spec.clone().into())
                .await?;

            // Create a user with no ID
            let user = LexicalUser {
                id: ServerIDFor::new(),
                email: "test@example.com".to_string(),
                name: "Test User".to_string(),
            };

            // Verify ID is None initially
            assert!(user.id.is_none());

            // Insert and retrieve the user
            let args = DocumentInsertArgs {
                message: "Insert user".to_string(),
                author: "test".to_string(),
                spec: spec.clone(),
                ..Default::default()
            };

            let (retrieved_user, commit_id) = client
                .insert_instance_and_retrieve(&user, args.clone())
                .await?;

            // Verify the ID was populated
            assert!(retrieved_user.id.is_some());
            assert_eq!(retrieved_user.email, user.email);
            assert_eq!(retrieved_user.name, user.name);

            // The ID should contain the email-based lexical key
            let id_str = retrieved_user.id.as_ref().unwrap().id();
            println!("Generated lexical ID: {}", id_str);

            // Verify commit ID format
            assert!(!commit_id.is_empty());

            // Retrieve the same user by ID to verify persistence
            let mut deserializer = deserialize::DefaultTDBDeserializer;
            let fetched: LexicalUser = client
                .get_instance(id_str, &spec, &mut deserializer)
                .await?;
            assert_eq!(fetched.email, user.email);
            assert_eq!(fetched.name, user.name);

            Ok(())
        })
        .await
}

#[tokio::test]
async fn test_insert_and_retrieve_value_hash_key() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_hash", |client, spec| async move {
            // Insert schema (using single-element tuple)
            client
                .insert_schemas::<(HashDocument,)>(spec.clone().into())
                .await?;

            // Create a document with no ID
            let doc = HashDocument {
                id: ServerIDFor::new(),
                content: "This is some content".to_string(),
                version: 1,
            };

            assert!(doc.id.is_none());

            let args = DocumentInsertArgs {
                message: "Insert document".to_string(),
                author: "test".to_string(),
                spec: spec.clone(),
                ..Default::default()
            };

            let (retrieved_doc, commit_id) =
                client.insert_instance_and_retrieve(&doc, args).await?;

            // Verify the ID was populated
            assert!(retrieved_doc.id.is_some());
            assert_eq!(retrieved_doc.content, doc.content);
            assert_eq!(retrieved_doc.version, doc.version);

            let id_str = retrieved_doc.id.as_ref().unwrap().id();
            println!("Generated hash ID: {}", id_str);

            // Verify commit ID
            assert!(!commit_id.is_empty());

            Ok(())
        })
        .await
}

#[tokio::test]
async fn test_insert_and_retrieve_multiple() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_multi_retrieve", |client, spec| async move {
            // Insert schema (using single-element tuple)
            client
                .insert_schemas::<(LexicalUser,)>(spec.clone().into())
                .await?;

            // Create multiple users
            let users = vec![
                LexicalUser {
                    id: ServerIDFor::new(),
                    email: "alice@example.com".to_string(),
                    name: "Alice".to_string(),
                },
                LexicalUser {
                    id: ServerIDFor::new(),
                    email: "bob@example.com".to_string(),
                    name: "Bob".to_string(),
                },
                LexicalUser {
                    id: ServerIDFor::new(),
                    email: "charlie@example.com".to_string(),
                    name: "Charlie".to_string(),
                },
            ];

            // Verify all IDs are None initially
            for user in &users {
                assert!(user.id.is_none());
            }

            let args = DocumentInsertArgs {
                message: "Insert multiple users".to_string(),
                author: "test".to_string(),
                spec: spec.clone(),
                ..Default::default()
            };

            let (retrieved_users, commit_id) = client
                .insert_instances_and_retrieve(users.clone(), args)
                .await?;

            // Verify we got the same number of users back
            assert_eq!(retrieved_users.len(), users.len());

            // Since TerminusDB doesn't guarantee order, we need to match by content
            // Create a map of original users by email for comparison
            let original_by_email: std::collections::HashMap<_, _> =
                users.iter().map(|u| (u.email.clone(), u)).collect();

            // Verify all IDs were populated and data matches
            for retrieved in &retrieved_users {
                assert!(retrieved.id.is_some());

                // Find the corresponding original user
                let original = original_by_email.get(&retrieved.email).ok_or_else(|| {
                    anyhow::anyhow!(
                        "Retrieved user with email {} not found in original list",
                        retrieved.email
                    )
                })?;

                assert_eq!(retrieved.email, original.email);
                assert_eq!(retrieved.name, original.name);

                println!(
                    "User {} has ID: {}",
                    retrieved.email,
                    retrieved.id.as_ref().unwrap().id()
                );
            }

            // Verify commit ID
            assert!(!commit_id.is_empty());

            Ok(())
        })
        .await
}

#[tokio::test]
async fn test_mixed_key_strategies() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_mixed_keys", |client, spec| async move {
            // Insert all schemas (using tuple approach)
            client
                .insert_schemas::<(LexicalUser, HashDocument, RandomEntity)>(spec.clone().into())
                .await?;

            let args = DocumentInsertArgs {
                message: "Insert mixed types".to_string(),
                author: "test".to_string(),
                spec: spec.clone(),
                ..Default::default()
            };

            // Test lexical key
            let user = LexicalUser {
                id: ServerIDFor::new(),
                email: "mixed@example.com".to_string(),
                name: "Mixed Test".to_string(),
            };
            let (retrieved_user, _) = client
                .insert_instance_and_retrieve(&user, args.clone())
                .await?;
            assert!(retrieved_user.id.is_some());

            // Test hash key
            let doc = HashDocument {
                id: ServerIDFor::new(),
                content: "Hash content".to_string(),
                version: 42,
            };
            let (retrieved_doc, _) = client
                .insert_instance_and_retrieve(&doc, args.clone())
                .await?;
            assert!(retrieved_doc.id.is_some());

            // Test random key - for random keys, we need to provide an ID
            let mut entity = RandomEntity {
                id: ServerIDFor::new(),
                name: "Random Entity".to_string(),
                value: 3.14,
            };
            // For random key, we need to generate an ID before insertion
            entity.id.__set_from_server(EntityIDFor::random());
            let (retrieved_entity, _) = client
                .insert_instance_and_retrieve(&entity, args.clone())
                .await?;
            assert!(retrieved_entity.id.is_some());

            println!("Lexical ID: {}", retrieved_user.id.as_ref().unwrap().id());
            println!("Hash ID: {}", retrieved_doc.id.as_ref().unwrap().id());
            println!("Random ID: {}", retrieved_entity.id.as_ref().unwrap().id());

            Ok(())
        })
        .await
}

#[tokio::test]
async fn test_insert_and_retrieve_error_cases() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_error_cases", |client, spec| async move {
            // Insert schema (using single-element tuple)
            client
                .insert_schemas::<(LexicalUser,)>(spec.clone().into())
                .await?;

            let args = DocumentInsertArgs {
                message: "Test error".to_string(),
                author: "test".to_string(),
                spec: spec.clone(),
                ..Default::default()
            };

            // Create and insert a user successfully first
            let user = LexicalUser {
                id: ServerIDFor::new(),
                email: "error@example.com".to_string(),
                name: "Error Test".to_string(),
            };
            let (_, _) = client
                .insert_instance_and_retrieve(&user, args.clone())
                .await?;

            // Try to insert and retrieve - insertion should work
            let new_user = LexicalUser {
                id: ServerIDFor::new(),
                email: "newuser@example.com".to_string(),
                name: "New User".to_string(),
            };

            // This should succeed because it's a new insert
            let result = client
                .insert_instance_and_retrieve(&new_user, args.clone())
                .await;
            assert!(result.is_ok());

            // Test with non-existent branch
            let bad_spec = BranchSpec {
                db: spec.db.clone(),
                branch: Some("non_existent_branch".to_string()),
                ref_commit: None,
            };
            let bad_args = DocumentInsertArgs {
                message: "Test error".to_string(),
                author: "test".to_string(),
                spec: bad_spec,
                ..Default::default()
            };

            let result = client.insert_instance_and_retrieve(&user, bad_args).await;
            assert!(result.is_err());
            if let Err(e) = result {
                println!("Expected error for non-existent branch: {}", e);
            }

            Ok(())
        })
        .await
}
