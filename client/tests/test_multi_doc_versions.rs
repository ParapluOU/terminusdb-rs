use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

/// Test model for multi-document version retrieval
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct MultiDocTest {
    id: EntityIDFor<Self>,
    doc_type: String,
    content: String,
    version: i32,
}

/// Test setup
async fn setup_test_client() -> anyhow::Result<(TerminusDBHttpClient, BranchSpec)> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let spec = BranchSpec::from("test");

    // Insert schema
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<MultiDocTest>(args).await.ok();

    Ok((client, spec))
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_get_multiple_instance_versions() -> anyhow::Result<()> {
    let (client, spec) = setup_test_client().await?;

    let doc1_id = &format!(
        "doc1_{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let doc2_id = &format!(
        "doc2_{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let doc3_id = &format!(
        "doc3_{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );

    println!("=== Testing get_multiple_instance_versions ===");
    println!("Document IDs: {}, {}, {}", doc1_id, doc2_id, doc3_id);

    // Create versions for doc1 (3 versions)
    let mut doc1_commits = Vec::new();
    for i in 1..=3 {
        let doc = MultiDocTest {
            id: EntityIDFor::new(doc1_id).unwrap(),
            doc_type: "TypeA".to_string(),
            content: format!("Doc1 Version {}", i),
            version: i,
        };

        let args = DocumentInsertArgs::from(spec.clone());
        let result = if i == 1 {
            client.create_instance(&doc, args).await?
        } else {
            client.update_instance(&doc, args).await?
        };

        let commit_id = result.extract_commit_id().expect("Should have commit ID");
        println!("Doc1 v{} created in commit: {}", i, &commit_id);
        doc1_commits.push(commit_id);
    }

    // Create versions for doc2 (2 versions)
    let mut doc2_commits = Vec::new();
    for i in 1..=2 {
        let doc = MultiDocTest {
            id: EntityIDFor::new(doc2_id).unwrap(),
            doc_type: "TypeB".to_string(),
            content: format!("Doc2 Version {}", i),
            version: i,
        };

        let args = DocumentInsertArgs::from(spec.clone());
        let result = if i == 1 {
            client.create_instance(&doc, args).await?
        } else {
            client.update_instance(&doc, args).await?
        };

        let commit_id = result.extract_commit_id().expect("Should have commit ID");
        println!("Doc2 v{} created in commit: {}", i, &commit_id);
        doc2_commits.push(commit_id);
    }

    // Create versions for doc3 (1 version)
    let doc3 = MultiDocTest {
        id: EntityIDFor::new(doc3_id).unwrap(),
        doc_type: "TypeC".to_string(),
        content: "Doc3 Version 1".to_string(),
        version: 1,
    };

    let args = DocumentInsertArgs::from(spec.clone());
    let result = client.create_instance(&doc3, args).await?;
    let doc3_commit = result.extract_commit_id().expect("Should have commit ID");
    println!("Doc3 v1 created in commit: {}", &doc3_commit);

    // Test 1: Get specific versions for each document
    println!("\n=== Test 1: Get specific versions ===");
    let queries = vec![
        (
            doc1_id.as_str(),
            vec![doc1_commits[0].clone(), doc1_commits[2].clone()],
        ), // v1 and v3
        (doc2_id.as_str(), vec![doc2_commits[1].clone()]), // v2 only
        (doc3_id.as_str(), vec![doc3_commit.clone()]),     // v1
    ];

    let mut deserializer = terminusdb_client::deserialize::DefaultTDBDeserializer;
    let versions = client
        .get_multiple_instance_versions::<MultiDocTest>(queries, &spec, &mut deserializer)
        .await?;

    println!("Retrieved versions for {} documents", versions.len());
    assert_eq!(
        versions.len(),
        3,
        "Should have versions for all 3 documents"
    );

    // Verify doc1 has 2 versions (v1 and v3)
    let doc1_versions = versions.get(doc1_id).unwrap();
    assert_eq!(doc1_versions.len(), 2);
    let doc1_version_numbers: Vec<i32> = doc1_versions.iter().map(|(doc, _)| doc.version).collect();
    assert!(doc1_version_numbers.contains(&1));
    assert!(doc1_version_numbers.contains(&3));
    assert!(!doc1_version_numbers.contains(&2));

    // Verify doc2 has 1 version (v2)
    let doc2_versions = versions.get(doc2_id).unwrap();
    assert_eq!(doc2_versions.len(), 1);
    assert_eq!(doc2_versions[0].0.version, 2);

    // Verify doc3 has 1 version (v1)
    let doc3_versions = versions.get(doc3_id).unwrap();
    assert_eq!(doc3_versions.len(), 1);
    assert_eq!(doc3_versions[0].0.version, 1);

    for (doc_id, doc_versions) in &versions {
        println!("Document {}: {} versions", doc_id, doc_versions.len());
        for (doc, commit_id) in doc_versions {
            println!(
                "  - {} (v{}) in commit {}",
                doc.content, doc.version, commit_id
            );
        }
    }

    // Test 2: Empty query list
    println!("\n=== Test 2: Empty query list ===");
    let empty_queries: Vec<(&str, Vec<String>)> = vec![];
    let versions = client
        .get_multiple_instance_versions::<MultiDocTest>(empty_queries, &spec, &mut deserializer)
        .await?;

    assert_eq!(
        versions.len(),
        0,
        "Empty queries should return empty HashMap"
    );

    // Test 3: Mix of existing and non-existing documents
    println!("\n=== Test 3: Mix with non-existing documents ===");
    let queries = vec![
        (doc1_id.as_str(), vec![doc1_commits[0].clone()]),
        ("non_existent_doc", vec![doc1_commits[0].clone()]), // This doc doesn't exist
        (doc2_id.as_str(), vec![doc2_commits[0].clone()]),
    ];

    let versions = client
        .get_multiple_instance_versions::<MultiDocTest>(queries, &spec, &mut deserializer)
        .await?;

    // Should only have versions for existing documents
    assert_eq!(
        versions.len(),
        2,
        "Should only have versions for existing documents"
    );
    assert!(versions.contains_key(doc1_id));
    assert!(versions.contains_key(doc2_id));
    assert!(!versions.contains_key("non_existent_doc"));

    // Test 4: Document with empty commit list
    println!("\n=== Test 4: Document with empty commit list ===");
    let queries = vec![
        (doc1_id.as_str(), vec![doc1_commits[0].clone()]),
        (doc2_id.as_str(), vec![]), // Empty commit list
        (doc3_id.as_str(), vec![doc3_commit.clone()]),
    ];

    let versions = client
        .get_multiple_instance_versions::<MultiDocTest>(queries, &spec, &mut deserializer)
        .await?;

    // Doc2 shouldn't be in results due to empty commit list
    assert_eq!(versions.len(), 2);
    assert!(versions.contains_key(doc1_id));
    assert!(!versions.contains_key(doc2_id));
    assert!(versions.contains_key(doc3_id));

    Ok(())
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_list_multiple_instance_versions() -> anyhow::Result<()> {
    let (client, spec) = setup_test_client().await?;

    let doc1_id = &format!(
        "doc1_list_{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let doc2_id = &format!(
        "doc2_list_{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );

    println!("=== Testing list_multiple_instance_versions ===");

    // Create 3 versions for doc1
    for i in 1..=3 {
        let doc = MultiDocTest {
            id: EntityIDFor::new(doc1_id).unwrap(),
            doc_type: "TypeA".to_string(),
            content: format!("Doc1 List Version {}", i),
            version: i,
        };

        let args = DocumentInsertArgs::from(spec.clone());
        if i == 1 {
            client.create_instance(&doc, args).await?;
        } else {
            client.update_instance(&doc, args).await?;
        }
    }

    // Create 2 versions for doc2
    for i in 1..=2 {
        let doc = MultiDocTest {
            id: EntityIDFor::new(doc2_id).unwrap(),
            doc_type: "TypeB".to_string(),
            content: format!("Doc2 List Version {}", i),
            version: i,
        };

        let args = DocumentInsertArgs::from(spec.clone());
        if i == 1 {
            client.create_instance(&doc, args).await?;
        } else {
            client.update_instance(&doc, args).await?;
        }
    }

    // Test: List all versions for multiple documents
    let instance_ids = vec![doc1_id.as_str(), doc2_id.as_str(), "non_existent_doc"];

    let mut deserializer = terminusdb_client::deserialize::DefaultTDBDeserializer;
    let all_versions = client
        .list_multiple_instance_versions::<MultiDocTest>(instance_ids, &spec, &mut deserializer)
        .await?;

    println!(
        "Retrieved all versions for {} documents",
        all_versions.len()
    );

    // Verify results
    assert_eq!(
        all_versions.len(),
        2,
        "Should have versions for 2 existing documents"
    );

    let doc1_versions = all_versions.get(doc1_id).unwrap();
    assert_eq!(doc1_versions.len(), 3, "Doc1 should have 3 versions");

    let doc2_versions = all_versions.get(doc2_id).unwrap();
    assert_eq!(doc2_versions.len(), 2, "Doc2 should have 2 versions");

    assert!(!all_versions.contains_key("non_existent_doc"));

    for (doc_id, versions) in &all_versions {
        println!("Document {}: {} total versions", doc_id, versions.len());
        for (doc, commit_id) in versions {
            println!(
                "  - {} (v{}) in commit {}",
                doc.content, doc.version, commit_id
            );
        }
    }

    // Compare with sequential single-document queries
    println!("\n=== Comparing with sequential queries ===");
    let mut sequential_results = HashMap::new();

    for doc_id in &[doc1_id.as_str(), doc2_id.as_str()] {
        let versions = client
            .list_instance_versions::<MultiDocTest>(doc_id, &spec, &mut deserializer)
            .await?;

        if !versions.is_empty() {
            sequential_results.insert(doc_id.to_string(), versions);
        }
    }

    // Results should be the same
    assert_eq!(all_versions.len(), sequential_results.len());
    for (doc_id, multi_versions) in all_versions {
        let seq_versions = sequential_results.get(&doc_id).unwrap();
        assert_eq!(
            multi_versions.len(),
            seq_versions.len(),
            "Version count should match for {}",
            doc_id
        );
    }

    Ok(())
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_multi_doc_overlapping_commits() -> anyhow::Result<()> {
    let (client, spec) = setup_test_client().await?;

    let doc1_id = &format!(
        "overlap1_{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let doc2_id = &format!(
        "overlap2_{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );

    println!("=== Testing overlapping commits scenario ===");

    // Create doc1 v1
    let doc1_v1 = MultiDocTest {
        id: EntityIDFor::new(doc1_id).unwrap(),
        doc_type: "Shared".to_string(),
        content: "Doc1 Initial".to_string(),
        version: 1,
    };
    let args = DocumentInsertArgs::from(spec.clone());
    let result1 = client.create_instance(&doc1_v1, args).await?;
    let commit1 = result1.extract_commit_id().expect("Should have commit ID");

    // Create doc2 v1
    let doc2_v1 = MultiDocTest {
        id: EntityIDFor::new(doc2_id).unwrap(),
        doc_type: "Shared".to_string(),
        content: "Doc2 Initial".to_string(),
        version: 1,
    };
    let args = DocumentInsertArgs::from(spec.clone());
    let result2 = client.create_instance(&doc2_v1, args).await?;
    let commit2 = result2.extract_commit_id().expect("Should have commit ID");

    // Update doc1 v2
    let doc1_v2 = MultiDocTest {
        id: EntityIDFor::new(doc1_id).unwrap(),
        doc_type: "Shared".to_string(),
        content: "Doc1 Updated".to_string(),
        version: 2,
    };
    let args = DocumentInsertArgs::from(spec.clone());
    let result3 = client.update_instance(&doc1_v2, args).await?;
    let commit3 = result3.extract_commit_id().expect("Should have commit ID");

    // Query with overlapping commits
    let queries = vec![
        (
            doc1_id.as_str(),
            vec![commit1.clone(), commit2.clone(), commit3.clone()],
        ),
        (
            doc2_id.as_str(),
            vec![commit1.clone(), commit2.clone(), commit3.clone()],
        ),
    ];

    let mut deserializer = terminusdb_client::deserialize::DefaultTDBDeserializer;
    let versions = client
        .get_multiple_instance_versions::<MultiDocTest>(queries, &spec, &mut deserializer)
        .await?;

    // Doc1 should have versions from commit1 and commit3
    let doc1_versions = versions.get(doc1_id).unwrap();
    assert!(
        doc1_versions.len() >= 2,
        "Doc1 should have at least 2 versions"
    );

    // Doc2 should have version from commit2 only
    let doc2_versions = versions.get(doc2_id).unwrap();
    assert!(
        doc2_versions.len() >= 1,
        "Doc2 should have at least 1 version"
    );

    println!("Doc1 found in {} commits", doc1_versions.len());
    println!("Doc2 found in {} commits", doc2_versions.len());

    Ok(())
}
