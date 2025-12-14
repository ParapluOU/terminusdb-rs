#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::{EntityIDFor, ToTDBInstance};
    use terminusdb_schema_derive::*;

    #[derive(Debug, Clone, TerminusDBModel, Serialize, Deserialize)]
    #[tdb(id_field = "id")]
    struct ModelWithCommitId {
        id: EntityIDFor<Self>,
        name: String,
        commit: String, // CommitId serializes to string
    }

    #[tokio::test]
    async fn test_commit_id_as_field() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_commit_id_field", |client, spec| async move {
                // Setup args
                let args = DocumentInsertArgs::from(spec.clone());

                // Insert schema
                client
                    .insert_entity_schema::<ModelWithCommitId>(args.clone())
                    .await?;

                // Create test instance
                let test_id = "test_commit_id_model_1";
                let test_commit = "abc123def456";
                let model = ModelWithCommitId {
                    id: EntityIDFor::new(test_id).unwrap(),
                    name: "Test Model".to_string(),
                    commit: test_commit.to_string(),
                };

                // Insert instance
                let _result = client.insert_instance(&model, args.clone()).await?;

                // Retrieve instance using the known ID
                let instance_id = format!("ModelWithCommitId/{}", test_id);
                let doc_json = client
                    .get_document(&instance_id, &spec, GetOpts::default())
                    .await?;
                let retrieved: ModelWithCommitId = serde_json::from_value(doc_json)?;

                // Verify
                assert_eq!(retrieved.name, "Test Model");
                assert_eq!(retrieved.commit, test_commit);

                Ok(())
            })
            .await
    }
}
