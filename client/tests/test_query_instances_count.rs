use serde::{Deserialize, Serialize};
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use terminusdb_woql_builder::prelude::*;

/// Test model for query count testing
#[derive(
    Debug, Clone, PartialEq, Serialize, Deserialize, Default, TerminusDBModel, FromTDBInstance,
)]
#[tdb(id_field = "id")]
struct CountTestModel {
    id: EntityIDFor<Self>,
    name: String,
    category: String,
    value: i32,
}

/// Custom query that filters by category
struct FilterByCategoryQuery {
    category: String,
}

impl InstanceQueryable for FilterByCategoryQuery {
    type Model = CountTestModel;

    fn build(&self, subject: Var, builder: WoqlBuilder) -> WoqlBuilder {
        builder.triple(
            subject.clone(),
            "@schema:category",
            string_literal(&self.category),
        )
    }
}

/// Test setup
async fn setup_test_data() -> anyhow::Result<(TerminusDBHttpClient, BranchSpec)> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let spec = BranchSpec::from("test_count");

    // Delete database if it exists
    client.delete_database(&spec.db).await.ok();

    // Create new database
    client.reset_database(&spec.db).await?;

    // Insert schema
    let args = DocumentInsertArgs::from(spec.clone());
    client
        .insert_entity_schema::<CountTestModel>(args.clone())
        .await?;

    // Insert test data
    let test_data = vec![
        CountTestModel {
            id: EntityIDFor::new("CountTestModel/count_test_001").unwrap(),
            name: "Item 1".to_string(),
            category: "A".to_string(),
            value: 10,
        },
        CountTestModel {
            id: EntityIDFor::new("CountTestModel/count_test_002").unwrap(),
            name: "Item 2".to_string(),
            category: "B".to_string(),
            value: 20,
        },
        CountTestModel {
            id: EntityIDFor::new("CountTestModel/count_test_003").unwrap(),
            name: "Item 3".to_string(),
            category: "A".to_string(),
            value: 30,
        },
        CountTestModel {
            id: EntityIDFor::new("CountTestModel/count_test_004").unwrap(),
            name: "Item 4".to_string(),
            category: "A".to_string(),
            value: 40,
        },
        CountTestModel {
            id: EntityIDFor::new("CountTestModel/count_test_005").unwrap(),
            name: "Item 5".to_string(),
            category: "B".to_string(),
            value: 50,
        },
    ];

    for model in test_data {
        let args = DocumentInsertArgs::from(spec.clone());
        client.save_instance(&model, args).await?;
    }

    Ok((client, spec))
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_query_instances_count_all() -> anyhow::Result<()> {
    let (client, spec) = setup_test_data().await?;

    // Count all instances using ListModels
    let query = ListModels::<CountTestModel>::default();
    let count = client.query_instances_count(&spec, query).await?;

    assert_eq!(count, 5, "Should count all 5 instances");

    Ok(())
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_query_instances_count_filtered() -> anyhow::Result<()> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let spec = BranchSpec::from("test_count_filtered");

    // Delete database if it exists
    client.delete_database(&spec.db).await.ok();

    // Create new database
    client.reset_database(&spec.db).await?;

    // Insert schema
    let args = DocumentInsertArgs::from(spec.clone());
    client
        .insert_entity_schema::<CountTestModel>(args.clone())
        .await?;

    // Insert test data
    let test_data = vec![
        CountTestModel {
            id: EntityIDFor::new("CountTestModel/filtered_test_001").unwrap(),
            name: "Item 1".to_string(),
            category: "A".to_string(),
            value: 10,
        },
        CountTestModel {
            id: EntityIDFor::new("CountTestModel/filtered_test_002").unwrap(),
            name: "Item 2".to_string(),
            category: "B".to_string(),
            value: 20,
        },
        CountTestModel {
            id: EntityIDFor::new("CountTestModel/filtered_test_003").unwrap(),
            name: "Item 3".to_string(),
            category: "A".to_string(),
            value: 30,
        },
        CountTestModel {
            id: EntityIDFor::new("CountTestModel/filtered_test_004").unwrap(),
            name: "Item 4".to_string(),
            category: "A".to_string(),
            value: 40,
        },
        CountTestModel {
            id: EntityIDFor::new("CountTestModel/filtered_test_005").unwrap(),
            name: "Item 5".to_string(),
            category: "B".to_string(),
            value: 50,
        },
    ];

    for model in test_data {
        let args = DocumentInsertArgs::from(spec.clone());
        client.save_instance(&model, args).await?;
    }

    // Count instances with category "A"
    let query_a = FilterByCategoryQuery {
        category: "A".to_string(),
    };
    let count_a = client.query_instances_count(&spec, query_a).await?;
    assert_eq!(count_a, 3, "Should count 3 instances with category A");

    // Count instances with category "B"
    let query_b = FilterByCategoryQuery {
        category: "B".to_string(),
    };
    let count_b = client.query_instances_count(&spec, query_b).await?;
    assert_eq!(count_b, 2, "Should count 2 instances with category B");

    // Count instances with non-existent category
    let query_c = FilterByCategoryQuery {
        category: "C".to_string(),
    };
    let count_c = client.query_instances_count(&spec, query_c).await?;
    assert_eq!(count_c, 0, "Should count 0 instances with category C");

    Ok(())
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_query_instances_count_empty_database() -> anyhow::Result<()> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let spec = BranchSpec::from("test_count_empty");

    // Delete database if it exists
    client.delete_database(&spec.db).await.ok();

    // Create new database
    client.reset_database(&spec.db).await?;

    // Insert schema only, no data
    let args = DocumentInsertArgs::from(spec.clone());
    client
        .insert_entity_schema::<CountTestModel>(args.clone())
        .await?;

    // Count should be 0
    let query = ListModels::<CountTestModel>::default();
    let count = client.query_instances_count(&spec, query).await?;

    assert_eq!(count, 0, "Should count 0 instances in empty database");

    Ok(())
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_query_instances_count_vs_apply() -> anyhow::Result<()> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let spec = BranchSpec::from("test_count_vs_apply");

    // Delete database if it exists
    client.delete_database(&spec.db).await.ok();

    // Create new database
    client.reset_database(&spec.db).await?;

    // Insert schema
    let args = DocumentInsertArgs::from(spec.clone());
    client
        .insert_entity_schema::<CountTestModel>(args.clone())
        .await?;

    // Insert test data
    let test_data = vec![
        CountTestModel {
            id: EntityIDFor::new("CountTestModel/vs_apply_test_001").unwrap(),
            name: "Item 1".to_string(),
            category: "A".to_string(),
            value: 10,
        },
        CountTestModel {
            id: EntityIDFor::new("CountTestModel/vs_apply_test_002").unwrap(),
            name: "Item 2".to_string(),
            category: "B".to_string(),
            value: 20,
        },
        CountTestModel {
            id: EntityIDFor::new("CountTestModel/vs_apply_test_003").unwrap(),
            name: "Item 3".to_string(),
            category: "A".to_string(),
            value: 30,
        },
        CountTestModel {
            id: EntityIDFor::new("CountTestModel/vs_apply_test_004").unwrap(),
            name: "Item 4".to_string(),
            category: "A".to_string(),
            value: 40,
        },
        CountTestModel {
            id: EntityIDFor::new("CountTestModel/vs_apply_test_005").unwrap(),
            name: "Item 5".to_string(),
            category: "B".to_string(),
            value: 50,
        },
    ];

    for model in test_data {
        let args = DocumentInsertArgs::from(spec.clone());
        client.save_instance(&model, args).await?;
    }

    // Test that count matches the length of apply results
    let query = ListModels::<CountTestModel>::default();

    // Get count
    let count = client.query_instances_count(&spec, query).await?;

    // Get actual instances
    let query2 = ListModels::<CountTestModel>::default();
    let instances = client.query_instances(&spec, None, None, query2).await?;

    assert_eq!(
        count,
        instances.len(),
        "Count should match the number of instances returned by apply"
    );

    // Test with filtered query
    let filtered_query = FilterByCategoryQuery {
        category: "A".to_string(),
    };
    let filtered_count = client.query_instances_count(&spec, filtered_query).await?;

    let filtered_query2 = FilterByCategoryQuery {
        category: "A".to_string(),
    };
    let filtered_instances = client
        .query_instances(&spec, None, None, filtered_query2)
        .await?;

    assert_eq!(
        filtered_count,
        filtered_instances.len(),
        "Filtered count should match the number of filtered instances"
    );

    Ok(())
}
