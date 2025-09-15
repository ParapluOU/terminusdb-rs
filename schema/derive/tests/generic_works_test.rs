#![cfg(feature = "generic-derive")]

use serde::{Deserialize, Serialize};
use terminusdb_schema::{EntityIDFor, ToSchemaClass, ToTDBInstance, ToTDBSchema};
use terminusdb_schema_derive::TerminusDBModel;

// Define concrete TerminusDBModel types
#[derive(Debug, Clone, TerminusDBModel, Serialize, Deserialize)]
struct Document {
    id: String,
    title: String,
    content: String,
}

#[derive(Debug, Clone, TerminusDBModel, Serialize, Deserialize)]
struct Author {
    id: String,
    name: String,
    bio: String,
}

// Generic model with EntityIDFor<T>
#[derive(Debug, Clone, TerminusDBModel)]
struct GenericReference<T>
where
    T: ToTDBSchema + ToSchemaClass + Send,
{
    id: String,
    target: EntityIDFor<T>,
    relationship_type: String,
}

#[test]
fn test_generic_reference_with_models() {
    // Create a reference to a Document
    let doc_ref = GenericReference::<Document> {
        id: "ref-1".to_string(),
        target: EntityIDFor::new("doc-123").unwrap(),
        relationship_type: "references".to_string(),
    };

    // Verify we can get the schema
    let doc_schema = <GenericReference<Document> as ToTDBSchema>::to_schema();
    assert_eq!(doc_schema.class_name(), "GenericReference<Document>");

    // Create a reference to an Author
    let author_ref = GenericReference::<Author> {
        id: "ref-2".to_string(),
        target: EntityIDFor::new("author-456").unwrap(),
        relationship_type: "authored_by".to_string(),
    };

    let author_schema = <GenericReference<Author> as ToTDBSchema>::to_schema();
    assert_eq!(author_schema.class_name(), "GenericReference<Author>");

    // Verify we can convert to instances
    let _doc_instance = doc_ref.to_instance(None);
    let _author_instance = author_ref.to_instance(None);

    println!("✅ GenericReference<T> successfully works with TerminusDBModel types!");
    println!("✅ Model<T> {{ EntityIDFor<T> }} pattern is fully functional!");
}
