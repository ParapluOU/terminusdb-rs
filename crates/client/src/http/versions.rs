//! Instance version retrieval implementation

use {
    crate::{spec::BranchSpec, CommitId, TDBInstanceDeserializer, WOQLResult},
    ::tracing::{debug, instrument, warn},
    anyhow::{anyhow, Context},
    std::collections::HashMap,
    terminusdb_schema::{ToJson, ToTDBInstance},
    terminusdb_woql_builder::prelude::{node, vars, WoqlBuilder},
};

use super::{helpers::format_id, TerminusDBModel};

impl super::client::TerminusDBHttpClient {
    /// Retrieves specific versions of an instance by their commit IDs.
    ///
    /// This method provides time-travel functionality by fetching specific historical versions
    /// of an instance using a single efficient WOQL query.
    ///
    /// # Type Parameters
    /// * `T` - The type to deserialize the instances into (implements `TerminusDBModel`)
    ///
    /// # Arguments
    /// * `instance_id` - The instance ID (without type prefix, e.g., "abc123")
    /// * `spec` - Branch specification (branch to query history from)
    /// * `commit_ids` - List of specific commit IDs to retrieve versions from
    /// * `deserializer` - Instance deserializer for converting from TerminusDB format
    ///
    /// # Returns
    /// A vector of tuples containing (instance, commit_id) pairs for the requested commits
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel, Serialize, Deserialize)]
    /// struct Person { name: String, age: i32 }
    ///
    /// let commit_ids = vec!["abc123".to_string(), "def456".to_string()];
    /// let mut deserializer = DefaultDeserializer::new();
    /// let versions = client.get_instance_versions::<Person>(
    ///     "personid123",
    ///     &branch_spec,
    ///     commit_ids,
    ///     &mut deserializer
    /// ).await?;
    ///
    /// for (person, commit_id) in versions {
    ///     println!("Commit {}: {} (age {})", commit_id, person.name, person.age);
    /// }
    /// ```
    #[instrument(
        name = "terminus.versions.get_instance_versions",
        skip(self, commit_ids, deserializer),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %T::schema_name(),
            instance_id = %instance_id,
            commit_count = commit_ids.len()
        ),
        err
    )]
    pub async fn get_instance_versions<T: TerminusDBModel>(
        &self,
        instance_id: &str,
        spec: &BranchSpec,
        commit_ids: Vec<CommitId>,
        deserializer: &mut impl TDBInstanceDeserializer<T>,
    ) -> anyhow::Result<Vec<(T, CommitId)>> {
        debug!(
            "Attempting WOQL-based instance version retrieval for {} with {} specific commits",
            instance_id,
            commit_ids.len()
        );

        if commit_ids.is_empty() {
            return Ok(vec![]);
        }

        // Build a WOQL query that uses OR to combine queries across commits
        let full_id = format_id::<T>(instance_id);

        // Build individual queries for each commit
        let mut commit_queries = Vec::new();
        let mut commit_map = HashMap::new();

        for commit_id in commit_ids {
            // Use the correct format: admin/{db}/local/commit/{commitID}
            let collection = format!("{}/{}/local/commit/{}", self.org, spec.db, &commit_id);

            // Create a unique variable for this commit's document
            let doc_var = vars!(format!("Doc_{}", commit_id.as_str().replace('/', "_")));
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

        debug!(
            "Executing combined WOQL query for {} commits",
            commit_map.len()
        );

        // Execute the query
        match self.query_raw(Some(spec.clone()), json_query, None).await {
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
                                    warn!(
                                        "Failed to deserialize version from commit {}: {}",
                                        commit_id, e
                                    );
                                }
                            }
                        }
                    }
                }

                debug!(
                    "Successfully retrieved {} versions via WOQL",
                    versions.len()
                );
                Ok(versions)
            }
            Err(e) => {
                warn!("WOQL query failed: {}", e);
                Err(e)
            }
        }
    }

    /// Lists all versions of a specific instance across its commit history.
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
    /// let versions = client.list_instance_versions::<Person>(
    ///     "abc123randomkey",
    ///     &branch_spec,
    ///     &mut deserializer
    /// ).await?;
    ///
    /// for (person, commit_id) in versions {
    ///     println!("Commit {}: {} (age {})", commit_id, person.name, person.age);
    /// }
    /// ```
    #[instrument(
        name = "terminus.versions.list_instance_versions",
        skip(self, deserializer),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %T::schema_name(),
            instance_id = %instance_id
        ),
        err
    )]
    pub async fn list_instance_versions<T: TerminusDBModel>(
        &self,
        instance_id: &str,
        spec: &BranchSpec,
        deserializer: &mut impl TDBInstanceDeserializer<T>,
    ) -> anyhow::Result<Vec<(T, CommitId)>> {
        debug!("Listing all versions for instance {}", instance_id);

        // First get the commit history
        let history = self
            .get_instance_history::<T>(instance_id, spec, None)
            .await?;
        if history.is_empty() {
            return Ok(vec![]);
        }

        debug!("Found {} commits in history", history.len());

        // Extract commit IDs from history
        let commit_ids: Vec<CommitId> = history.into_iter().map(|entry| entry.identifier).collect();

        // Use get_instance_versions with the full list of commit IDs
        self.get_instance_versions(instance_id, spec, commit_ids, deserializer)
            .await
    }

    /// Retrieves specific versions for multiple instances in a single query.
    ///
    /// This method efficiently fetches different versions for different documents
    /// using a single WOQL query, ideal when each document needs specific commits.
    ///
    /// # Type Parameters
    /// * `T` - The type to deserialize the instances into (implements `TerminusDBModel`)
    ///
    /// # Arguments
    /// * `queries` - Vector of tuples containing (instance_id, commit_ids) pairs
    /// * `spec` - Branch specification (branch to query history from)
    /// * `deserializer` - Instance deserializer for converting from TerminusDB format
    ///
    /// # Returns
    /// A HashMap mapping each instance_id to its vector of (instance, commit_id) pairs
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel, Serialize, Deserialize)]
    /// struct Product { name: String, price: f64 }
    ///
    /// let queries = vec![
    ///     ("product1", vec!["commit_a".to_string(), "commit_b".to_string()]),
    ///     ("product2", vec!["commit_c".to_string()]),
    /// ];
    ///
    /// let mut deserializer = DefaultDeserializer::new();
    /// let versions = client.get_multiple_instance_versions::<Product>(
    ///     queries,
    ///     &branch_spec,
    ///     &mut deserializer
    /// ).await?;
    ///
    /// for (product_id, versions) in versions {
    ///     println!("Product {}: {} versions", product_id, versions.len());
    /// }
    /// ```
    #[instrument(
        name = "terminus.versions.get_multiple_instance_versions",
        skip(self, queries, deserializer),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %T::schema_name(),
            document_count = queries.len()
        ),
        err
    )]
    #[pseudonym::alias(get_instances_versions)]
    pub async fn get_multiple_instance_versions<T: TerminusDBModel>(
        &self,
        queries: Vec<(&str, Vec<CommitId>)>,
        spec: &BranchSpec,
        deserializer: &mut impl TDBInstanceDeserializer<T>,
    ) -> anyhow::Result<HashMap<String, Vec<(T, CommitId)>>> {
        debug!(
            "Attempting multi-document WOQL version retrieval for {} documents",
            queries.len()
        );

        if queries.is_empty() {
            return Ok(HashMap::new());
        }

        // Build individual queries for each document×commit combination
        let mut all_queries = Vec::new();
        let mut var_map: HashMap<String, (String, CommitId)> = HashMap::new();

        for (instance_id, commit_ids) in queries {
            if commit_ids.is_empty() {
                continue;
            }

            let full_id = format_id::<T>(instance_id);

            for commit_id in commit_ids {
                // Use the correct format: org/db/local/commit/{commitID}
                let collection = format!("{}/{}/local/commit/{}", self.org, spec.db, &commit_id);

                // Create unique variables for this document×commit combination
                let safe_instance_id = instance_id.replace('/', "_").replace('-', "_");
                let safe_commit_id = commit_id.as_str().replace('/', "_").replace('-', "_");
                let doc_var = vars!(format!("Doc_{}_{}", safe_instance_id, safe_commit_id));

                // Track which document and commit this variable represents
                var_map.insert(
                    doc_var.to_string(),
                    (instance_id.to_string(), commit_id.clone()),
                );

                let query = WoqlBuilder::new()
                    .triple(node(&full_id), "rdf:type", vars!("Type"))
                    .read_document(node(&full_id), doc_var.clone())
                    .select(vec![doc_var])
                    .using(&collection);

                all_queries.push(query);
            }
        }

        if all_queries.is_empty() {
            return Ok(HashMap::new());
        }

        // Combine all queries with OR
        let mut queries_iter = all_queries.into_iter();
        let mut combined_query = queries_iter.next().unwrap();
        for query in queries_iter {
            combined_query = combined_query.or([query]);
        }

        let final_query = combined_query.finalize();
        let json_query = final_query.to_instance(None).to_json();

        debug!(
            "Executing combined WOQL query for {} document×commit combinations",
            var_map.len()
        );

        // Execute the query
        match self.query_raw(Some(spec.clone()), json_query, None).await {
            Ok(result) => {
                let result: WOQLResult<serde_json::Value> = result;
                debug!("WOQL query returned {} bindings", result.bindings.len());

                // Process results and group by document ID
                let mut results: HashMap<String, Vec<(T, CommitId)>> = HashMap::new();

                for binding in result.bindings {
                    // Find which doc variable has data
                    for (var_name, (doc_id, commit_id)) in &var_map {
                        if let Some(doc_json) = binding.get(var_name) {
                            // Deserialize the document
                            match deserializer.from_instance(doc_json.clone()) {
                                Ok(instance) => {
                                    results
                                        .entry(doc_id.clone())
                                        .or_insert_with(Vec::new)
                                        .push((instance, commit_id.clone()));
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to deserialize {} from commit {}: {}",
                                        doc_id, commit_id, e
                                    );
                                }
                            }
                        }
                    }
                }

                debug!(
                    "Successfully retrieved versions for {} documents",
                    results.len()
                );
                Ok(results)
            }
            Err(e) => {
                warn!("Multi-document WOQL query failed: {}", e);
                Err(e)
            }
        }
    }

    /// Lists all versions for multiple instances in a single query.
    ///
    /// This convenience method fetches the complete version history for multiple
    /// documents efficiently using a single WOQL query.
    ///
    /// # Type Parameters
    /// * `T` - The type to deserialize the instances into (implements `TerminusDBModel`)
    ///
    /// # Arguments
    /// * `instance_ids` - List of instance IDs to get versions for
    /// * `spec` - Branch specification (branch to query history from)
    /// * `deserializer` - Instance deserializer for converting from TerminusDB format
    ///
    /// # Returns
    /// A HashMap mapping each instance_id to its vector of (instance, commit_id) pairs
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel, Serialize, Deserialize)]
    /// struct Product { name: String, price: f64 }
    ///
    /// let instance_ids = vec!["product1", "product2", "product3"];
    /// let mut deserializer = DefaultDeserializer::new();
    /// let all_versions = client.list_multiple_instance_versions::<Product>(
    ///     instance_ids,
    ///     &branch_spec,
    ///     &mut deserializer
    /// ).await?;
    ///
    /// for (product_id, versions) in all_versions {
    ///     println!("Product {} has {} versions", product_id, versions.len());
    /// }
    /// ```
    #[instrument(
        name = "terminus.versions.list_multiple_instance_versions",
        skip(self, instance_ids, deserializer),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %T::schema_name(),
            instance_count = instance_ids.len()
        ),
        err
    )]
    #[pseudonym::alias(list_instances_versions)]
    pub async fn list_multiple_instance_versions<T: TerminusDBModel>(
        &self,
        instance_ids: Vec<&str>,
        spec: &BranchSpec,
        deserializer: &mut impl TDBInstanceDeserializer<T>,
    ) -> anyhow::Result<HashMap<String, Vec<(T, CommitId)>>> {
        debug!("Listing all versions for {} instances", instance_ids.len());

        if instance_ids.is_empty() {
            return Ok(HashMap::new());
        }

        // Get history for each instance and build queries structure
        let mut queries: Vec<(&str, Vec<CommitId>)> = Vec::new();

        for instance_id in &instance_ids {
            // Get the commit history for this instance
            let history = self
                .get_instance_history::<T>(instance_id, spec, None)
                .await?;

            if !history.is_empty() {
                let commit_ids: Vec<CommitId> =
                    history.into_iter().map(|entry| entry.identifier).collect();

                queries.push((instance_id, commit_ids));
            }
        }

        // Use get_multiple_instance_versions with the collected data
        self.get_multiple_instance_versions(queries, spec, deserializer)
            .await
    }
}
