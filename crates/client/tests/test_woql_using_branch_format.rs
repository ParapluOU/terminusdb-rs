#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::*;
    use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};
    use terminusdb_woql_builder::prelude::*;

    /// Test model with explicit ID for version history testing
    #[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
    #[tdb(id_field = "id")]
    struct PersonWithId {
        id: EntityIDFor<Self>,
        name: String,
        age: i32,
        email: Option<String>,
    }

    async fn test_query_format(
        client: &TerminusDBHttpClient,
        spec: &BranchSpec,
        collection: &str,
        format_name: &str,
    ) -> anyhow::Result<()> {
        println!("Testing {}: {}", format_name, collection);

        // Build query
        let query = WoqlBuilder::new()
            .triple(vars!("Subject"), "rdf:type", vars!("Type"))
            .select(vec![vars!("Subject"), vars!("Type")])
            .using(collection)
            .limit(10)
            .finalize();

        let json_query = query.to_instance(None).to_json();
        println!("Query JSON: {}", serde_json::to_string_pretty(&json_query)?);

        // Execute query
        match client.query_raw(Some(spec.clone()), json_query, None).await {
            Ok(result) => {
                let result: WOQLResult<HashMap<String, serde_json::Value>> = result;
                println!("✓ Success! Found {} bindings", result.bindings.len());

                // Check if we found PersonWithId
                let has_person = result.bindings.iter().any(|b| {
                    b.get("Type")
                        .and_then(|t| t.as_str())
                        .map(|s| s.contains("PersonWithId"))
                        .unwrap_or(false)
                });

                if has_person {
                    println!("  🎯 Found PersonWithId instances!");
                }

                // Show first few results
                for (i, binding) in result.bindings.iter().take(3).enumerate() {
                    println!(
                        "  Binding {}: Subject={:?}, Type={:?}",
                        i + 1,
                        binding.get("Subject"),
                        binding.get("Type")
                    );
                }
            }
            Err(e) => {
                println!("✗ Failed: {}", e);
            }
        }

        Ok(())
    }

    async fn test_multi_version_query(
        client: &TerminusDBHttpClient,
        spec: &BranchSpec,
        commit_ids: Vec<CommitId>,
    ) -> anyhow::Result<()> {
        println!(
            "Building multi-version query for {} commits",
            commit_ids.len()
        );

        // Try with branch/commitID format
        let mut or_queries = Vec::new();

        for commit_id in &commit_ids {
            let collection = format!("main/{}", commit_id);

            let query = WoqlBuilder::new()
                .triple(vars!("Subject"), "rdf:type", node("@schema:PersonWithId"))
                .triple(vars!("Subject"), vars!("Prop"), vars!("Value"))
                .select(vec![vars!("Subject"), vars!("Prop"), vars!("Value")])
                .using(&collection);

            or_queries.push(query);
        }

        // Combine with OR
        if !or_queries.is_empty() {
            let mut or_queries_iter = or_queries.into_iter();
            let mut query_builder = or_queries_iter.next().unwrap();
            for q in or_queries_iter {
                query_builder = query_builder.or([q]);
            }

            let final_query = query_builder.finalize();
            let json_query = final_query.to_instance(None).to_json();

            println!(
                "Multi-version query JSON: {}",
                serde_json::to_string_pretty(&json_query)?
            );

            match client.query_raw(Some(spec.clone()), json_query, None).await {
                Ok(result) => {
                    let result: WOQLResult<HashMap<String, serde_json::Value>> = result;
                    println!(
                        "Multi-version query returned {} bindings",
                        result.bindings.len()
                    );

                    // Group by subject to see versions
                    let mut subjects: std::collections::HashSet<String> =
                        std::collections::HashSet::new();
                    for binding in &result.bindings {
                        if let Some(subj) = binding.get("Subject").and_then(|s| s.as_str()) {
                            subjects.insert(subj.to_string());
                        }
                    }

                    println!(
                        "Found {} unique subjects across all versions",
                        subjects.len()
                    );
                }
                Err(e) => {
                    println!("Multi-version query failed: {}", e);
                }
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_woql_using_branch_commit_format() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;

        server
            .with_tmp_db("test_branch_format", |client, spec| async move {
                // Insert schema
                let args = DocumentInsertArgs::from(spec.clone());
                client
                    .insert_entity_schema::<PersonWithId>(args)
                    .await
                    .ok();

                let fixed_id = &format!(
                    "test_branch_format_{}",
                    chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
                );
                println!("=== Testing WOQL using() with branch/commitID format ===");

                // Create 3 versions using the new semantic API
                // Version 1: Create
                let person_v1 = PersonWithId {
                    id: EntityIDFor::new(fixed_id).unwrap(),
                    name: "Version 1 Person".to_string(),
                    age: 25,
                    email: None,
                };

                let args = DocumentInsertArgs::from(spec.clone());
                let result1 = client.create_instance(&person_v1, args).await?;
                let commit_id_1 = result1.extract_commit_id().expect("Should have commit ID");
                println!("V1: Created in commit {}", commit_id_1);

                // Version 2: Update
                let person_v2 = PersonWithId {
                    id: EntityIDFor::new(fixed_id).unwrap(),
                    name: "Version 2 Person".to_string(),
                    age: 30,
                    email: Some("v2@test.com".to_string()),
                };

                let args = DocumentInsertArgs::from(spec.clone());
                let result2 = client.update_instance(&person_v2, args).await?;
                let commit_id_2 = result2.extract_commit_id().expect("Should have commit ID");
                println!("V2: Updated in commit {}", commit_id_2);

                // Version 3: Update again
                let person_v3 = PersonWithId {
                    id: EntityIDFor::new(fixed_id).unwrap(),
                    name: "Version 3 Person".to_string(),
                    age: 35,
                    email: Some("v3@test.com".to_string()),
                };

                let args = DocumentInsertArgs::from(spec.clone());
                let result3 = client.update_instance(&person_v3, args).await?;
                let commit_id_3 = result3.extract_commit_id().expect("Should have commit ID");
                println!("V3: Updated in commit {}", commit_id_3);

                println!("\n=== Testing different using() formats ===");

                // Format 1: branch/commitID (as suggested by JS client)
                println!("\nFormat 1: branch/commitID");
                let branch_format = format!("main/{}", commit_id_1);
                test_query_format(&client, &spec, &branch_format, "branch/commitID").await?;

                // Format 2: Just the commit ID
                println!("\nFormat 2: Just commitID");
                test_query_format(&client, &spec, &commit_id_1.to_string(), "commitID only")
                    .await?;

                // Format 3: Full path as in JS docs: userName/dbName/local/commit/commitID
                println!("\nFormat 3: Full path userName/dbName/local/commit/commitID");
                let full_path = format!("{}/{}/local/commit/{}", "admin", spec.db, commit_id_1);
                test_query_format(&client, &spec, &full_path, "full path").await?;

                // Format 4: Full path with branch: userName/dbName/local/branch/commitID
                println!("\nFormat 4: Full path userName/dbName/local/branch/commitID");
                let full_branch_path =
                    format!("{}/{}/local/branch/{}", "admin", spec.db, commit_id_1);
                test_query_format(&client, &spec, &full_branch_path, "full branch path").await?;

                // Format 5: Our current format for comparison
                println!("\nFormat 5: Our current format admin/db/local/commit/commitID");
                let current_format = format!("admin/{}/local/commit/{}", spec.db, commit_id_1);
                test_query_format(&client, &spec, &current_format, "current format").await?;

                println!("\n=== Testing multi-version query with best format ===");
                // Now test querying across multiple commits with the format that works best
                test_multi_version_query(
                    &client,
                    &spec,
                    vec![commit_id_1, commit_id_2, commit_id_3],
                )
                .await?;

                Ok(())
            })
            .await
    }
}
