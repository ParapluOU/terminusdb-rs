use anyhow::Result;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
pub struct SubModel {
    pub id: EntityIDFor<Self>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
pub struct MainModel {
    pub id: EntityIDFor<Self>,
    pub title: String,
    pub sub: SubModel,
}

#[tokio::test]
#[ignore] // Requires running TerminusDB instance
async fn test_post_filtering_behavior() -> Result<()> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let spec = BranchSpec::from("test_post_filter");
    
    // Reset database
    client.reset_database(&spec.db).await?;
    
    // Insert schemas
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<SubModel>(args.clone()).await?;
    client.insert_entity_schema::<MainModel>(args.clone()).await?;
    
    println!("\n=== Test 1: Create sub-model first ===");
    
    // Create a sub-model
    let sub = SubModel {
        id: EntityIDFor::new("sub1").unwrap(),
        name: "First Sub".to_string(),
    };
    
    let result = client.create_instance(&sub, args.clone()).await?;
    println!("Created sub-model: {:?}", result.root_id);
    
    println!("\n=== Test 2: Create main model referencing existing sub ===");
    
    // Create main model that references the existing sub
    // When using POST, the sub will be filtered out since it exists
    let main1 = MainModel {
        id: EntityIDFor::new("main1").unwrap(),
        title: "First Main".to_string(),
        sub: sub.clone(),
    };
    
    // This will work - the main model gets created, sub is filtered
    let result = client.create_instance(&main1, args.clone()).await?;
    println!("Created main model: {:?}", result.root_id);
    println!("Sub-entities in result: {:?}", result.sub_entities.keys().collect::<Vec<_>>());
    
    println!("\n=== Test 3: Try to create duplicate main model ===");
    
    // Try to create the same main model again
    // Everything will be filtered out
    match client.create_instance(&main1, args.clone()).await {
        Ok(_) => panic!("Should have failed - all documents filtered"),
        Err(e) => {
            println!("Got expected error: {}", e);
            assert!(e.to_string().contains("Could not find root instance ID"));
        }
    }
    
    println!("\n=== Test 4: Create new main with new sub ===");
    
    // Create a completely new main model with a new sub
    let sub2 = SubModel {
        id: EntityIDFor::new("sub2").unwrap(),
        name: "Second Sub".to_string(),
    };
    
    let main2 = MainModel {
        id: EntityIDFor::new("main2").unwrap(),
        title: "Second Main".to_string(),
        sub: sub2,
    };
    
    let result = client.create_instance(&main2, args.clone()).await?;
    println!("Created second main model: {:?}", result.root_id);
    println!("Sub-entities created: {:?}", result.sub_entities);
    
    println!("\n=== Test 5: Alternative - use insert_instance (PUT) ===");
    
    // For idempotent operations, use insert_instance which uses PUT
    let main3 = MainModel {
        id: EntityIDFor::new("main3").unwrap(),
        title: "Third Main".to_string(),
        sub: sub.clone(), // Reuse first sub
    };
    
    // This always works, even if documents exist
    let result = client.insert_instance(&main3, args.clone()).await?;
    println!("Inserted main model with PUT: {:?}", result.root_id);
    
    // Can call it again without error
    let result = client.insert_instance(&main3, args).await?;
    println!("Re-inserted same model with PUT: {:?}", result.root_id);
    
    println!("\n✅ All tests passed - POST filtering working as expected!");
    
    Ok(())
}