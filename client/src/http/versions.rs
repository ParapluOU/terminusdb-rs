//! Instance version retrieval implementation

use {
    crate::{
        spec::BranchSpec,
        TDBInstanceDeserializer,
        WOQLResult,
    },
    ::log::{debug, warn},
    anyhow::{anyhow, Context},
    std::collections::HashMap,
    terminusdb_schema::{ToTDBInstance, ToJson},
    terminusdb_woql_builder::prelude::{node, vars, WoqlBuilder},
};

use super::{TerminusDBModel, helpers::format_id};

impl super::client::TerminusDBHttpClient {
    /// Retrieves all versions of a specific instance across its commit history.
    ///
    /// This method provides time-travel functionality by fetching all historical versions
    /// of an instance that share the same ID using a single efficient WOQL query.
    /// It returns versions in reverse chronological order (most recent first).
    ///
    /// # Type Parameters
    /// * `T` - The type to deserialize the instances into (implements `TerminusDBModel`)
    ///
    /// # Arguments
    /// * `instance_id` - The instance ID (without type prefix, e.g., "abc123")
    /// * `spec` - Branch specification (branch to query history from)
    /// * `deserializer` - Instance deserializer for converting from TerminusDB format
    ///
    /// # Returns
    /// A vector of tuples containing (instance, commit_id) pairs representing the full version history
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel, Serialize, Deserialize)]
    /// struct Person { name: String, age: i32 }
    ///
    /// let mut deserializer = DefaultDeserializer::new();
    /// let versions = client.get_instance_versions::<Person>(
    ///     "abc123randomkey",
    ///     &branch_spec,
    ///     &mut deserializer
    /// ).await?;
    ///
    /// for (person, commit_id) in versions {
    ///     println!("Commit {}: {} (age {})", commit_id, person.name, person.age);
    /// }
    /// ```
    pub async fn get_instance_versions<T: TerminusDBModel>(
        &self,
        instance_id: &str,
        spec: &BranchSpec,
        deserializer: &mut impl TDBInstanceDeserializer<T>,
    ) -> anyhow::Result<Vec<(T, String)>> {
        debug!("Attempting WOQL-based instance version retrieval for {}", instance_id);
        
        // First get the commit history
        let history = self.get_instance_history::<T>(instance_id, spec, None).await?;
        if history.is_empty() {
            return Ok(vec![]);
        }
        
        debug!("Found {} commits in history", history.len());
        
        // Build a WOQL query that uses OR to combine queries across commits
        let full_id = format_id::<T>(instance_id);
        
        // Build individual queries for each commit
        let mut commit_queries = Vec::new();
        let mut commit_map = HashMap::new();
        
        for entry in history {
            let commit_id = entry.identifier.clone();
            // Use the correct format: admin/{db}/local/commit/{commitID}
            let collection = format!("admin/{}/local/commit/{}", spec.db, &commit_id);
            
            // Create a unique variable for this commit's document
            let doc_var = vars!(format!("Doc_{}", commit_id.replace('/', "_")));
            commit_map.insert(doc_var.to_string(), commit_id.clone());
            
            let query = WoqlBuilder::new()
                .triple(node(&full_id), "rdf:type", vars!("Type"))
                .read_document(node(&full_id), doc_var.clone())
                .select(vec![doc_var])
                .using(&collection);
            
            commit_queries.push(query);
        }
        
        if commit_queries.is_empty() {
            return Ok(vec![]);
        }
        
        // Combine all queries with OR
        let mut commit_queries_iter = commit_queries.into_iter();
        let mut combined_query = commit_queries_iter.next().unwrap();
        for query in commit_queries_iter {
            combined_query = combined_query.or([query]);
        }
        
        let final_query = combined_query.finalize();
        let json_query = final_query.to_instance(None).to_json();
        
        debug!("Executing combined WOQL query for {} commits", commit_map.len());
        
        // Execute the query
        match self.query_raw(Some(spec.clone()), json_query).await {
            Ok(result) => {
                let result: WOQLResult<serde_json::Value> = result;
                debug!("WOQL query returned {} bindings", result.bindings.len());
                
                // Process results
                let mut versions = Vec::new();
                
                for binding in result.bindings {
                    // Find which doc variable has data
                    for (var_name, commit_id) in &commit_map {
                        if let Some(doc_json) = binding.get(var_name) {
                            // Deserialize the document
                            match deserializer.from_instance(doc_json.clone()) {
                                Ok(instance) => {
                                    versions.push((instance, commit_id.clone()));
                                }
                                Err(e) => {
                                    warn!("Failed to deserialize version from commit {}: {}", commit_id, e);
                                }
                            }
                        }
                    }
                }
                
                debug!("Successfully retrieved {} versions via WOQL", versions.len());
                Ok(versions)
            }
            Err(e) => {
                warn!("WOQL query failed: {}", e);
                Err(e)
            }
        }
    }
    
    /// Test method to verify which using() format works
    pub async fn test_using_formats(
        &self,
        spec: &BranchSpec,
        commit_id: &str,
    ) -> anyhow::Result<()> {
        println!("\n=== Testing WOQL using() formats ===");
        
        let formats = vec![
            ("branch/commitID", format!("main/{}", commit_id)),
            ("just commitID", commit_id.to_string()),
            ("commit/commitID", format!("commit/{}", commit_id)),
            ("full path", format!("admin/{}/local/commit/{}", spec.db, commit_id)),
            ("full branch path", format!("admin/{}/local/branch/{}", spec.db, commit_id)),
        ];
        
        for (name, collection) in formats {
            println!("\nTesting format '{}': {}", name, collection);
            
            let query = WoqlBuilder::new()
                .triple(vars!("S"), vars!("P"), vars!("O"))
                .select(vec![vars!("S")])
                .using(&collection)
                .limit(1)
                .finalize();
            
            let json_query = query.to_instance(None).to_json();
            
            match self.query_raw(Some(spec.clone()), json_query).await {
                Ok(result) => {
                    let result: WOQLResult<serde_json::Value> = result;
                    println!("  ✓ Success! Found {} results", result.bindings.len());
                }
                Err(e) => {
                    println!("  ✗ Failed: {}", e);
                }
            }
        }
        
        Ok(())
    }
}