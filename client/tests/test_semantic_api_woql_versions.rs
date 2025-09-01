use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
use terminusdb_woql_builder::prelude::*;

/// Test model for semantic API version testing
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct VersionTestModel {
    id: EntityIDFor<Self>,
    name: String,
    version: i32,
    data: String,
}

/// Test setup
async fn setup_test_client() -> anyhow::Result<(TerminusDBHttpClient, BranchSpec)> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let spec = BranchSpec::from("test");

    // Insert schema
    let args = DocumentInsertArgs::from(spec.clone());
    client
        .insert_entity_schema::<VersionTestModel>(args)
        .await
        .ok();

    Ok((client, spec))
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_semantic_api_creates_queryable_versions() -> anyhow::Result<()> {
    let (client, spec) = setup_test_client().await?;

    let fixed_id = &format!(
        "semantic_version_test_{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    println!("=== Testing semantic API version creation with WOQL queries ===");
    println!("Using ID: {}", fixed_id);

    // Version 1: Create new instance
    let model_v1 = VersionTestModel {
        id: EntityIDFor::new(fixed_id).unwrap(),
        name: "Version 1".to_string(),
        version: 1,
        data: "Initial data".to_string(),
    };

    let args = DocumentInsertArgs::from(spec.clone());
    let result1 = client.create_instance(&model_v1, args).await?;
    let commit_id_1 = result1.extract_commit_id().expect("Should have commit ID");
    println!("\nV1: Created in commit {}", commit_id_1);

    // Version 2: Update existing instance
    let model_v2 = VersionTestModel {
        id: EntityIDFor::new(fixed_id).unwrap(),
        name: "Version 2".to_string(),
        version: 2,
        data: "Updated data".to_string(),
    };

    let args = DocumentInsertArgs::from(spec.clone());
    let result2 = client.update_instance(&model_v2, args).await?;
    let commit_id_2 = result2.extract_commit_id().expect("Should have commit ID");
    println!("V2: Updated in commit {}", commit_id_2);

    // Version 3: Replace instance
    let model_v3 = VersionTestModel {
        id: EntityIDFor::new(fixed_id).unwrap(),
        name: "Version 3".to_string(),
        version: 3,
        data: "Replaced data".to_string(),
    };

    let args = DocumentInsertArgs::from(spec.clone());
    let result3 = client.replace_instance(&model_v3, args).await?;
    let commit_id_3 = result3.extract_commit_id().expect("Should have commit ID");
    println!("V3: Replaced in commit {}", commit_id_3);

    println!("\n=== Testing WOQL queries on each commit ===");

    // Test querying each commit
    let commit_ids = vec![commit_id_1, commit_id_2, commit_id_3];
    let mut found_versions = Vec::new();

    for (i, commit_id) in commit_ids.iter().enumerate() {
        println!("\nQuerying commit {} ({})", i + 1, commit_id);
        let collection = format!("admin/{}/local/commit/{}", spec.db, commit_id);

        // Query for VersionTestModel instances
        let query = WoqlBuilder::new()
            .triple(
                vars!("Subject"),
                "rdf:type",
                node("@schema:VersionTestModel"),
            )
            .triple(vars!("Subject"), "@schema:name", vars!("Name"))
            .triple(vars!("Subject"), "@schema:version", vars!("Version"))
            .triple(vars!("Subject"), "@schema:data", vars!("Data"))
            .select(vec![
                vars!("Subject"),
                vars!("Name"),
                vars!("Version"),
                vars!("Data"),
            ])
            .using(&collection)
            .finalize();

        let json_query = query.to_instance(None).to_json();

        match client.query_raw(Some(spec.clone()), json_query).await {
            Ok(result) => {
                let result: WOQLResult<HashMap<String, serde_json::Value>> = result;
                println!("  Found {} bindings", result.bindings.len());

                for binding in &result.bindings {
                    if let (Some(name), Some(version), Some(data)) = (
                        binding.get("Name"),
                        binding.get("Version"),
                        binding.get("Data"),
                    ) {
                        println!(
                            "  - Name: {:?}, Version: {:?}, Data: {:?}",
                            name, version, data
                        );
                        found_versions.push((i + 1, name.clone(), version.clone()));
                    }
                }
            }
            Err(e) => {
                println!("  Error querying commit: {}", e);
            }
        }
    }

    println!("\n=== Testing combined WOQL query across all commits ===");

    // Build OR query across all commits
    let mut or_queries = Vec::new();

    for commit_id in &commit_ids {
        let collection = format!("admin/{}/local/commit/{}", spec.db, commit_id);

        let query = WoqlBuilder::new()
            .triple(
                vars!("Subject"),
                "rdf:type",
                node("@schema:VersionTestModel"),
            )
            .read_document(vars!("Subject"), vars!("Doc"))
            .select(vec![vars!("Subject"), vars!("Doc")])
            .using(&collection);

        or_queries.push(query);
    }

    if !or_queries.is_empty() {
        let mut or_queries_iter = or_queries.into_iter();
        let mut combined_query = or_queries_iter.next().unwrap();
        for q in or_queries_iter {
            combined_query = combined_query.or([q]);
        }

        let final_query = combined_query.finalize();
        let json_query = final_query.to_instance(None).to_json();

        match client.query_raw(Some(spec.clone()), json_query).await {
            Ok(result) => {
                let result: WOQLResult<HashMap<String, serde_json::Value>> = result;
                println!("Combined query returned {} bindings", result.bindings.len());
            }
            Err(e) => {
                println!("Combined query failed: {}", e);
            }
        }
    }

    println!("\n=== RESULT ===");
    if found_versions.is_empty() {
        println!("❌ PROBLEM: No versions found via WOQL queries");
        println!("This suggests that semantic API methods are not creating data visible to commit-specific queries");
    } else {
        println!(
            "✅ Found {} version(s) via WOQL queries",
            found_versions.len()
        );
        for (commit_num, name, version) in &found_versions {
            println!(
                "  Commit {}: Name={:?}, Version={:?}",
                commit_num, name, version
            );
        }
    }

    // Also test the list_instance_versions method
    println!("\n=== Testing list_instance_versions method ===");
    let mut deserializer = terminusdb_client::deserialize::DefaultTDBDeserializer;
    let versions = client
        .list_instance_versions::<VersionTestModel>(fixed_id, &spec, &mut deserializer)
        .await?;

    println!(
        "list_instance_versions returned {} versions",
        versions.len()
    );
    for (i, (model, commit_id)) in versions.iter().enumerate() {
        println!(
            "  Version {}: {} (v{}) in commit {}",
            i + 1,
            model.name,
            model.version,
            commit_id
        );
    }

    Ok(())
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_direct_rest_api_time_travel() -> anyhow::Result<()> {
    let (client, spec) = setup_test_client().await?;

    let fixed_id = &format!(
        "rest_api_test_{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    println!("=== Testing direct REST API time-travel ===");

    // Create and update an instance
    let model = VersionTestModel {
        id: EntityIDFor::new(fixed_id).unwrap(),
        name: "REST Test".to_string(),
        version: 1,
        data: "Initial".to_string(),
    };

    let args = DocumentInsertArgs::from(spec.clone());
    let result = client.create_instance(&model, args).await?;
    let commit_id = result.extract_commit_id().expect("Should have commit ID");
    println!("Created in commit: {}", commit_id);

    // Try to retrieve it directly with commit ref
    let mut commit_spec = spec.clone();
    commit_spec.ref_commit = Some(commit_id.clone().into());

    match client
        .get_instance::<VersionTestModel>(
            fixed_id,
            &commit_spec,
            &mut terminusdb_client::deserialize::DefaultTDBDeserializer,
        )
        .await
    {
        Ok(instance) => {
            println!(
                "✅ Successfully retrieved instance from commit {}",
                commit_id
            );
            println!("  Instance: {} (v{})", instance.name, instance.version);
        }
        Err(e) => {
            println!("❌ Failed to retrieve instance from commit: {}", e);
        }
    }

    Ok(())
}
