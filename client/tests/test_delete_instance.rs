use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use serde::{Deserialize, Serialize};

/// Test model for instance deletion
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct DeleteTestModel {
    id: EntityIDFor<Self>,
    name: String,
    value: i32,
}

/// Test setup
async fn setup_test_client() -> anyhow::Result<(TerminusDBHttpClient, BranchSpec)> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let spec = BranchSpec::from("test");
    
    // Insert schema
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<DeleteTestModel>(args).await.ok();
    
    Ok((client, spec))
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_delete_instance() -> anyhow::Result<()> {
    let (client, spec) = setup_test_client().await?;
    
    let test_id = &format!("delete_test_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
    
    println!("=== Testing delete_instance ==");
    println!("Test ID: {}", test_id);
    
    // Create an instance
    let instance = DeleteTestModel {
        id: EntityIDFor::new(test_id).unwrap(),
        name: "Test Item".to_string(),
        value: 42,
    };
    
    let args = DocumentInsertArgs::from(spec.clone());
    let result = client.create_instance(&instance, args.clone()).await?;
    println!("Created instance in commit: {:?}", result.extract_commit_id());
    
    // Verify it exists
    assert!(client.has_instance(&instance, &spec).await);
    println!("✓ Instance exists before deletion");
    
    // Delete the instance
    client.delete_instance(&instance, args.clone(), DeleteOpts::document_only()).await?;
    println!("✓ Delete operation completed");
    
    // Verify it no longer exists
    assert!(!client.has_instance(&instance, &spec).await);
    println!("✓ Instance no longer exists after deletion");
    
    Ok(())
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_delete_instance_by_id() -> anyhow::Result<()> {
    let (client, spec) = setup_test_client().await?;
    
    let test_id = &format!("delete_by_id_test_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
    
    println!("=== Testing delete_instance_by_id ===");
    println!("Test ID: {}", test_id);
    
    // Create an instance
    let instance = DeleteTestModel {
        id: EntityIDFor::new(test_id).unwrap(),
        name: "Test Item By ID".to_string(),
        value: 123,
    };
    
    let args = DocumentInsertArgs::from(spec.clone());
    let result = client.create_instance(&instance, args.clone()).await?;
    println!("Created instance in commit: {:?}", result.extract_commit_id());
    
    // Verify it exists
    assert!(client.has_instance(&instance, &spec).await);
    println!("✓ Instance exists before deletion");
    
    // Delete by ID
    client.delete_instance_by_id::<DeleteTestModel>(test_id, args.clone(), DeleteOpts::document_only()).await?;
    println!("✓ Delete by ID operation completed");
    
    // Verify it no longer exists
    assert!(!client.has_instance(&instance, &spec).await);
    println!("✓ Instance no longer exists after deletion by ID");
    
    Ok(())
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_delete_document_untyped() -> anyhow::Result<()> {
    let (client, spec) = setup_test_client().await?;
    
    let test_id = &format!("delete_doc_test_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
    
    println!("=== Testing delete_document (untyped) ===");
    println!("Test ID: {}", test_id);
    
    // Create an instance first
    let instance = DeleteTestModel {
        id: EntityIDFor::new(test_id).unwrap(),
        name: "Test Document".to_string(),
        value: 456,
    };
    
    let args = DocumentInsertArgs::from(spec.clone());
    let result = client.create_instance(&instance, args.clone()).await?;
    println!("Created instance in commit: {:?}", result.extract_commit_id());
    
    // Verify it exists
    assert!(client.has_instance(&instance, &spec).await);
    println!("✓ Instance exists before deletion");
    
    // Delete using untyped method
    let full_id = format!("DeleteTestModel/{}", test_id);
    client.delete_document(
        Some(&full_id),
        &spec,
        "test",
        "Delete test document",
        "instance",
        DeleteOpts::document_only(),
    ).await?;
    println!("✓ Untyped delete operation completed");
    
    // Verify it no longer exists
    assert!(!client.has_instance(&instance, &spec).await);
    println!("✓ Instance no longer exists after untyped deletion");
    
    Ok(())
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_delete_nonexistent_instance() -> anyhow::Result<()> {
    let (client, spec) = setup_test_client().await?;
    
    let test_id = &format!("nonexistent_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
    
    println!("=== Testing delete of nonexistent instance ===");
    println!("Test ID: {}", test_id);
    
    let args = DocumentInsertArgs::from(spec.clone());
    
    // Try to delete an instance that doesn't exist
    // This should not fail - TerminusDB typically handles this gracefully
    let result = client.delete_instance_by_id::<DeleteTestModel>(test_id, args, DeleteOpts::document_only()).await;
    
    match result {
        Ok(_) => println!("✓ Delete of nonexistent instance completed without error"),
        Err(e) => {
            println!("Delete of nonexistent instance failed: {}", e);
            // This might be expected behavior - check if the error is reasonable
            assert!(e.to_string().contains("not") || e.to_string().contains("exist") || e.to_string().contains("found"));
            println!("✓ Error message is reasonable for nonexistent instance");
        }
    }
    
    Ok(())
}

#[ignore] // Requires running TerminusDB instance  
#[tokio::test]
async fn test_delete_multiple_instances() -> anyhow::Result<()> {
    let (client, spec) = setup_test_client().await?;
    
    let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let test_ids = vec![
        format!("multi_delete_1_{}", timestamp),
        format!("multi_delete_2_{}", timestamp),
        format!("multi_delete_3_{}", timestamp),
    ];
    
    println!("=== Testing delete of multiple instances ===");
    println!("Test IDs: {:?}", test_ids);
    
    let args = DocumentInsertArgs::from(spec.clone());
    
    // Create multiple instances
    for (i, test_id) in test_ids.iter().enumerate() {
        let instance = DeleteTestModel {
            id: EntityIDFor::new(test_id).unwrap(),
            name: format!("Multi Test Item {}", i + 1),
            value: (i + 1) as i32 * 100,
        };
        
        let result = client.create_instance(&instance, args.clone()).await?;
        println!("Created instance {} in commit: {:?}", i + 1, result.extract_commit_id());
    }
    
    // Verify all exist
    for test_id in &test_ids {
        let instance = DeleteTestModel {
            id: EntityIDFor::new(test_id).unwrap(),
            name: "dummy".to_string(),
            value: 0,
        };
        assert!(client.has_instance(&instance, &spec).await);
    }
    println!("✓ All instances exist before deletion");
    
    // Delete each instance
    for (i, test_id) in test_ids.iter().enumerate() {
        client.delete_instance_by_id::<DeleteTestModel>(test_id, args.clone(), DeleteOpts::document_only()).await?;
        println!("✓ Deleted instance {}", i + 1);
    }
    
    // Verify none exist anymore
    for test_id in &test_ids {
        let instance = DeleteTestModel {
            id: EntityIDFor::new(test_id).unwrap(),
            name: "dummy".to_string(),
            value: 0,
        };
        assert!(!client.has_instance(&instance, &spec).await);
    }
    println!("✓ All instances deleted successfully");
    
    Ok(())
}