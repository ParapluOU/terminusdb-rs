#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::*;
    use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
    use terminusdb_woql_builder::prelude::*;

    /// Test model for query count testing
    #[derive(Debug, Clone, PartialEq, Default, TerminusDBModel, FromTDBInstance)]
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

    /// Insert test data into the database
    async fn insert_test_data(
        client: &TerminusDBHttpClient,
        spec: &BranchSpec,
        prefix: &str,
    ) -> anyhow::Result<()> {
        let test_data = vec![
            CountTestModel {
                id: EntityIDFor::new(&format!("{}_001", prefix)).unwrap(),
                name: "Item 1".to_string(),
                category: "A".to_string(),
                value: 10,
            },
            CountTestModel {
                id: EntityIDFor::new(&format!("{}_002", prefix)).unwrap(),
                name: "Item 2".to_string(),
                category: "B".to_string(),
                value: 20,
            },
            CountTestModel {
                id: EntityIDFor::new(&format!("{}_003", prefix)).unwrap(),
                name: "Item 3".to_string(),
                category: "A".to_string(),
                value: 30,
            },
            CountTestModel {
                id: EntityIDFor::new(&format!("{}_004", prefix)).unwrap(),
                name: "Item 4".to_string(),
                category: "A".to_string(),
                value: 40,
            },
            CountTestModel {
                id: EntityIDFor::new(&format!("{}_005", prefix)).unwrap(),
                name: "Item 5".to_string(),
                category: "B".to_string(),
                value: 50,
            },
        ];

        for model in test_data {
            let args = DocumentInsertArgs::from(spec.clone());
            client.save_instance(&model, args).await?;
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_query_instances_count_all() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_count_all", |client, spec| async move {
                // Insert schema
                let args = DocumentInsertArgs::from(spec.clone());
                client
                    .insert_entity_schema::<CountTestModel>(args.clone())
                    .await?;

                // Insert test data
                insert_test_data(&client, &spec, "count_all").await?;

                // Count all instances using ListModels
                let query = ListModels::<CountTestModel>::default();
                let count = client.query_instances_count(&spec, query).await?;

                assert_eq!(count, 5, "Should count all 5 instances");

                Ok(())
            })
            .await
    }

    #[tokio::test]
    async fn test_query_instances_count_filtered() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_count_filtered", |client, spec| async move {
                // Insert schema
                let args = DocumentInsertArgs::from(spec.clone());
                client
                    .insert_entity_schema::<CountTestModel>(args.clone())
                    .await?;

                // Insert test data
                insert_test_data(&client, &spec, "filtered").await?;

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
            })
            .await
    }

    #[tokio::test]
    async fn test_query_instances_count_empty_database() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_count_empty", |client, spec| async move {
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
            })
            .await
    }

    #[tokio::test]
    async fn test_query_instances_count_vs_apply() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_count_vs_apply", |client, spec| async move {
                // Insert schema
                let args = DocumentInsertArgs::from(spec.clone());
                client
                    .insert_entity_schema::<CountTestModel>(args.clone())
                    .await?;

                // Insert test data
                insert_test_data(&client, &spec, "vs_apply").await?;

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
            })
            .await
    }
}
