use terminusdb_schema::{Schema, ToTDBSchema, ToTDBInstance, TypeFamily, Key};
use terminusdb_schema_derive::TerminusDBModel;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq, TerminusDBModel)]
#[tdb(unfoldable = true, key = "value_hash")]
pub struct IdAndTitle {
    /// section or document ID
    pub id: String,

    /// title if exists
    pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq, TerminusDBModel)]
#[serde(rename_all = "camelCase")]
#[tdb(unfoldable = true)]
pub struct ReviewSession {
    #[tdb(subdocument = true)]
    pub document_ids: Vec<IdAndTitle>,
    
    pub session_name: String,
}

#[test]
fn test_unfoldable_subdocument_schema() {
    // Test IdAndTitle schema
    let id_and_title_schema = IdAndTitle::to_schema();
    match &id_and_title_schema {
        Schema::Class { unfoldable, key, subdocument, .. } => {
            assert!(unfoldable, "IdAndTitle should be unfoldable");
            assert_eq!(key, &Key::ValueHash, "IdAndTitle should have value_hash key");
            assert!(!subdocument, "IdAndTitle itself is not marked as subdocument");
        }
        _ => panic!("IdAndTitle should generate a Class schema"),
    }
    
    // Test ReviewSession schema
    let review_session_schema = ReviewSession::to_schema();
    match &review_session_schema {
        Schema::Class { unfoldable, properties, .. } => {
            assert!(unfoldable, "ReviewSession should be unfoldable");
            
            // Find the document_ids property
            let doc_ids_prop = properties.iter()
                .find(|p| p.name == "document_ids")
                .expect("Should have document_ids property");
            
            // Check that it's a set/list type
            assert_eq!(doc_ids_prop.r#type, Some(TypeFamily::List));
            assert_eq!(doc_ids_prop.class, "IdAndTitle");
        }
        _ => panic!("ReviewSession should generate a Class schema"),
    }
}

#[test]
fn test_subdocument_instance_generation() {
    let review_session = ReviewSession {
        session_name: "Test Session".to_string(),
        document_ids: vec![
            IdAndTitle {
                id: "doc1".to_string(),
                title: Some("First Document".to_string()),
            },
            IdAndTitle {
                id: "doc2".to_string(),
                title: None,
            },
        ],
    };
    
    // Convert to instance
    let instance = review_session.to_instance(None);
    
    // Check the properties
    assert_eq!(instance.properties.len(), 2, "Should have 2 properties");
    
    // Check document_ids property
    let doc_ids_prop = instance.properties.get("document_ids")
        .expect("Should have document_ids property");
    
    println!("document_ids property type: {:?}", doc_ids_prop);
    
    // For subdocuments in a list, they should be Relations
    match doc_ids_prop {
        terminusdb_schema::InstanceProperty::Relations(relations) => {
            assert_eq!(relations.len(), 2, "Should have 2 subdocuments");
            
            // Each relation should contain a subdocument instance
            for (i, relation) in relations.iter().enumerate() {
                println!("Relation {}: {:?}", i, relation);
                match relation {
                    terminusdb_schema::RelationValue::One(sub_instance) => {
                        // The subdocument flag is in the schema, not the instance
                        // When a field is marked with #[tdb(subdocument=true)], it affects
                        // how the instances are stored (inline vs reference)
                        
                        // Verify schema properties
                        match &sub_instance.schema {
                            Schema::Class { unfoldable, .. } => {
                                assert!(*unfoldable, "IdAndTitle should be unfoldable");
                            }
                            _ => panic!("Subdocument should have Class schema"),
                        }
                    }
                    _ => panic!("Expected RelationValue::One for subdocument"),
                }
            }
        }
        _ => panic!("document_ids should be Relations property, got: {:?}", doc_ids_prop),
    }
}