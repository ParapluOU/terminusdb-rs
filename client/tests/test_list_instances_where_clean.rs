#[cfg(test)]
mod tests {
    use terminusdb_client::{BranchSpec, TerminusDBHttpClient, DocumentInsertArgs};
    use terminusdb_schema_derive::TerminusDBModel;
    use terminusdb_schema::ToTDBInstance;
    use serde::{Serialize, Deserialize};
    
    #[derive(TerminusDBModel, Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct Product {
        name: String,
        price: i32,
        category: String,
        in_stock: bool,
    }
    
    // This test demonstrates the working list_instances_where functionality
    #[ignore]
    #[tokio::test]
    async fn test_list_instances_where_string_filters() {
        let client = TerminusDBHttpClient::local_node().await;
        let test_db = format!("test_where_clean_{}", std::process::id());
        let spec = BranchSpec::new(&test_db);
        
        // Create test database
        let _ = client.delete_database(&test_db).await;
        client.reset_database(&test_db).await
            .expect("Failed to create test database");
        
        // Insert schema
        client.insert_schemas::<(Product,)>(DocumentInsertArgs::from(spec.clone()).as_schema())
            .await
            .expect("Failed to insert schema");
        
        // Insert test data
        let products = vec![
            Product { name: "Laptop".to_string(), price: 1200, category: "Electronics".to_string(), in_stock: true },
            Product { name: "Mouse".to_string(), price: 25, category: "Electronics".to_string(), in_stock: true },
            Product { name: "Keyboard".to_string(), price: 75, category: "Electronics".to_string(), in_stock: false },
            Product { name: "Desk".to_string(), price: 300, category: "Furniture".to_string(), in_stock: true },
            Product { name: "Chair".to_string(), price: 200, category: "Furniture".to_string(), in_stock: false },
        ];
        
        for product in &products {
            client.insert_instance(product, DocumentInsertArgs::from(spec.clone()))
                .await
                .expect("Failed to insert product");
        }
        
        // Test 1: Filter by category (string)
        let electronics: Vec<Product> = client.list_instances_where(
            &spec,
            None,
            None,
            vec![("category", "Electronics")],
        ).await.expect("Failed to filter by category");
        
        assert_eq!(electronics.len(), 3, "Should have 3 electronics items");
        assert!(electronics.iter().all(|p| p.category == "Electronics"));
        
        // Test 2: Filter by different category
        let furniture: Vec<Product> = client.list_instances_where(
            &spec,
            None,
            None,
            vec![("category", "Furniture")],
        ).await.expect("Failed to filter by category");
        
        assert_eq!(furniture.len(), 2, "Should have 2 furniture items");
        assert!(furniture.iter().all(|p| p.category == "Furniture"));
        
        // Test 3: Empty filters returns all
        let all: Vec<Product> = client.list_instances_where(
            &spec,
            None,
            None,
            Vec::<(&str, &str)>::new(),
        ).await.expect("Failed to query with empty filters");
        
        assert_eq!(all.len(), 5, "Should return all 5 products");
        
        // Test 4: With pagination
        let limited: Vec<Product> = client.list_instances_where(
            &spec,
            None,     // offset
            Some(2),  // limit
            vec![("category", "Electronics")],
        ).await.expect("Failed to query with limit");
        
        assert_eq!(limited.len(), 2, "Should respect limit of 2");
        
        // Clean up
        client.delete_database(&test_db).await
            .expect("Failed to delete test database");
        
        println!("All string filtering tests passed!");
    }
}