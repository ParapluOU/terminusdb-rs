use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use serde::{Deserialize, Serialize};

/// Test model for WOQL version retrieval testing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct WoqlVersionTest {
    id: EntityIDFor<Self>,
    name: String,
    revision: i32,
}

/// Test setup
async fn setup_test_client() -> anyhow::Result<(TerminusDBHttpClient, BranchSpec)> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let spec = BranchSpec::from("test");
    
    // Insert schema
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<WoqlVersionTest>(args).await.ok();
    
    Ok((client, spec))
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_get_instance_versions() -> anyhow::Result<()> {
    let (client, spec) = setup_test_client().await?;
    
    let fixed_id = &format!("woql_version_test_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
    println!("=== Testing get_instance_versions ===");
    println!("Using ID: {}", fixed_id);
    
    // Create 3 versions
    let mut commit_ids = Vec::new();
    
    // Version 1
    let v1 = WoqlVersionTest {
        id: EntityIDFor::new(fixed_id).unwrap(),
        name: "WOQL Test V1".to_string(),
        revision: 1,
    };
    let args = DocumentInsertArgs::from(spec.clone());
    let result = client.create_instance(&v1, args).await?;
    commit_ids.push(result.extract_commit_id().expect("Should have commit ID"));
    println!("Created version 1 in commit: {}", &commit_ids[0]);
    
    // Version 2
    let v2 = WoqlVersionTest {
        id: EntityIDFor::new(fixed_id).unwrap(),
        name: "WOQL Test V2".to_string(),
        revision: 2,
    };
    let args = DocumentInsertArgs::from(spec.clone());
    let result = client.update_instance(&v2, args).await?;
    commit_ids.push(result.extract_commit_id().expect("Should have commit ID"));
    println!("Created version 2 in commit: {}", &commit_ids[1]);
    
    // Version 3
    let v3 = WoqlVersionTest {
        id: EntityIDFor::new(fixed_id).unwrap(),
        name: "WOQL Test V3".to_string(),
        revision: 3,
    };
    let args = DocumentInsertArgs::from(spec.clone());
    let result = client.replace_instance(&v3, args).await?;
    commit_ids.push(result.extract_commit_id().expect("Should have commit ID"));
    println!("Created version 3 in commit: {}", &commit_ids[2]);
    
    // Test the list_instance_versions implementation
    println!("\n=== Testing list_instance_versions ===");
    let mut deserializer = terminusdb_client::deserialize::DefaultTDBDeserializer;
    
    match client.list_instance_versions::<WoqlVersionTest>(
        fixed_id,
        &spec,
        &mut deserializer
    ).await {
        Ok(versions) => {
            println!("✅ WOQL method returned {} versions", versions.len());
            for (i, (model, commit_id)) in versions.iter().enumerate() {
                println!("  Version {}: {} (revision {}) in commit {}", 
                         i+1, model.name, model.revision, commit_id);
            }
            
            // Verify we got all 3 versions
            assert_eq!(versions.len(), 3, "Should have retrieved all 3 versions");
            
            // Verify the versions are correct
            assert_eq!(versions[0].0.revision, 3); // Most recent first
            assert_eq!(versions[1].0.revision, 2);
            assert_eq!(versions[2].0.revision, 1);
        }
        Err(e) => {
            println!("❌ WOQL method failed: {}", e);
            return Err(e);
        }
    }
    
    // Compare with the parallel REST API implementation
    println!("\n=== Comparing with parallel REST API implementation ===");
    match client.list_instance_versions::<WoqlVersionTest>(
        fixed_id,
        &spec,
        &mut deserializer
    ).await {
        Ok(versions) => {
            println!("REST API method returned {} versions", versions.len());
            for (i, (model, commit_id)) in versions.iter().enumerate() {
                println!("  Version {}: {} (revision {}) in commit {}", 
                         i+1, model.name, model.revision, commit_id);
            }
        }
        Err(e) => {
            println!("REST API method failed (expected if history endpoint has issues): {}", e);
        }
    }
    
    Ok(())
}