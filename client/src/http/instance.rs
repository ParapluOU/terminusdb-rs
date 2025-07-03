//! Strongly-typed instance operations

use {
    crate::{
        document::{DocumentInsertArgs, DocumentType, GetOpts},
        result::ResponseWithHeaders,
        spec::BranchSpec,
        TDBInsertInstanceResult, TDBInstanceDeserializer, IntoBoxedTDBInstances,
    },
    ::log::{debug, warn, error},
    anyhow::{anyhow, bail, Context},
    futures_util::StreamExt,
    std::{collections::HashMap, fmt::Debug},
    tap::{Tap, TapFallible},
    terminusdb_schema::{Instance, ToTDBInstance, ToJson},
    terminusdb_woql2::prelude::Query as Woql2Query,
    terminusdb_woql_builder::prelude::{node, vars, Var, WoqlBuilder},
    terminusdb_schema::GraphType,
};

use super::{TerminusDBModel, helpers::{dedup_instances_by_id, dump_schema, format_id}};

#[cfg(not(target_arch = "wasm32"))]
use crate::log::{CommitLogIterator, LogEntry, LogOpts};

/// Strongly-typed instance operations for the TerminusDB HTTP client
impl super::client::TerminusDBHttpClient {
    /// Checks if a strongly-typed model instance exists in the database.
    ///
    /// This is the **preferred method** for checking existence of typed models.
    /// Use this instead of `has_document` when working with structs that derive `TerminusDBModel`.
    ///
    /// # Type Parameters
    /// * `I` - A type that implements `TerminusDBModel` (derives `TerminusDBModel`)
    ///
    /// # Arguments
    /// * `model` - The strongly-typed model instance to check for
    /// * `args` - Document insertion arguments specifying the database and branch
    ///
    /// # Returns
    /// `true` if the instance exists, `false` otherwise
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel)]
    /// struct User { name: String, age: i32 }
    ///
    /// let user = User { name: "Alice".to_string(), age: 30 };
    /// let exists = client.has_instance(&user, args).await;
    /// ```
    #[pseudonym::alias(has)]
    pub async fn has_instance<I: TerminusDBModel>(
        &self,
        model: &I,
        spec: &BranchSpec,
    ) -> bool {
        match model.to_instance(None).gen_id() {
            None => {
                return false;
            }
            Some(id) => self.has_document(&id, spec).await,
        }
    }

    #[pseudonym::alias(has_id)]
    pub async fn has_instance_id<I: TerminusDBModel>(
        &self,
        model_id: &str,
        spec: &BranchSpec,
    ) -> bool {
        self.has_document(&format_id::<I>(model_id), spec).await
    }

    /// Inserts a strongly-typed model instance into the database.
    ///
    /// This is the **preferred method** for inserting typed models into TerminusDB.
    /// Use this instead of `insert_document` when working with structs that derive `TerminusDBModel`.
    ///
    /// The response includes the `TerminusDB-Data-Version` header when data is modified,
    /// which contains the commit ID for the operation.
    ///
    /// # Type Parameters
    /// * `I` - A type that implements `TerminusDBModel` (derives `TerminusDBModel`)
    ///
    /// # Arguments
    /// * `model` - The strongly-typed model instance to insert
    /// * `args` - Document insertion arguments specifying the database, branch, and options
    ///
    /// # Returns
    /// A `ResponseWithHeaders` containing:
    /// - `data`: HashMap with instance IDs and insert results
    /// - `commit_id`: Optional commit ID from the TerminusDB-Data-Version header
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel, Serialize, Deserialize)]
    /// struct User { name: String, age: i32 }
    ///
    /// let user = User { name: "Alice".to_string(), age: 30 };
    /// let result = client.insert_instance(&user, args).await?;
    /// 
    /// println!("Inserted with commit: {:?}", result.commit_id);
    /// ```
    ///
    /// # See Also
    /// - [`insert_instance_with_commit_id`](Self::insert_instance_with_commit_id) - For direct access to commit ID
    /// - [`insert_instances`](Self::insert_instances) - For bulk insertion
    #[pseudonym::alias(insert)]
    pub async fn insert_instance<I: TerminusDBModel>(
        &self,
        model: &I,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>> {
        let instance = model.to_instance(None);

        let gen_id = instance.gen_id();

        if !args.force && self.has_instance(model, &args.spec).await {
            // todo: make strongly typed ID helper
            let id = gen_id.unwrap().split("/").last().unwrap().to_string();

            warn!("not inserted because it already exists");

            // todo: if the document is configured to not use HashKey, this cannot work
            return Ok(ResponseWithHeaders::without_headers(HashMap::from([(
                id.clone(),
                TDBInsertInstanceResult::AlreadyExists(id),
            )])));
        }

        debug!("inserting instance...");

        let res = self
            .insert_instances(instance, args.clone())
            .await
            .tap_err(|_| dump_schema::<I>())?;

        dbg!(&res);

        // Get the first inserted ID from the result HashMap's values
        let inserted_tdb_id = match res.values().next() {
            Some(TDBInsertInstanceResult::Inserted(id)) => id.clone(),
            Some(TDBInsertInstanceResult::AlreadyExists(id)) => {
                // If it already existed, we might still want to verify it exists
                id.clone()
            }
            None => return Err(anyhow!("Insert operation did not return any ID")),
        };

        // assert
        self.get_document(&inserted_tdb_id, args.as_ref(), GetOpts::default())
            .await
            .expect("expected document to exist!");

        Ok(res)
    }

    /// Inserts an instance into TerminusDB and returns both the instance ID and the commit ID
    /// that created it. This is much more efficient than the previous implementation as it uses
    /// the TerminusDB-Data-Version header instead of walking the commit log.
    ///
    /// # Returns
    /// A tuple of (instance_id, commit_id) where:
    /// - instance_id: The ID of the inserted instance
    /// - commit_id: The commit ID (e.g., "ValidCommit/...") that added this instance
    pub async fn insert_instance_with_commit_id<I: TerminusDBModel>(
        &self,
        model: &I,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<(String, String)> {
        // Insert the instance and capture the header
        let insert_result = self.insert_instance(model, args.clone()).await?;
        
        // Extract the inserted instance ID
        let instance_id = match insert_result.values().next() {
            Some(TDBInsertInstanceResult::Inserted(id)) => id.clone(),
            Some(TDBInsertInstanceResult::AlreadyExists(id)) => {
                // For already existing instances, we need to find their commit the old way
                debug!("Instance {} already exists, falling back to commit log search", id);
                let commit_id = self.find_commit_for_instance::<I>(id, &args.spec).await
                    .context(format!("Failed to find commit for existing instance {}", id))?;
                return Ok((id.clone(), commit_id));
            }
            None => return Err(anyhow!("Insert operation did not return any ID")),
        };
        
        // Extract commit ID from the TerminusDB-Data-Version header
        let commit_id = match &insert_result.commit_id {
            Some(header_value) => {
                // According to docs: "colon separated value" where commit ID is on the right side
                // Format is typically "branch:COMMIT_ID", we want just the COMMIT_ID part
                if let Some(commit_id) = header_value.split(':').last() {
                    commit_id.to_string()
                } else {
                    return Err(anyhow!(
                        "Invalid TerminusDB-Data-Version header format: {}. Expected colon-separated value.",
                        header_value
                    ));
                }
            }
            None => {
                // Fallback to the old method if header is not present
                warn!("TerminusDB-Data-Version header not found, falling back to commit log search");
                self.find_commit_for_instance::<I>(&instance_id, &args.spec).await
                    .context(format!("Failed to find commit for instance {}", instance_id))?
            }
        };
        
        Ok((instance_id, commit_id))
    }

    /// Finds the commit that added a specific instance by walking through the commit log
    /// and checking each commit to see if it added the given instance ID.
    ///
    /// Uses the existing `all_commit_created_entity_ids_any_type` helper to get all
    /// entity IDs created in each commit and checks if our target instance is in that list.
    #[cfg(not(target_arch = "wasm32"))]
    async fn find_commit_for_instance<I: TerminusDBModel>(
        &self,
        instance_id: &str,
        spec: &BranchSpec,
    ) -> anyhow::Result<String> {
        debug!("=== find_commit_for_instance START ===");
        debug!("Looking for commit that created instance: {}", instance_id);
        debug!("Using branch spec: db={}, branch={:?}", spec.db, spec.branch);
        
        if !self.has_document(&instance_id, spec).await {
            return bail!("Instance {} does not exist", instance_id);
        }

        // Wrap the entire search operation with a timeout
        let search_future = async {
            // Create a commit log iterator with a smaller batch size
            let log_opts = LogOpts {
                count: Some(10), // Reduce from default to 10 commits per batch
                ..Default::default()
            };
            let mut log_iter = self.log_iter(spec.clone(), log_opts).await;
            
            let mut commits_checked = 0;
            let max_commits = 10; // Reduced from 100 to 10
            
            // Walk through commits until we find the one containing our instance
            while let Some(log_entry_result) = log_iter.next().await {
                debug!("Getting next log entry from iterator...");
                let log_entry = log_entry_result?;
                commits_checked += 1;
                
                debug!("Checking commit {} ({}/{})...", log_entry.id, commits_checked, max_commits);
                debug!("  Commit timestamp: {:?}", log_entry.timestamp);
                debug!("  Commit message: {:?}", log_entry.message);
                
                // Get all entity IDs created in this commit (any type)
                debug!("Calling all_commit_created_entity_ids_any_type for commit {}...", log_entry.id);
                let start_time = std::time::Instant::now();
                match self.all_commit_created_entity_ids_any_type(spec, &log_entry).await {
                    Ok(created_ids) => {
                        let elapsed = start_time.elapsed();
                        debug!("Query completed in {:?}", elapsed);
                        debug!("Commit {} has {} entities", log_entry.id, created_ids.len());
                        if created_ids.len() > 0 {
                            debug!("First few entity IDs: {:?}", created_ids.iter().take(5).collect::<Vec<_>>());
                        }
                        
                        // Check if our instance ID is in the list of created entities
                        // Need to check both with and without schema prefix since insert_instance 
                        // returns IDs without prefix but commits store them with prefix
                        debug!("Checking if instance_id '{}' is in created_ids...", instance_id);
                        let id_matches = created_ids.iter().any(|id| {
                            let matches = id == instance_id || id.split('/').last() == Some(instance_id);
                            if matches {
                                debug!("  Found match: {}", id);
                            }
                            matches
                        });
                        
                        if id_matches {
                            debug!("✓ Found instance {} in commit {}", instance_id, log_entry.id);
                            return Ok(log_entry.id.clone());
                        } else {
                            debug!("Instance not found in this commit, continuing...");
                        }
                    }
                    Err(e) => {
                        let elapsed = start_time.elapsed();
                        warn!("Failed to query commit {} after {:?}: {}", log_entry.id, elapsed, e);
                        // Continue to next commit instead of failing entirely
                    }
                }
                
                // Early exit if we've checked enough commits
                if commits_checked >= max_commits {
                    warn!("Reached max commit limit ({}) without finding instance {}", max_commits, instance_id);
                    break;
                }
            }
            
            Err(anyhow!("Could not find commit for instance {} after checking {} commits", instance_id, commits_checked))
        };
        
        // Apply overall timeout of 60 seconds
        let timeout_duration = std::time::Duration::from_secs(30);
        match tokio::time::timeout(timeout_duration, search_future).await {
            Ok(result) => {
                debug!("=== find_commit_for_instance END === Result: {:?}", result.as_ref().map(|s| s.as_str()));
                result
            },
            Err(_) => {
                error!("Timeout: find_commit_for_instance exceeded 60 seconds for instance {}", instance_id);
                Err(anyhow!("Operation timed out after 60 seconds while searching for commit containing instance {}", instance_id))
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    async fn find_commit_for_instance<I: TerminusDBModel>(
        &self,
        _instance_id: &str,
        _spec: &BranchSpec,
    ) -> anyhow::Result<String> {
        // WASM stub - implement as needed
        Err(anyhow!("find_commit_for_instance not implemented for WASM"))
    }

    /// Helper method to get all entity IDs created in a commit, regardless of type
    #[cfg(not(target_arch = "wasm32"))]
    async fn all_commit_created_entity_ids_any_type(
        &self,
        spec: &BranchSpec,
        commit: &LogEntry,
    ) -> anyhow::Result<Vec<crate::EntityID>> {
        let commit_collection = format!("commit/{}", &commit.identifier);
        let db_collection = format!("{}/{}", &self.org, &spec.db);
        
        debug!("=== all_commit_created_entity_ids_any_type START ===");
        debug!("Querying commit {} for added entities", &commit.identifier);
        debug!("Using collections: commit={}, db={}", &commit_collection, &db_collection);
        
        let id_var = vars!("id");
        let type_var = vars!("type");
        
        // Query for all added triples (any type)
        let query = WoqlBuilder::new()
            .added_triple(
                id_var.clone(),                 // subject: variable "id"
                "rdf:type",                     // predicate: "rdf:type"
                type_var.clone(),               // object: any type
                GraphType::Instance.into(),
            )
            .using(commit_collection)
            .using(db_collection)
            .limit(1000)
            .finalize();

        let json_query = query.to_instance(None).to_json();
        
        debug!("Running WOQL query: {}", serde_json::to_string_pretty(&json_query).unwrap_or_default());
        
        // Add a timeout to prevent hanging forever
        let query_future = self.query_raw(Some(spec.clone()), json_query);
        let timeout_duration = std::time::Duration::from_secs(30);
        
        let res: crate::WOQLResult<serde_json::Value> = match tokio::time::timeout(timeout_duration, query_future).await {
            Ok(result) => result?,
            Err(_) => {
                error!("Query timed out after 30 seconds for commit {}", &commit.identifier);
                return Err(anyhow!("Query timed out"));
            }
        };

        debug!("Query returned {} bindings", res.bindings.len());

        let err = format!("failed to deserialize from Value: {:#?}", &res);

        #[derive(serde::Deserialize)]
        struct ObjectFormat {
            pub id: String,
        }

        let result: Vec<crate::EntityID> = res
            .bindings
            .into_iter()
            .map(|bind| serde_json::from_value::<ObjectFormat>(bind))
            .collect::<Result<Vec<_>, _>>()
            .context(err)?
            .into_iter()
            .map(|obj| obj.id)
            .collect();
        
        debug!("=== all_commit_created_entity_ids_any_type END === Found {} entities", result.len());
        Ok(result)
    }

    #[cfg(target_arch = "wasm32")]
    async fn all_commit_created_entity_ids_any_type(
        &self,
        _spec: &BranchSpec,
        _commit: &LogEntry,
    ) -> anyhow::Result<Vec<crate::EntityID>> {
        // WASM stub - implement as needed
        Err(anyhow!("all_commit_created_entity_ids_any_type not implemented for WASM"))
    }

    /// Inserts multiple strongly-typed model instances into the database.
    ///
    /// This is the **preferred method** for bulk insertion of typed models into TerminusDB.
    /// Use this instead of [`insert_documents`](Self::insert_documents) when working with
    /// multiple structs that derive `TerminusDBModel`.
    ///
    /// # Arguments
    /// * `models` - Collection of models that can be converted to TerminusDB instances
    /// * `args` - Document insertion arguments (DocumentType will be set to Instance automatically)
    ///
    /// # Returns
    /// A `ResponseWithHeaders` containing:
    /// - `data`: HashMap with instance IDs and insert results
    /// - `commit_id`: Optional commit ID from the TerminusDB-Data-Version header
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel, Serialize, Deserialize)]
    /// struct User { name: String, age: i32 }
    ///
    /// let users = vec![
    ///     User { name: "Alice".to_string(), age: 30 },
    ///     User { name: "Bob".to_string(), age: 25 },
    /// ];
    /// let result = client.insert_instances(users, args).await?;
    /// ```
    ///
    /// # See Also
    /// - [`insert_instance`](Self::insert_instance) - For single instance insertion
    #[pseudonym::alias(insert_many)]
    pub async fn insert_instances(
        &self,
        models: impl IntoBoxedTDBInstances,
        mut args: DocumentInsertArgs,
    ) -> anyhow::Result<ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>> {
        args.ty = DocumentType::Instance;

        // let mut instances = models
        //     .into_iter()
        //     .map(|m| m.to_instance_tree_flatten(true))
        //     .flatten()
        //     .collect::<Vec<_>>();

        let mut instances = models.into_boxed().to_instance_tree_flatten(true);

        for instance in &mut instances {
            instance.set_random_key_prefix();
            // instance.ref_props = false; // todo: make configurable
            instance.capture = true;
        }

        // kick out all references because they will lead to schema failure errors on insertion
        instances.retain(|i| !i.is_reference());

        let mut models = instances.iter().collect();

        // dedup_instances_by_id(&mut models);

        self.insert_documents(models, args).await
    }

    /// Retrieves and deserializes a strongly-typed model instance from the database.
    ///
    /// This is the **preferred method** for retrieving typed models from TerminusDB.
    /// Use this instead of [`get_document`](Self::get_document) when working with
    /// structs that implement `ToTDBInstance`.
    ///
    /// # Type Parameters
    /// * `Target` - The type to deserialize the instance into (implements `ToTDBInstance`)
    ///
    /// # Arguments
    /// * `id` - The instance ID (number only, no schema class prefix)
    /// * `spec` - Branch specification (for time-travel, use commit-specific specs)
    /// * `deserializer` - Instance deserializer for converting from TerminusDB format
    ///
    /// # Returns
    /// The deserialized instance of type `Target`
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel)]
    /// struct User { name: String, age: i32 }
    ///
    /// let mut deserializer = DefaultDeserializer::new();
    /// let user: User = client.get_instance("12345", &spec, &mut deserializer).await?;
    /// ```
    ///
    /// # Time Travel
    /// ```rust
    /// // Retrieve from a specific commit
    /// let past_spec = BranchSpec::from("main/local/commit/abc123");
    /// let old_user: User = client.get_instance("12345", &past_spec, &mut deserializer).await?;
    /// ```
    #[pseudonym::alias(get)]
    pub async fn get_instance<Target: ToTDBInstance>(
        &self,
        // number only, no schema class prefix
        id: &str,
        spec: &BranchSpec,
        mut deserializer: &mut impl TDBInstanceDeserializer<Target>,
    ) -> anyhow::Result<Target>
    where
        Target:,
    {
        let doc_id = format_id::<Target>(id);

        let json_instance_doc = self.get_document(&doc_id, spec, Default::default()).await?;

        let res = deserializer.from_instance(json_instance_doc.clone());

        match res {
            Ok(t) => Ok(t),
            Err(err) => Err(err).context(format!(
                "TerminusHTTPClient failed to deserialize Instance. See: {}",
                super::helpers::dump_json(&json_instance_doc).display()
            )),
        }
    }
}