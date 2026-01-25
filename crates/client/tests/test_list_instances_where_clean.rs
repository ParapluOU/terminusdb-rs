#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::{BranchSpec, DocumentInsertArgs, TerminusDBHttpClient};
    use terminusdb_schema::ToTDBInstance;
    use terminusdb_schema_derive::TerminusDBModel;

    #[derive(TerminusDBModel, Clone, Debug, PartialEq)]
    struct Product {
        name: String,
        price: i32,
        category: String,
        in_stock: bool,
    }

    async fn setup_test_data(
        client: &TerminusDBHttpClient,
        spec: &BranchSpec,
    ) -> anyhow::Result<()> {
        // Insert schema
        client
            .insert_schemas::<(Product,)>(DocumentInsertArgs::from(spec.clone()).as_schema())
            .await?;

        // Insert test data
        let products = vec![
            Product {
                name: "Laptop".to_string(),
                price: 1200,
                category: "Electronics".to_string(),
                in_stock: true,
            },
            Product {
                name: "Mouse".to_string(),
                price: 25,
                category: "Electronics".to_string(),
                in_stock: true,
            },
            Product {
                name: "Keyboard".to_string(),
                price: 75,
                category: "Electronics".to_string(),
                in_stock: false,
            },
            Product {
                name: "Desk".to_string(),
                price: 300,
                category: "Furniture".to_string(),
                in_stock: true,
            },
            Product {
                name: "Chair".to_string(),
                price: 200,
                category: "Furniture".to_string(),
                in_stock: false,
            },
        ];

        for product in &products {
            client
                .insert_instance(product, DocumentInsertArgs::from(spec.clone()))
                .await?;
        }

        Ok(())
    }

    // This test demonstrates the working list_instances_where functionality
    #[tokio::test]
    async fn test_list_instances_where_string_filters() {
        let server = TerminusDBServer::test_instance().await.unwrap();

        server
            .with_tmp_db("test_where_string", |client, spec| async move {
                setup_test_data(&client, &spec).await?;

                // Test 1: Filter by category (string)
                let electronics: Vec<Product> = client
                    .list_instances_where(&spec, None, None, vec![("category", "Electronics")])
                    .await?;

                assert_eq!(electronics.len(), 3, "Should have 3 electronics items");
                assert!(electronics.iter().all(|p| p.category == "Electronics"));

                // Test 2: Filter by different category
                let furniture: Vec<Product> = client
                    .list_instances_where(&spec, None, None, vec![("category", "Furniture")])
                    .await?;

                assert_eq!(furniture.len(), 2, "Should have 2 furniture items");
                assert!(furniture.iter().all(|p| p.category == "Furniture"));

                // Test 3: Empty filters returns all
                let all: Vec<Product> = client
                    .list_instances_where(&spec, None, None, Vec::<(&str, &str)>::new())
                    .await?;

                assert_eq!(all.len(), 5, "Should return all 5 products");

                // Test 4: With pagination
                let limited: Vec<Product> = client
                    .list_instances_where(
                        &spec,
                        None,    // offset
                        Some(2), // limit
                        vec![("category", "Electronics")],
                    )
                    .await?;

                assert_eq!(limited.len(), 2, "Should respect limit of 2");

                println!("All string filtering tests passed!");

                Ok(())
            })
            .await
            .unwrap();
    }

    // Test for count_instances_where functionality
    #[tokio::test]
    async fn test_count_instances_where() {
        let server = TerminusDBServer::test_instance().await.unwrap();

        server
            .with_tmp_db("test_count_where", |client, spec| async move {
                setup_test_data(&client, &spec).await?;

                // Test 1: Count by category
                let electronics_count = client
                    .count_instances_where::<Product, _, _, _>(
                        &spec,
                        vec![("category", "Electronics")],
                    )
                    .await?;
                assert_eq!(electronics_count, 3, "Should have 3 electronics items");

                // Test 2: Count by different category
                let furniture_count = client
                    .count_instances_where::<Product, _, _, _>(
                        &spec,
                        vec![("category", "Furniture")],
                    )
                    .await?;
                assert_eq!(furniture_count, 2, "Should have 2 furniture items");

                // Test 3: Count with boolean filter
                let in_stock_count = client
                    .count_instances_where::<Product, _, _, _>(&spec, vec![("in_stock", true)])
                    .await?;
                assert_eq!(in_stock_count, 3, "Should have 3 items in stock");

                // Test 4: Count out of stock items
                let out_of_stock_count = client
                    .count_instances_where::<Product, _, _, _>(&spec, vec![("in_stock", false)])
                    .await?;
                assert_eq!(out_of_stock_count, 2, "Should have 2 items out of stock");

                // Test 5: Empty filters counts all
                let all_count = client
                    .count_instances_where::<Product, _, _, _>(&spec, Vec::<(&str, &str)>::new())
                    .await?;
                assert_eq!(all_count, 5, "Should count all 5 products");

                // Test 6: Non-existent filter value
                let none_count = client
                    .count_instances_where::<Product, _, _, _>(&spec, vec![("category", "Books")])
                    .await?;
                assert_eq!(none_count, 0, "Should return 0 for non-existent category");

                println!("All count filtering tests passed!");

                Ok(())
            })
            .await
            .unwrap();
    }
}
