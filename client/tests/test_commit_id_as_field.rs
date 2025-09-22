use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::*;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, TerminusDBModel, Serialize, Deserialize)]
struct ModelWithCommitId {
    id: String,
    name: String,
    commit: CommitId,
}

#[tokio::test]
#[ignore] // Requires running TerminusDB instance
async fn test_commit_id_as_field() -> anyhow::Result<()> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let db_name = "test_commit_id_field";
    
    // Reset database to ensure clean state
    let _ = client.reset_database(db_name).await;
    
    // Setup args
    let spec = BranchSpec::from(db_name);
    let args = DocumentInsertArgs::from(spec.clone());
    
    // Insert schema
    client.insert_entity_schema::<ModelWithCommitId>(args.clone()).await?;
    
    // Create test instance
    let test_id = "test_commit_id_model_1";
    let test_commit = CommitId::new("abc123def456");
    let model = ModelWithCommitId {
        id: test_id.to_string(),
        name: "Test Model".to_string(),
        commit: test_commit.clone(),
    };
    
    // Insert instance
    let _result = client.insert_instance(&model, args.clone()).await?;
    
    // Retrieve instance using the known ID
    let instance_id = format!("ModelWithCommitId/{}", test_id);
    let doc_json = client.get_document(&instance_id, &spec, GetOpts::default()).await?;
    let retrieved: ModelWithCommitId = serde_json::from_value(doc_json)?;
    
    // Verify
    assert_eq!(retrieved.name, "Test Model");
    assert_eq!(retrieved.commit, test_commit);
    
    // Clean up
    let _ = client.delete_database(db_name).await;
    
    Ok(())
}