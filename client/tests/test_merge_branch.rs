//! Integration tests for `with_merge_branch` functionality.
//!
//! These tests require `terminusdb-bin` to be built.
//!
//! Run with:
//! ```bash
//! cargo test -p terminusdb-client test_merge_branch -- --ignored --nocapture
//! ```

use serde::{Deserialize, Serialize};
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

/// Test model for merge branch testing
#[derive(
    Debug, Clone, PartialEq, Serialize, Deserialize, Default, TerminusDBModel, FromTDBInstance,
)]
#[tdb(id_field = "id")]
struct TestPerson {
    id: EntityIDFor<Self>,
    name: String,
    age: i32,
}

/// Helper to create a unique database name
fn unique_db_name() -> String {
    format!("test_merge_{}", uuid::Uuid::new_v4().to_string().replace("-", "_"))
}

#[tokio::test]
#[ignore = "requires terminusdb-bin to be built"]
async fn test_with_merge_branch_success() -> anyhow::Result<()> {
    // Use shared server instance (prevents port conflicts when tests run in parallel)
    let server = TerminusDBServer::test_instance().await?;

    let client = server.client().await?;
    let db_name = unique_db_name();
    let client = client.ensure_database(&db_name).await?;
    let spec = BranchSpec::new(&db_name);

    // Insert schema
    let args = DocumentInsertArgs::from(spec.clone());
    client
        .insert_entity_schema::<TestPerson>(args.clone())
        .await?;

    // Insert initial person BEFORE merge branch
    let person1 = TestPerson {
        id: EntityIDFor::new("person_alice").unwrap(),
        name: "Alice".to_string(),
        age: 30,
    };
    client.save_instance(&person1, args.clone()).await?;

    // Verify initial count
    let initial_count = client.count_instances::<TestPerson>(&spec).await?;
    assert_eq!(initial_count, 1, "Should have 1 person initially");

    // Use with_merge_branch to add more data
    let result = client
        .with_merge_branch(
            &spec,
            MergeBranchOptions {
                squash: true,
                author: "test".into(),
                squash_message: Some("Squashed test changes".into()),
                merge_message: Some("Merged test branch".into()),
            },
            |branch_client| async move {
                let person2 = TestPerson {
                    id: EntityIDFor::new("person_bob").unwrap(),
                    name: "Bob".to_string(),
                    age: 25,
                };
                let args = DocumentInsertArgs::from(branch_client.spec_clone());
                branch_client.save_instance(&person2, args).await?;
                Ok("success")
            },
        )
        .await?;

    assert_eq!(result, "success");

    // Verify: both persons should exist on main branch
    let final_count = client.count_instances::<TestPerson>(&spec).await?;
    assert_eq!(final_count, 2, "Should have 2 persons after merge");

    // Cleanup
    client.delete_database(&db_name).await?;

    Ok(())
}

#[tokio::test]
#[ignore = "requires terminusdb-bin to be built"]
async fn test_with_merge_branch_error_rollback() -> anyhow::Result<()> {
    // Use shared server instance (prevents port conflicts when tests run in parallel)
    let server = TerminusDBServer::test_instance().await?;

    let client = server.client().await?;
    let db_name = unique_db_name();
    let client = client.ensure_database(&db_name).await?;
    let spec = BranchSpec::new(&db_name);

    // Insert schema
    let args = DocumentInsertArgs::from(spec.clone());
    client
        .insert_entity_schema::<TestPerson>(args.clone())
        .await?;

    // Insert initial person
    let person1 = TestPerson {
        id: EntityIDFor::new("person_alice").unwrap(),
        name: "Alice".to_string(),
        age: 30,
    };
    client.save_instance(&person1, args.clone()).await?;

    // Verify initial count
    let initial_count = client.count_instances::<TestPerson>(&spec).await?;
    assert_eq!(initial_count, 1, "Should have 1 person initially");

    // Use with_merge_branch but return an error
    let result: anyhow::Result<()> = client
        .with_merge_branch(
            &spec,
            MergeBranchOptions {
                squash: true,
                author: "test".into(),
                ..Default::default()
            },
            |branch_client| async move {
                eprintln!("[TEST] Inside closure, branch spec = {:?}", branch_client.spec());

                // Insert data on temp branch
                let person2 = TestPerson {
                    id: EntityIDFor::new("person_bob").unwrap(),
                    name: "Bob".to_string(),
                    age: 25,
                };
                let args = DocumentInsertArgs::from(branch_client.spec_clone());
                eprintln!("[TEST] About to insert Bob on branch...");
                branch_client.save_instance(&person2, args).await?;
                eprintln!("[TEST] Bob inserted, now returning error...");

                // Return an error - should trigger rollback
                anyhow::bail!("Intentional error to test rollback")
            },
        )
        .await;

    eprintln!("[TEST] with_merge_branch returned: {:?}", result.is_ok());

    // Should have failed
    assert!(result.is_err(), "with_merge_branch should return error");

    // Verify: only initial person should exist (no merge happened)
    let final_count = client.count_instances::<TestPerson>(&spec).await?;
    assert_eq!(
        final_count, 1,
        "Should still have only 1 person after failed merge"
    );

    // Cleanup
    client.delete_database(&db_name).await?;

    Ok(())
}

#[tokio::test]
#[ignore = "requires terminusdb-bin to be built"]
async fn test_with_merge_branch_without_squash() -> anyhow::Result<()> {
    // Use shared server instance (prevents port conflicts when tests run in parallel)
    let server = TerminusDBServer::test_instance().await?;

    let client = server.client().await?;
    let db_name = unique_db_name();
    let client = client.ensure_database(&db_name).await?;
    let spec = BranchSpec::new(&db_name);

    // Insert schema
    let args = DocumentInsertArgs::from(spec.clone());
    client
        .insert_entity_schema::<TestPerson>(args.clone())
        .await?;

    // Use with_merge_branch WITHOUT squash
    let result = client
        .with_merge_branch(
            &spec,
            MergeBranchOptions {
                squash: false, // No squashing
                author: "test".into(),
                merge_message: Some("Merged test branch".into()),
                ..Default::default()
            },
            |branch_client| async move {
                // Insert multiple people to create multiple commits
                for i in 0..3 {
                    let person = TestPerson {
                        id: EntityIDFor::new(&format!("person_{}", i)).unwrap(),
                        name: format!("Person {}", i),
                        age: 20 + i,
                    };
                    let args = DocumentInsertArgs::from(branch_client.spec_clone());
                    branch_client.save_instance(&person, args).await?;
                }
                Ok(3)
            },
        )
        .await?;

    assert_eq!(result, 3);

    // Verify: all persons should exist
    let final_count = client.count_instances::<TestPerson>(&spec).await?;
    assert_eq!(final_count, 3, "Should have 3 persons after merge");

    // Cleanup
    client.delete_database(&db_name).await?;

    Ok(())
}
