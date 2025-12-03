#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use anyhow::Result;
    use serde::{Deserialize, Serialize};
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::*;
    use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
    #[tdb(id_field = "id")]
    pub struct TestModel {
        pub id: EntityIDFor<Self>,
        pub name: String,
    }

    #[tokio::test]
    async fn test_woql_check_existing_clean() -> Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_woql_check", |client, spec| async move {
                // Insert schema
                let args = DocumentInsertArgs::from(spec.clone());
                client
                    .insert_entity_schema::<TestModel>(args.clone())
                    .await?;

                // Insert some documents
                let model1 = TestModel {
                    id: EntityIDFor::new("id1").unwrap(),
                    name: "Model 1".to_string(),
                };

                let model2 = TestModel {
                    id: EntityIDFor::new("id2").unwrap(),
                    name: "Model 2".to_string(),
                };

                // Create using insert_instance (uses PUT with create=true)
                client.insert_instance(&model1, args.clone()).await?;
                client.insert_instance(&model2, args.clone()).await?;

                // Now test check_existing_ids
                let ids_to_check = vec![
                    "TestModel/id1".to_string(),
                    "TestModel/id2".to_string(),
                    "TestModel/id3".to_string(), // doesn't exist
                    "TestModel/id4".to_string(), // doesn't exist
                ];

                println!("\nChecking IDs: {:?}", ids_to_check);
                let existing_ids = client.check_existing_ids(&ids_to_check, &spec).await?;
                println!("Found existing IDs: {:?}", existing_ids);

                // Verify results
                assert_eq!(existing_ids.len(), 2);
                assert!(existing_ids.contains("TestModel/id1"));
                assert!(existing_ids.contains("TestModel/id2"));
                assert!(!existing_ids.contains("TestModel/id3"));
                assert!(!existing_ids.contains("TestModel/id4"));

                println!("\n✅ WOQL check_existing_ids works correctly!");

                // Test with create_instance (uses POST)
                let model3 = TestModel {
                    id: EntityIDFor::new("id3").unwrap(),
                    name: "Model 3".to_string(),
                };

                // This should succeed since id3 doesn't exist
                let result = client.create_instance(&model3, args.clone()).await?;
                println!("\nCreated new model with id3: {:?}", result);

                // Try to create a model with existing ID using create_instance
                let duplicate = TestModel {
                    id: EntityIDFor::new("id1").unwrap(), // This ID already exists!
                    name: "Duplicate Model".to_string(),
                };

                println!("\nTrying to create duplicate with existing id1...");
                match client.create_instance(&duplicate, args).await {
                    Ok(_) => panic!("Should have failed or been filtered!"),
                    Err(e) => {
                        println!("Got expected error: {:?}", e);
                        // With filtering, we might get a different error or empty result
                    }
                }

                Ok(())
            })
            .await
    }
}
