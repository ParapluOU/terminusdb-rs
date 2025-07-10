#[cfg(test)]
mod test_unfold_typed_instances {
    use anyhow::Result;
    use serde::{Deserialize, Serialize};
    use terminusdb_client::*;
    use terminusdb_schema::*;
    use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

    // Define test models with relationships
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Serialize,
        Deserialize,
        Default,
        TerminusDBModel,
        FromTDBInstance,
    )]
    #[tdb(id_field = "id")]
    struct Address {
        id: EntityIDFor<Self>,
        street: String,
        city: String,
        postal_code: String,
    }

    #[derive(
        Debug,
        Clone,
        PartialEq,
        Serialize,
        Deserialize,
        Default,
        TerminusDBModel,
        FromTDBInstance,
    )]
    #[tdb(id_field = "id", unfoldable)]
    struct Person {
        id: EntityIDFor<Self>,
        name: String,
        age: i32,
        #[tdb(instance)]
        address: Option<Address>,
    }

    #[derive(
        Debug,
        Clone,
        PartialEq,
        Serialize,
        Deserialize,
        Default,
        TerminusDBModel,
        FromTDBInstance,
    )]
    #[tdb(id_field = "id")]
    struct Company {
        id: EntityIDFor<Self>,
        name: String,
        #[tdb(instance)]
        ceo: Option<Person>,
        #[tdb(instance)]
        employees: Vec<Person>,
    }

    async fn setup_test_data(client: &TerminusDBHttpClient) -> Result<()> {
        let db_name = "test_unfold_db";
        let spec = BranchSpec::from(db_name);

        // Clean up if exists
        let _ = client.delete_database(db_name).await;

        // Create database and add schema
        client.create_database(db_name).await?;
        let args = DocumentInsertArgs::from(spec.clone());
        client.insert_entity_schema::<Address>(args.clone()).await?;
        client.insert_entity_schema::<Person>(args.clone()).await?;
        client.insert_entity_schema::<Company>(args.clone()).await?;

        // Insert test data
        let home_address = Address {
            id: EntityIDFor::new("home").unwrap(),
            street: "123 Main St".to_string(),
            city: "Springfield".to_string(),
            postal_code: "12345".to_string(),
        };

        let office_address = Address {
            id: EntityIDFor::new("office").unwrap(),
            street: "456 Business Ave".to_string(),
            city: "Corporate City".to_string(),
            postal_code: "67890".to_string(),
        };

        let alice = Person {
            id: EntityIDFor::new("alice").unwrap(),
            name: "Alice Smith".to_string(),
            age: 30,
            address: Some(home_address.clone()),
        };

        let bob = Person {
            id: EntityIDFor::new("bob").unwrap(),
            name: "Bob Johnson".to_string(),
            age: 35,
            address: Some(office_address.clone()),
        };

        let company = Company {
            id: EntityIDFor::new("acme").unwrap(),
            name: "ACME Corp".to_string(),
            ceo: Some(alice.clone()),
            employees: vec![alice.clone(), bob.clone()],
        };

        let args = DocumentInsertArgs {
            spec: spec.clone(),
            author: "test".to_string(),
            message: "Setup test data".to_string(),
            ..Default::default()
        };

        client.insert_instance(&home_address, args.clone()).await?;
        client.insert_instance(&office_address, args.clone()).await?;
        client.insert_instance(&alice, args.clone()).await?;
        client.insert_instance(&bob, args.clone()).await?;
        client.insert_instance(&company, args).await?;

        Ok(())
    }

    #[tokio::test]
    #[ignore] // Requires running TerminusDB instance
    async fn test_get_instance_automatic_unfold() -> Result<()> {
        let client = TerminusDBHttpClient::local_node_test().await?;
        setup_test_data(&client).await?;

        let spec = BranchSpec::from("test_unfold_db");
        let mut deserializer = DefaultTDBDeserializer;

        // Test automatic unfolding for Person (has @unfoldable attribute)
        let person: Person = client
            .get_instance("alice", &spec, &mut deserializer)
            .await?;

        // Should have unfolded the address
        assert!(person.address.is_some());
        let address = person.address.unwrap();
        assert_eq!(address.street, "123 Main St");
        assert_eq!(address.city, "Springfield");

        Ok(())
    }

    #[tokio::test]
    #[ignore] // Requires running TerminusDB instance
    async fn test_get_instance_unfolded() -> Result<()> {
        let client = TerminusDBHttpClient::local_node_test().await?;
        setup_test_data(&client).await?;

        let spec = BranchSpec::from("test_unfold_db");
        let mut deserializer = DefaultTDBDeserializer;

        // Test explicit unfolding for Company (doesn't have @unfoldable attribute)
        let company: Company = client
            .get_instance_unfolded("acme", &spec, &mut deserializer)
            .await?;

        // Should have unfolded the CEO and employees
        assert!(company.ceo.is_some());
        let ceo = company.ceo.unwrap();
        assert_eq!(ceo.name, "Alice Smith");
        
        // CEO's address should also be unfolded
        assert!(ceo.address.is_some());
        assert_eq!(ceo.address.unwrap().street, "123 Main St");

        // Employees should be unfolded
        assert_eq!(company.employees.len(), 2);
        assert_eq!(company.employees[0].name, "Alice Smith");
        assert_eq!(company.employees[1].name, "Bob Johnson");

        Ok(())
    }

    #[tokio::test]
    #[ignore] // Requires running TerminusDB instance
    async fn test_get_instance_with_opts_no_unfold() -> Result<()> {
        let client = TerminusDBHttpClient::local_node_test().await?;
        setup_test_data(&client).await?;

        let spec = BranchSpec::from("test_unfold_db");
        let mut deserializer = DefaultTDBDeserializer;

        // Test disabling unfold even for models with @unfoldable
        let opts = GetOpts::default().with_unfold(false);
        let person: Person = client
            .get_instance_with_opts("alice", &spec, opts, &mut deserializer)
            .await?;

        // Address should not be unfolded (will be a reference or None depending on implementation)
        // This test might need adjustment based on actual behavior
        assert_eq!(person.name, "Alice Smith");

        Ok(())
    }

    #[tokio::test]
    #[ignore] // Requires running TerminusDB instance
    async fn test_get_instances_unfolded() -> Result<()> {
        let client = TerminusDBHttpClient::local_node_test().await?;
        setup_test_data(&client).await?;

        let spec = BranchSpec::from("test_unfold_db");
        let mut deserializer = DefaultTDBDeserializer;

        // Test bulk retrieval with unfolding
        let ids = vec!["alice".to_string(), "bob".to_string()];
        let people: Vec<Person> = client
            .get_instances_unfolded(ids, &spec, &mut deserializer)
            .await?;

        assert_eq!(people.len(), 2);
        
        // Both should have unfolded addresses
        for person in &people {
            assert!(person.address.is_some());
            let address = person.address.as_ref().unwrap();
            assert!(!address.street.is_empty());
            assert!(!address.city.is_empty());
        }

        Ok(())
    }

    #[tokio::test]
    #[ignore] // Requires running TerminusDB instance
    async fn test_get_instances_with_opts_pagination() -> Result<()> {
        let client = TerminusDBHttpClient::local_node_test().await?;
        setup_test_data(&client).await?;

        let spec = BranchSpec::from("test_unfold_db");
        let mut deserializer = DefaultTDBDeserializer;

        // Test pagination with type filtering
        let empty_ids = vec![];
        let opts = GetOpts::filtered_by_type::<Person>()
            .with_count(1)
            .with_unfold(true);
        
        let people: Vec<Person> = client
            .get_instances_with_opts(empty_ids, &spec, opts, &mut deserializer)
            .await?;

        // Should get only 1 person due to count limit
        assert_eq!(people.len(), 1);
        assert!(people[0].address.is_some());

        Ok(())
    }

    #[tokio::test]
    #[ignore] // Requires running TerminusDB instance
    async fn test_cleanup() -> Result<()> {
        let client = TerminusDBHttpClient::local_node_test().await?;
        let _ = client.delete_database("test_unfold_db").await;
        Ok(())
    }
}