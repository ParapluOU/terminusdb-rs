#[cfg(test)]
mod tests {
    use terminusdb_client::{BranchSpec, TerminusDBHttpClient, DocumentInsertArgs};
    use terminusdb_schema_derive::TerminusDBModel;
    use terminusdb_schema::ToTDBInstance;
    use serde::{Serialize, Deserialize};
    
    #[derive(TerminusDBModel, Clone, Debug, Serialize, Deserialize, PartialEq)]
    struct Person {
        name: String,
        age: i32,
    }
    
    // Test that demonstrates integer filtering works correctly after the fix
    #[ignore]
    #[tokio::test]
    async fn test_filter_by_integer() {
        let client = TerminusDBHttpClient::local_node().await;
        let test_db = format!("test_int_filter_{}", std::process::id());
        let spec = BranchSpec::new(&test_db);
        
        // Setup database
        let _ = client.delete_database(&test_db).await;
        client.reset_database(&test_db).await.unwrap();
        client.insert_schemas::<(Person,)>(DocumentInsertArgs::from(spec.clone()).as_schema())
            .await.unwrap();
        
        // Insert test data
        let people = vec![
            Person { name: "Alice".to_string(), age: 25 },
            Person { name: "Bob".to_string(), age: 30 },
            Person { name: "Charlie".to_string(), age: 25 },
            Person { name: "David".to_string(), age: 35 },
        ];
        
        for person in &people {
            client.insert_instance(person, DocumentInsertArgs::from(spec.clone()))
                .await.unwrap();
        }
        
        // Test: Filter by age (integer)
        let age_25: Vec<Person> = client.list_instances_where(
            &spec,
            None,  // offset
            None,  // limit
            vec![("age", 25)],  // filters
        ).await.unwrap();
        
        assert_eq!(age_25.len(), 2, "Should find 2 people aged 25");
        assert!(age_25.iter().all(|p| p.age == 25));
        assert!(age_25.iter().any(|p| p.name == "Alice"));
        assert!(age_25.iter().any(|p| p.name == "Charlie"));
        
        // Test: Filter by different age
        let age_30: Vec<Person> = client.list_instances_where(
            &spec,
            None,  // offset
            None,  // limit
            vec![("age", 30)],
        ).await.unwrap();
        
        assert_eq!(age_30.len(), 1, "Should find 1 person aged 30");
        assert_eq!(age_30[0].name, "Bob");
        
        // Test: Multiple filters
        let alice_25: Vec<Person> = client.list_instances_where(
            &spec,
            None,  // offset
            None,  // limit
            vec![
                ("name", "Alice"),
                ("age", 25),
            ],
        ).await.unwrap();
        
        assert_eq!(alice_25.len(), 1, "Should find exactly Alice");
        assert_eq!(alice_25[0].name, "Alice");
        assert_eq!(alice_25[0].age, 25);
        
        // Clean up
        client.delete_database(&test_db).await.unwrap();
    }
}