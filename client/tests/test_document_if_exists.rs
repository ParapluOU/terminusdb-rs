#[cfg(test)]
mod test_document_if_exists {
    use terminusdb_client::document::GetOpts;
    use terminusdb_client::spec::BranchSpec;
    use terminusdb_client::TerminusDBHttpClient;

    /// Test that get_document_if_exists returns None for non-existent documents
    /// without logging errors (unlike get_document which logs DocumentNotFound as error)
    #[tokio::test]
    #[ignore] // Requires running TerminusDB instance
    async fn test_get_document_if_exists_not_found() {
        let client = TerminusDBHttpClient::local_node().await.unwrap();
        let spec = BranchSpec::from("test_db");

        // Ensure the database exists
        let _ = client.ensure_database(&spec.db).await;

        // Try to get a document that doesn't exist
        let result = client
            .get_document_if_exists("NonExistentDoc/12345", &spec, GetOpts::default())
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    /// Test that get_document_if_exists returns Some(document) for existing documents
    #[tokio::test]
    #[ignore] // Requires running TerminusDB instance
    async fn test_get_document_if_exists_found() {
        use serde_json::json;
        use terminusdb_client::document::DocumentInsertArgs;
        use terminusdb_client::document::DocumentType;

        let client = TerminusDBHttpClient::local_node().await.unwrap();
        let spec = BranchSpec::from("test_db");

        // Ensure the database exists
        let _ = client.ensure_database(&spec.db).await;

        // Insert a test document
        let doc = json!({
            "@id": "TestDoc/exists_test",
            "@type": "TestDoc",
            "name": "Test Document"
        });

        let args = DocumentInsertArgs {
            message: "Insert test document".to_string(),
            ty: DocumentType::Instance,
            author: "test".to_string(),
            spec: spec.clone(),
            force: false,
        };

        let _ = client.post_documents(vec![&doc], args).await;

        // Now try to get it with get_document_if_exists
        let result = client
            .get_document_if_exists("TestDoc/exists_test", &spec, GetOpts::default())
            .await;

        assert!(result.is_ok());
        let doc_opt = result.unwrap();
        assert!(doc_opt.is_some());
        
        let retrieved_doc = doc_opt.unwrap();
        assert_eq!(retrieved_doc["@id"], "TestDoc/exists_test");
        assert_eq!(retrieved_doc["name"], "Test Document");
    }

    /// Test that has_document now uses the non-error-logging approach
    #[tokio::test]
    #[ignore] // Requires running TerminusDB instance
    async fn test_has_document_no_error_logging() {
        let client = TerminusDBHttpClient::local_node().await.unwrap();
        let spec = BranchSpec::from("test_db");

        // Ensure the database exists
        let _ = client.ensure_database(&spec.db).await;

        // Check for a non-existent document - should not log errors
        let exists = client.has_document("NonExistentDoc/99999", &spec).await;
        assert_eq!(exists, false);
    }
}

#[cfg(test)]
mod test_instance_if_exists {
    use terminusdb_client::{DefaultDeserializer, TDBInstanceDeserializer};
    use terminusdb_client::spec::BranchSpec;
    use terminusdb_client::TerminusDBHttpClient;
    use terminusdb_schema_derive::TerminusDBModel;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TerminusDBModel)]
    struct TestPerson {
        name: String,
        age: i32,
    }

    /// Test that get_instance_if_exists returns None for non-existent instances
    /// without logging errors
    #[tokio::test]
    #[ignore] // Requires running TerminusDB instance
    async fn test_get_instance_if_exists_not_found() {
        let client = TerminusDBHttpClient::local_node().await.unwrap();
        let spec = BranchSpec::from("test_db");

        // Ensure the database exists
        let _ = client.ensure_database(&spec.db).await;

        // Insert schema
        use terminusdb_client::document::DocumentInsertArgs;
        use terminusdb_client::document::DocumentType;
        let args = DocumentInsertArgs {
            message: "Insert schema".to_string(),
            ty: DocumentType::Schema,
            author: "test".to_string(),
            spec: spec.clone(),
            force: false,
        };
        let _ = client.insert_entity_schema::<TestPerson>(args).await;

        // Try to get an instance that doesn't exist
        let mut deserializer = DefaultDeserializer::new();
        let result = client
            .get_instance_if_exists::<TestPerson>("nonexistent123", &spec, &mut deserializer)
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    /// Test that get_instance_if_exists returns Some(instance) for existing instances
    #[tokio::test]
    #[ignore] // Requires running TerminusDB instance
    async fn test_get_instance_if_exists_found() {
        let client = TerminusDBHttpClient::local_node().await.unwrap();
        let spec = BranchSpec::from("test_db");

        // Ensure the database exists
        let _ = client.ensure_database(&spec.db).await;

        // Insert schema
        use terminusdb_client::document::DocumentInsertArgs;
        use terminusdb_client::document::DocumentType;
        let args = DocumentInsertArgs {
            message: "Insert schema".to_string(),
            ty: DocumentType::Schema,
            author: "test".to_string(),
            spec: spec.clone(),
            force: false,
        };
        let _ = client.insert_entity_schema::<TestPerson>(args.clone()).await;

        // Insert a test instance
        let person = TestPerson {
            name: "Alice".to_string(),
            age: 30,
        };
        
        let insert_args = DocumentInsertArgs {
            message: "Insert test person".to_string(),
            ty: DocumentType::Instance,
            author: "test".to_string(),
            spec: spec.clone(),
            force: false,
        };
        let result = client.create_instance(&person, insert_args).await.unwrap();
        let person_id = result.root_id.clone();

        // Now try to get it with get_instance_if_exists
        let mut deserializer = DefaultDeserializer::new();
        let result = client
            .get_instance_if_exists::<TestPerson>(&person_id, &spec, &mut deserializer)
            .await;

        assert!(result.is_ok());
        let person_opt = result.unwrap();
        assert!(person_opt.is_some());
        
        let retrieved_person = person_opt.unwrap();
        assert_eq!(retrieved_person.name, "Alice");
        assert_eq!(retrieved_person.age, 30);
    }
}