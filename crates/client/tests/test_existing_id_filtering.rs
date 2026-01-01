#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use anyhow::Result;
    use serde::{Deserialize, Serialize};
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::*;
    use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

    #[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
    #[tdb(id_field = "id")]
    pub struct SubfieldModel {
        pub id: EntityIDFor<Self>,
        pub name: String,
    }

    #[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
    #[tdb(id_field = "id")]
    pub struct MainModel {
        pub id: EntityIDFor<Self>,
        pub title: String,
        pub subfield: SubfieldModel,
    }

    #[tokio::test]
    async fn test_existing_id_filtering_on_post() -> Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_existing_filter", |client, spec| async move {
                // Insert schema for both models
                let args = DocumentInsertArgs::from(spec.clone());
                client
                    .insert_entity_schema::<SubfieldModel>(args.clone())
                    .await
                    .ok();
                client.insert_entity_schema::<MainModel>(args).await.ok();

                // Step 1: Create a subfield model
                let subfield = SubfieldModel {
                    id: EntityIDFor::new("test_sub_1").unwrap(),
                    name: "Test Subfield".to_string(),
                };

                // Step 2: Pre-create the subfield model using create_instance (which uses POST)
                let args = DocumentInsertArgs::from(spec.clone());
                let result = client.create_instance(&subfield, args).await?;
                println!("Pre-created subfield model: {:?}", result);

                // Step 3: Create a main model that reuses the subfield model
                let main_model = MainModel {
                    id: EntityIDFor::new("test_main_1").unwrap(),
                    title: "Test Main Model".to_string(),
                    subfield: subfield.clone(), // Reuse the same subfield
                };

                // Step 4: Try to save using create_instance (which uses POST internally) - this should filter out the existing subfield
                let args = DocumentInsertArgs::from(spec.clone());

                // This should work without errors because the existing subfield should be filtered out
                let result = client.create_instance(&main_model, args).await?;
                println!("Main model creation result: {:?}", result);

                // Verify the main model was created (result should indicate success)
                assert!(
                    matches!(result.root_result, TDBInsertInstanceResult::Inserted(_)),
                    "Expected root model to be inserted"
                );

                // Test another scenario: try to create the same main model again
                // This should fail because the model already exists
                let args = DocumentInsertArgs::from(spec.clone());

                let result2 = client.create_instance(&main_model, args).await;
                println!("Duplicate main model creation result: {:?}", result2);

                // Duplicate creation should fail
                assert!(
                    result2.is_err(),
                    "Expected duplicate create to fail, but it succeeded"
                );

                Ok(())
            })
            .await
    }

    #[tokio::test]
    async fn test_check_existing_ids() -> Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_check_existing", |client, spec| async move {
                // Generate unique IDs based on timestamp to avoid conflicts
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis();

                let existing_id = format!("existing_{}", timestamp);
                let not_existing_id_1 = format!("not_existing_1_{}", timestamp);
                let not_existing_id_2 = format!("not_existing_2_{}", timestamp);

                // Insert schema
                let args = DocumentInsertArgs::from(spec.clone());
                client
                    .insert_entity_schema::<SubfieldModel>(args.clone())
                    .await
                    .ok();

                // Insert a document with dynamic ID
                let subfield = SubfieldModel {
                    id: EntityIDFor::new(&existing_id).unwrap(),
                    name: "Existing Document".to_string(),
                };

                client.create_instance(&subfield, args).await?;

                // Test check_existing_ids with dynamic IDs
                let ids_to_check = vec![
                    format!("SubfieldModel/{}", existing_id),
                    format!("SubfieldModel/{}", not_existing_id_1),
                    format!("SubfieldModel/{}", not_existing_id_2),
                ];

                let existing_ids = client.check_existing_ids(&ids_to_check, &spec).await?;
                println!("Existing IDs: {:?}", existing_ids);

                // Should only find the existing one
                assert_eq!(existing_ids.len(), 1);
                assert!(existing_ids
                    .iter()
                    .any(|id| id.ends_with(&format!("SubfieldModel/{}", existing_id))));
                assert!(!existing_ids
                    .iter()
                    .any(|id| id.ends_with(&format!("SubfieldModel/{}", not_existing_id_1))));
                assert!(!existing_ids
                    .iter()
                    .any(|id| id.ends_with(&format!("SubfieldModel/{}", not_existing_id_2))));

                Ok(())
            })
            .await
    }
}
