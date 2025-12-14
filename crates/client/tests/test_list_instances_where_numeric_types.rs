#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::{BranchSpec, DocumentInsertArgs, TerminusDBHttpClient};
    use terminusdb_schema::ToTDBInstance;
    use terminusdb_schema_derive::TerminusDBModel;

    #[derive(TerminusDBModel, Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct TestItem {
        name: String,
        count: i32,
        price: f64,
        active: bool,
    }

    // Test to isolate the issue with different data types
    #[tokio::test]
    async fn test_filter_by_different_types() {
        let server = TerminusDBServer::test_instance().await.unwrap();

        server
            .with_tmp_db("test_filter_types", |client, spec| async move {
                // Insert schema
                client
                    .insert_schemas::<(TestItem,)>(DocumentInsertArgs::from(spec.clone()).as_schema())
                    .await
                    .unwrap();

                // Insert test data
                let items = vec![
                    TestItem {
                        name: "Item1".to_string(),
                        count: 10,
                        price: 99.99,
                        active: true,
                    },
                    TestItem {
                        name: "Item2".to_string(),
                        count: 20,
                        price: 49.99,
                        active: false,
                    },
                    TestItem {
                        name: "Item3".to_string(),
                        count: 10,
                        price: 99.99,
                        active: true,
                    },
                    TestItem {
                        name: "Item4".to_string(),
                        count: 30,
                        price: 19.99,
                        active: false,
                    },
                ];

                for item in &items {
                    client
                        .insert_instance(item, DocumentInsertArgs::from(spec.clone()))
                        .await
                        .unwrap();
                }

                println!("\n=== Testing different filter types ===");

                // Test 1: String filtering (baseline - we know this works)
                let by_name: Vec<TestItem> = client
                    .list_instances_where(&spec, None, None, vec![("name", "Item1")])
                    .await
                    .unwrap();
                println!("Filter by string (name='Item1'): {} results", by_name.len());
                assert_eq!(by_name.len(), 1, "String filtering should work");

                // Test 2: Integer filtering
                let by_count: Vec<TestItem> = client
                    .list_instances_where(&spec, None, None, vec![("count", 10)])
                    .await
                    .unwrap();
                println!("Filter by integer (count=10): {} results", by_count.len());
                println!("Expected 2 items with count=10, got: {}", by_count.len());
                if by_count.is_empty() {
                    println!("  ❌ Integer filtering NOT WORKING");
                }

                // Test 3: Boolean filtering
                let by_active: Vec<TestItem> = client
                    .list_instances_where(&spec, None, None, vec![("active", true)])
                    .await
                    .unwrap();
                println!(
                    "Filter by boolean (active=true): {} results",
                    by_active.len()
                );
                println!("Expected 2 items with active=true, got: {}", by_active.len());
                if by_active.is_empty() {
                    println!("  ❌ Boolean filtering NOT WORKING");
                }

                // Test 4: Float filtering
                let by_price: Vec<TestItem> = client
                    .list_instances_where(&spec, None, None, vec![("price", 99.99)])
                    .await
                    .unwrap();
                println!(
                    "Filter by float (price=99.99): {} results",
                    by_price.len()
                );
                println!("Expected 2 items with price=99.99, got: {}", by_price.len());
                if by_price.is_empty() {
                    println!("  ❌ Float filtering NOT WORKING");
                }

                // Let's check what the data looks like when retrieved without filters
                println!("\n=== All items in database ===");
                let all_items: Vec<TestItem> = client
                    .list_instances_where(&spec, None, None, Vec::<(&str, &str)>::new())
                    .await
                    .unwrap();

                for item in &all_items {
                    println!("  {:?}", item);
                }

                // Report results
                println!("\n=== Summary ===");
                println!("String filtering: ✓ WORKS");
                println!(
                    "Integer filtering: {} WORKS",
                    if by_count.is_empty() {
                        "✗ DOES NOT"
                    } else {
                        "✓"
                    }
                );
                println!(
                    "Boolean filtering: {} WORKS",
                    if by_active.is_empty() {
                        "✗ DOES NOT"
                    } else {
                        "✓"
                    }
                );
                println!(
                    "Float filtering: {} WORKS",
                    if by_price.is_empty() {
                        "✗ DOES NOT"
                    } else {
                        "✓"
                    }
                );

                Ok(())
            })
            .await
            .unwrap();
    }

    // Test to debug the WOQL query generation
    #[tokio::test]
    async fn test_debug_woql_queries() {
        use terminusdb_woql_builder::prelude::*;

        let server = TerminusDBServer::test_instance().await.unwrap();

        server
            .with_tmp_db("test_woql_debug", |client, spec| async move {
                // Insert schema
                client
                    .insert_schemas::<(TestItem,)>(DocumentInsertArgs::from(spec.clone()).as_schema())
                    .await
                    .unwrap();

                // Insert one test item
                let item = TestItem {
                    name: "Test".to_string(),
                    count: 42,
                    price: 99.99,
                    active: true,
                };
                client
                    .insert_instance(&item, DocumentInsertArgs::from(spec.clone()))
                    .await
                    .unwrap();

                println!("\n=== Testing raw WOQL queries ===");

                // Test different ways to query integer fields
                let queries = vec![
                    (
                        "Integer as i64",
                        WoqlBuilder::new()
                            .triple(vars!("Item"), "rdf:type", "@schema:TestItem")
                            .triple(vars!("Item"), "count", WoqlInput::Integer(42))
                            .read_document(vars!("Item"), vars!("Doc"))
                            .select(vec![vars!("Doc")])
                            .finalize(),
                    ),
                    (
                        "Integer as string",
                        WoqlBuilder::new()
                            .triple(vars!("Item"), "rdf:type", "@schema:TestItem")
                            .triple(vars!("Item"), "count", "42")
                            .read_document(vars!("Item"), vars!("Doc"))
                            .select(vec![vars!("Doc")])
                            .finalize(),
                    ),
                    (
                        "Integer as decimal",
                        WoqlBuilder::new()
                            .triple(vars!("Item"), "rdf:type", "@schema:TestItem")
                            .triple(vars!("Item"), "count", WoqlInput::Decimal("42".to_string()))
                            .read_document(vars!("Item"), vars!("Doc"))
                            .select(vec![vars!("Doc")])
                            .finalize(),
                    ),
                    (
                        "Boolean as bool",
                        WoqlBuilder::new()
                            .triple(vars!("Item"), "rdf:type", "@schema:TestItem")
                            .triple(vars!("Item"), "active", WoqlInput::Boolean(true))
                            .read_document(vars!("Item"), vars!("Doc"))
                            .select(vec![vars!("Doc")])
                            .finalize(),
                    ),
                    (
                        "Boolean as string",
                        WoqlBuilder::new()
                            .triple(vars!("Item"), "rdf:type", "@schema:TestItem")
                            .triple(vars!("Item"), "active", "true")
                            .read_document(vars!("Item"), vars!("Doc"))
                            .select(vec![vars!("Doc")])
                            .finalize(),
                    ),
                ];

                for (desc, query) in queries {
                    let result = client
                        .query::<serde_json::Value>(spec.clone().into(), query)
                        .await
                        .unwrap();
                    println!("{}: {} results", desc, result.bindings.len());
                    if !result.bindings.is_empty() {
                        println!("  ✓ This representation works!");
                    }
                }

                // Let's also check what the actual stored values look like
                println!("\n=== Raw triple data ===");
                let raw_triples = WoqlBuilder::new()
                    .triple(vars!("Subject"), vars!("Predicate"), vars!("Object"))
                    .triple(vars!("Subject"), "rdf:type", "@schema:TestItem")
                    .select(vec![vars!("Subject"), vars!("Predicate"), vars!("Object")])
                    .finalize();

                let triples = client
                    .query::<serde_json::Value>(spec.clone().into(), raw_triples)
                    .await
                    .unwrap();
                for binding in &triples.bindings {
                    if let (Some(pred), Some(obj)) =
                        (binding.get("Predicate"), binding.get("Object"))
                    {
                        println!("  {} -> {}", pred, obj);
                    }
                }

                Ok(())
            })
            .await
            .unwrap();
    }
}
