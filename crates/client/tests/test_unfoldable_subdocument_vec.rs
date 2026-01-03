use anyhow::Result;
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{TerminusDBModel, FromTDBInstance};

#[derive(Debug, Clone, Eq, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(unfoldable = true)]
pub struct IdAndTitle {
    /// section or document ID
    pub id: String,
    /// title if exists
    pub title: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(unfoldable = true)]
pub struct ReviewSession {
    #[tdb(subdocument = true)]
    pub document_ids: Vec<IdAndTitle>,

    // Add a simple field to make it easier to identify
    pub session_name: String,
}

#[tokio::test]
async fn test_unfoldable_with_subdocument_vec() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_unfoldable_subdoc", |client, spec| async move {
            // Insert schemas - both IdAndTitle and ReviewSession are marked as unfoldable
            let args = DocumentInsertArgs::from(spec.clone());
            client.insert_entity_schema::<IdAndTitle>(args.clone()).await?;
            client.insert_entity_schema::<ReviewSession>(args.clone()).await?;

            // Create test data
            let review_session = ReviewSession {
                session_name: "Test Session".to_string(),
                document_ids: vec![
                    IdAndTitle {
                        id: "doc1".to_string(),
                        title: Some("Document One".to_string()),
                    },
                    IdAndTitle {
                        id: "doc2".to_string(),
                        title: None,
                    },
                    IdAndTitle {
                        id: "doc3".to_string(),
                        title: Some("Document Three".to_string()),
                    },
                ],
            };

            // Insert the instance
            let insert_result = client.insert_instance(&review_session, args.clone())
                .await?;

            println!("Inserted instance with commit ID: {:?}", insert_result.commit_id);

            // Extract the instance ID from the result
            let instance_id = match &insert_result.root_result {
                TDBInsertInstanceResult::Inserted(id) => id.clone(),
                TDBInsertInstanceResult::AlreadyExists(id) => id.clone(),
            };

            // Extract just the ID part (after the last /)
            let short_id = instance_id.split('/').last().unwrap_or(&instance_id).to_string();
            println!("Instance ID: {}", short_id);

            // First, get instances WITHOUT unfolding to see the difference
            // When unfold=false, subdocuments would typically be stored as references/IDs
            let mut deserializer = DefaultTDBDeserializer;
            let opts = GetOpts::default().with_unfold(false);
            let sessions_no_unfold: Vec<ReviewSession> = client
                .get_instances(vec![short_id.clone()], &spec, opts, &mut deserializer)
                .await?;

            println!("Retrieved {} sessions without unfolding", sessions_no_unfold.len());
            assert_eq!(sessions_no_unfold.len(), 1, "Should retrieve exactly one session");

            // Check what the subdocuments look like without unfolding
            println!("Without unfolding - Number of document_ids: {}", sessions_no_unfold[0].document_ids.len());
            for (i, doc) in sessions_no_unfold[0].document_ids.iter().enumerate() {
                println!("  Doc {}: id={}, title={:?}", i, doc.id, doc.title);
            }

            // Now test with get_instances_unfolded - this automatically sets unfold=true
            // This means subdocuments marked with #[tdb(subdocument=true)] will be retrieved
            // with their full content instead of just references
            let sessions: Vec<ReviewSession> = client
                .get_instances_unfolded(vec![short_id], &spec, &mut deserializer)
                .await?;

            println!("Retrieved {} sessions WITH unfolding", sessions.len());
            assert_eq!(sessions.len(), 1, "Should retrieve exactly one session");

            let first_session = &sessions[0];

            // Check the subdocuments - they should be populated
            println!("Session name: {}", first_session.session_name);
            println!("Number of document_ids: {}", first_session.document_ids.len());

            assert_eq!(first_session.document_ids.len(), 3, "Should have 3 subdocuments");
            assert_eq!(first_session.document_ids[0].id, "doc1");
            assert_eq!(first_session.document_ids[0].title, Some("Document One".to_string()));
            assert_eq!(first_session.document_ids[1].id, "doc2");
            assert_eq!(first_session.document_ids[1].title, None);
            assert_eq!(first_session.document_ids[2].id, "doc3");
            assert_eq!(first_session.document_ids[2].title, Some("Document Three".to_string()));

            println!("Test passed! Unfoldable structs with subdocument vectors work correctly.");
            Ok(())
        })
        .await
}

#[tokio::test]
async fn test_empty_subdocument_vec() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;

    server
        .with_tmp_db("test_empty_subdoc", |client, spec| async move {
            // Insert schemas
            let args = DocumentInsertArgs::from(spec.clone());
            client.insert_entity_schema::<IdAndTitle>(args.clone()).await?;
            client.insert_entity_schema::<ReviewSession>(args.clone()).await?;

            let empty_session = ReviewSession {
                session_name: "Empty Session".to_string(),
                document_ids: vec![], // Empty vector
            };

            client.insert_instance(&empty_session, args.clone())
                .await?;

            let mut deserializer = DefaultTDBDeserializer;
            let sessions: Vec<ReviewSession> = client
                .get_instances_unfolded(vec![], &spec, &mut deserializer)
                .await?;

            assert_eq!(sessions.len(), 1);
            assert_eq!(sessions[0].session_name, "Empty Session");
            assert_eq!(sessions[0].document_ids.len(), 0);

            Ok(())
        })
        .await
}
