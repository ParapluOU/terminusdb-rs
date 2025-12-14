#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test_document_if_exists {
    use serde_json::json;
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;

    /// Test that get_document_if_exists returns None for non-existent documents
    /// without logging errors (unlike get_document which logs DocumentNotFound as error)
    #[tokio::test]
    async fn test_get_document_if_exists_not_found() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_doc_if_exists", |client, spec| async move {
                // Try to get a document that doesn't exist
                let result = client
                    .get_document_if_exists("NonExistentDoc/12345", &spec, GetOpts::default())
                    .await;

                assert!(result.is_ok());
                assert_eq!(result.unwrap(), None);

                Ok(())
            })
            .await
    }

    /// Test that get_document_if_exists returns Some(document) for existing documents
    #[tokio::test]
    async fn test_get_document_if_exists_found() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_doc_exists_found", |client, spec| async move {
                // First insert a schema
                let schema = json!({
                    "@type": "Class",
                    "@id": "TestDoc",
                    "name": "xsd:string"
                });
                client
                    .insert_documents(
                        vec![&schema],
                        DocumentInsertArgs::from(spec.clone()).as_schema(),
                    )
                    .await?;

                // Insert a test document
                let doc = json!({
                    "@id": "TestDoc/exists_test",
                    "@type": "TestDoc",
                    "name": "Test Document"
                });

                client
                    .insert_documents(vec![&doc], DocumentInsertArgs::from(spec.clone()))
                    .await?;

                // Now try to get it with get_document_if_exists
                let result = client
                    .get_document_if_exists("TestDoc/exists_test", &spec, GetOpts::default())
                    .await;

                assert!(result.is_ok());
                let doc_opt = result.unwrap();
                assert!(doc_opt.is_some());

                let retrieved_doc = doc_opt.unwrap();
                assert!(retrieved_doc["@id"]
                    .as_str()
                    .map(|s| s.ends_with("TestDoc/exists_test"))
                    .unwrap_or(false));
                assert_eq!(retrieved_doc["name"], "Test Document");

                Ok(())
            })
            .await
    }

    /// Test that has_document now uses the non-error-logging approach
    #[tokio::test]
    async fn test_has_document_no_error_logging() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_has_doc", |client, spec| async move {
                // Check for a non-existent document - should not log errors
                let exists = client.has_document("NonExistentDoc/99999", &spec).await;
                assert!(!exists);

                Ok(())
            })
            .await
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test_instance_if_exists {
    use serde::{Deserialize, Serialize};
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::ToTDBInstance;
    use terminusdb_schema_derive::TerminusDBModel;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TerminusDBModel)]
    struct TestPerson {
        name: String,
        age: i32,
    }

    /// Test that get_instance_if_exists returns None for non-existent instances
    /// without logging errors
    #[tokio::test]
    async fn test_get_instance_if_exists_not_found() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_inst_not_found", |client, spec| async move {
                // Insert schema
                let args = DocumentInsertArgs::from(spec.clone());
                let _ = client.insert_entity_schema::<TestPerson>(args).await;

                // Try to get an instance that doesn't exist
                let mut deserializer = deserialize::DefaultTDBDeserializer;
                let result = client
                    .get_instance_if_exists::<TestPerson>(
                        "nonexistent123",
                        &spec,
                        &mut deserializer,
                    )
                    .await;

                assert!(result.is_ok());
                assert_eq!(result.unwrap(), None);

                Ok(())
            })
            .await
    }

    /// Test that get_instance_if_exists returns Some(instance) for existing instances
    #[tokio::test]
    async fn test_get_instance_if_exists_found() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_inst_found", |client, spec| async move {
                // Insert schema
                let args = DocumentInsertArgs::from(spec.clone());
                let _ = client.insert_entity_schema::<TestPerson>(args.clone()).await;

                // Insert a test instance
                let person = TestPerson {
                    name: "Alice".to_string(),
                    age: 30,
                };

                let result = client.create_instance(&person, args).await?;
                let person_id = result.root_id.clone();

                // Extract just the ID part (after the last /)
                let short_id = person_id.split('/').last().unwrap_or(&person_id);

                // Now try to get it with get_instance_if_exists
                let mut deserializer = deserialize::DefaultTDBDeserializer;
                let result = client
                    .get_instance_if_exists::<TestPerson>(short_id, &spec, &mut deserializer)
                    .await;

                assert!(result.is_ok());
                let person_opt = result.unwrap();
                assert!(person_opt.is_some());

                let retrieved_person = person_opt.unwrap();
                assert_eq!(retrieved_person.name, "Alice");
                assert_eq!(retrieved_person.age, 30);

                Ok(())
            })
            .await
    }
}
