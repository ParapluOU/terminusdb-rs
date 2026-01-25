//! Example demonstrating multi-document version retrieval
//!
//! This example shows how to efficiently retrieve versions for multiple
//! documents in a single WOQL query.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

#[derive(Debug, Clone, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Document {
    id: EntityIDFor<Self>,
    title: String,
    content: String,
    author: String,
    version: i32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create client
    let client = TerminusDBHttpClient::local_node().await;
    let spec = BranchSpec::from("mydb");

    // Example 1: Get specific versions for multiple documents
    println!("=== Example 1: Get specific versions ===");

    // Suppose we have document IDs and their commit IDs we want to retrieve
    let queries = vec![
        ("doc123", vec!["commit_abc".into(), "commit_def".into()]),
        ("doc456", vec!["commit_ghi".into()]),
        (
            "doc789",
            vec![
                "commit_jkl".into(),
                "commit_mno".into(),
                "commit_pqr".into(),
            ],
        ),
    ];

    let mut deserializer = terminusdb_client::deserialize::DefaultTDBDeserializer;
    let versions = client
        .get_multiple_instance_versions::<Document>(queries, &spec, &mut deserializer)
        .await?;

    // Process results
    for (doc_id, doc_versions) in &versions {
        println!(
            "\nDocument {}: {} versions found",
            doc_id,
            doc_versions.len()
        );
        for (doc, commit_id) in doc_versions {
            println!(
                "  - '{}' by {} (v{}) in commit {}",
                doc.title, doc.author, doc.version, commit_id
            );
        }
    }

    // Example 2: List all versions for multiple documents
    println!("\n=== Example 2: List all versions ===");

    let doc_ids = vec!["doc123", "doc456", "doc789"];
    let all_versions = client
        .list_multiple_instance_versions::<Document>(doc_ids.clone(), &spec, &mut deserializer)
        .await?;

    println!(
        "\nRetrieved complete history for {} documents",
        all_versions.len()
    );
    for doc_id in &doc_ids {
        if let Some(versions) = all_versions.get(*doc_id) {
            println!("  {} has {} total versions", doc_id, versions.len());
        } else {
            println!("  {} not found or has no versions", doc_id);
        }
    }

    // Example 3: Building revision endpoint data
    println!("\n=== Example 3: Building revision data ===");

    // This is ideal for an endpoint that returns all revisions grouped by document
    #[derive(Serialize)]
    struct RevisionResponse {
        document_id: String,
        revisions: Vec<RevisionInfo>,
    }

    #[derive(Serialize)]
    struct RevisionInfo {
        commit_id: String,
        version: i32,
        title: String,
        author: String,
    }

    let mut response_data: Vec<RevisionResponse> = Vec::new();

    for (doc_id, versions) in all_versions {
        let revisions = versions
            .into_iter()
            .map(|(doc, commit_id)| RevisionInfo {
                commit_id: commit_id.to_string(),
                version: doc.version,
                title: doc.title,
                author: doc.author,
            })
            .collect();

        response_data.push(RevisionResponse {
            document_id: doc_id,
            revisions,
        });
    }

    println!(
        "Built revision response for {} documents",
        response_data.len()
    );

    Ok(())
}
