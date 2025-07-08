//! Strongly-typed instance operations

use {
    crate::{
        document::{CommitHistoryEntry, DocumentHistoryParams, DocumentInsertArgs, DocumentType, GetOpts},
        http::document::DeleteOpts,
        result::ResponseWithHeaders,
        spec::BranchSpec,
        TDBInsertInstanceResult, TDBInstanceDeserializer, IntoBoxedTDBInstances,
    },
    ::log::{debug, warn, error},
    anyhow::{anyhow, bail, Context},
    futures_util::StreamExt,
    std::{collections::HashMap, fmt::Debug},
    tap::{Tap, TapFallible},
    terminusdb_schema::{Instance, ToTDBInstance, ToTDBInstances, ToJson},
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

        // dbg!(&res);

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

    /// Inserts an instance into TerminusDB and returns a structured result with the root entity
    /// clearly identified, along with the commit ID that created it. This method properly handles
    /// models with sub-entities by tracking which ID is the root.
    ///
    /// # Returns
    /// A tuple of (InsertInstanceResult, commit_id) where:
    /// - InsertInstanceResult: Contains the root ID, sub-entity results, and all results
    /// - commit_id: The commit ID (e.g., "ValidCommit/...") that added this instance
    pub async fn insert_instance_with_commit_id<I: TerminusDBModel>(
        &self,
        model: &I,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<(crate::InsertInstanceResult, String)> {
        // Generate the instance tree to understand the structure
        let instance = model.to_instance(None);
        let instance_tree = instance.to_instance_tree_flatten(true);
        
        // The root instance is always the first in the tree
        let root_instance = instance_tree.first()
            .ok_or_else(|| anyhow!("Instance tree is empty"))?;
        
        let root_id = root_instance.gen_id()
            .ok_or_else(|| anyhow!("Cannot generate ID for root instance"))?;
        
        // Insert the instance
        let insert_response = self.insert_instance(model, args.clone()).await?;
        
        // Find the actual root ID in the results (it might have a URI prefix)
        let actual_root_id = insert_response.keys()
            .find(|k| k.ends_with(&root_id) || k.split('/').last() == Some(&root_id))
            .cloned()
            .ok_or_else(|| anyhow!("Could not find root instance ID in insert results"))?;
        
        // Create structured result
        let structured_result = crate::InsertInstanceResult::new(
            (*insert_response).clone(),
            actual_root_id.clone()
        )?;
        
        // Handle commit ID based on whether instance was inserted or already existed
        let commit_id = match &structured_result.root_result {
            TDBInsertInstanceResult::Inserted(_) => {
                // Extract from header for newly inserted instances
                insert_response.extract_commit_id()
                    .ok_or_else(|| anyhow!("TerminusDB-Data-Version header not found or invalid format"))?
            },
            TDBInsertInstanceResult::AlreadyExists(id) => {
                // For already existing instances, fall back to commit log search
                debug!("Instance {} already exists, falling back to commit log search", id);
                let short_id = id.split('/').last().unwrap_or(id);
                self.get_latest_version::<I>(short_id, &args.spec).await
                    .context(format!("Failed to find commit for existing instance {}", id))?
            }
        };
        
        Ok((structured_result, commit_id))
    }

    /// Creates a new instance in the database using POST.
    ///
    /// This method uses the POST endpoint which will fail if the instance already exists.
    /// Use this when you want to ensure you're creating a new instance, not updating an existing one.
    ///
    /// # Type Parameters
    /// * `I` - A type that implements `TerminusDBModel` (derives `TerminusDBModel`)
    ///
    /// # Arguments
    /// * `model` - The strongly-typed model instance to create
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
    /// let result = client.create_instance(&user, args).await?;
    /// ```
    #[pseudonym::alias(create)]
    pub async fn create_instance<I: TerminusDBModel>(
        &self,
        model: &I,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>> {
        let instance = model.to_instance(None);
        let instances = instance.to_instance_tree_flatten(true)
            .into_iter()
            .map(|mut i| {
                i.set_random_key_prefix();
                i.capture = true;
                i
            })
            .filter(|i| !i.is_reference())
            .collect::<Vec<_>>();

        let models = instances.iter().collect();
        self.post_documents(models, args).await
    }

    /// Updates an existing instance in the database using PUT without create.
    ///
    /// This method uses the PUT endpoint without create=true, which will fail if the instance doesn't exist.
    /// Use this when you want to update an existing instance and ensure it already exists.
    ///
    /// # Type Parameters
    /// * `I` - A type that implements `TerminusDBModel` (derives `TerminusDBModel`)
    ///
    /// # Arguments
    /// * `model` - The strongly-typed model instance to update
    /// * `args` - Document insertion arguments specifying the database, branch, and options
    ///
    /// # Returns
    /// A `ResponseWithHeaders` containing:
    /// - `data`: HashMap with instance IDs and update results
    /// - `commit_id`: Optional commit ID from the TerminusDB-Data-Version header
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel, Serialize, Deserialize)]
    /// struct User { name: String, age: i32 }
    ///
    /// let user = User { name: "Alice Updated".to_string(), age: 31 };
    /// let result = client.update_instance(&user, args).await?;
    /// ```
    #[pseudonym::alias(update)]
    pub async fn update_instance<I: TerminusDBModel>(
        &self,
        model: &I,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>> {
        let instance = model.to_instance(None);
        
        // Check if instance has an ID
        if instance.gen_id().is_none() {
            return Err(anyhow!("Cannot update instance without an ID"));
        }

        let instances = instance.to_instance_tree_flatten(true)
            .into_iter()
            .map(|mut i| {
                i.set_random_key_prefix();
                i.capture = true;
                i
            })
            .filter(|i| !i.is_reference())
            .collect::<Vec<_>>();

        let models = instances.iter().collect();
        self.put_documents(models, args).await
    }

    /// Replaces an existing instance in the database.
    ///
    /// This is an alias for [`update_instance`](Self::update_instance).
    ///
    /// # Type Parameters
    /// * `I` - A type that implements `TerminusDBModel` (derives `TerminusDBModel`)
    ///
    /// # Arguments
    /// * `model` - The strongly-typed model instance to replace
    /// * `args` - Document insertion arguments specifying the database, branch, and options
    ///
    /// # Returns
    /// A `ResponseWithHeaders` containing:
    /// - `data`: HashMap with instance IDs and update results
    /// - `commit_id`: Optional commit ID from the TerminusDB-Data-Version header
    #[pseudonym::alias(replace)]
    pub async fn replace_instance<I: TerminusDBModel>(
        &self,
        model: &I,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>> {
        self.update_instance(model, args).await
    }

    /// Saves an instance to the database, creating it if it doesn't exist or updating if it does.
    ///
    /// This method first attempts to create the instance using POST. If that fails because
    /// the instance already exists, it falls back to updating using PUT.
    ///
    /// # Type Parameters
    /// * `I` - A type that implements `TerminusDBModel` (derives `TerminusDBModel`)
    ///
    /// # Arguments
    /// * `model` - The strongly-typed model instance to save
    /// * `args` - Document insertion arguments specifying the database, branch, and options
    ///
    /// # Returns
    /// A `ResponseWithHeaders` containing:
    /// - `data`: HashMap with instance IDs and save results
    /// - `commit_id`: Optional commit ID from the TerminusDB-Data-Version header
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel, Serialize, Deserialize)]
    /// struct User { name: String, age: i32 }
    ///
    /// let user = User { name: "Alice".to_string(), age: 30 };
    /// // Works whether user exists or not
    /// let result = client.save_instance(&user, args).await?;
    /// ```
    #[pseudonym::alias(save)]
    pub async fn save_instance<I: TerminusDBModel>(
        &self,
        model: &I,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>> {
        // First try to create
        match self.create_instance(model, args.clone()).await {
            Ok(result) => Ok(result),
            Err(e) => {
                // If creation failed, check if it's because the instance exists
                let error_str = e.to_string();
                if error_str.contains("already exists") || error_str.contains("conflict") {
                    debug!("Instance already exists, attempting update instead");
                    self.update_instance(model, args).await
                } else {
                    // If it's a different error, propagate it
                    Err(e)
                }
            }
        }
    }

    // /// Finds the commit that added a specific instance by walking through the commit log
    // /// and checking each commit to see if it added the given instance ID.
    // ///
    // /// Uses the existing `all_commit_created_entity_ids_any_type` helper to get all
    // /// entity IDs created in each commit and checks if our target instance is in that list.
    // #[cfg(not(target_arch = "wasm32"))]
    // async fn find_commit_for_instance<I: TerminusDBModel>(
    //     &self,
    //     instance_id: &str,
    //     spec: &BranchSpec,
    // ) -> anyhow::Result<String> {
    //     debug!("=== find_commit_for_instance START ===");
    //     debug!("Looking for commit that created instance: {}", instance_id);
    //     debug!("Using branch spec: db={}, branch={:?}", spec.db, spec.branch);
    //
    //     if !self.has_document(&instance_id, spec).await {
    //         return bail!("Instance {} does not exist", instance_id);
    //     }
    //
    //     // Wrap the entire search operation with a timeout
    //     let search_future = async {
    //         // Create a commit log iterator with a smaller batch size
    //         let log_opts = LogOpts {
    //             count: Some(10), // Reduce from default to 10 commits per batch
    //             ..Default::default()
    //         };
    //         let mut log_iter = self.log_iter(spec.clone(), log_opts).await;
    //
    //         let mut commits_checked = 0;
    //         let max_commits = 10; // Reduced from 100 to 10
    //
    //         // Walk through commits until we find the one containing our instance
    //         while let Some(log_entry_result) = log_iter.next().await {
    //             debug!("Getting next log entry from iterator...");
    //             let log_entry = log_entry_result?;
    //             commits_checked += 1;
    //
    //             debug!("Checking commit {} ({}/{})...", log_entry.id, commits_checked, max_commits);
    //             debug!("  Commit timestamp: {:?}", log_entry.timestamp);
    //             debug!("  Commit message: {:?}", log_entry.message);
    //
    //             // Get all entity IDs created in this commit (any type)
    //             debug!("Calling all_commit_created_entity_ids_any_type for commit {}...", log_entry.id);
    //             let start_time = std::time::Instant::now();
    //             match self.all_commit_created_entity_ids_any_type(spec, &log_entry).await {
    //                 Ok(created_ids) => {
    //                     let elapsed = start_time.elapsed();
    //                     debug!("Query completed in {:?}", elapsed);
    //                     debug!("Commit {} has {} entities", log_entry.id, created_ids.len());
    //                     if created_ids.len() > 0 {
    //                         debug!("First few entity IDs: {:?}", created_ids.iter().take(5).collect::<Vec<_>>());
    //                     }
    //
    //                     // Check if our instance ID is in the list of created entities
    //                     // Need to check both with and without schema prefix since insert_instance
    //                     // returns IDs without prefix but commits store them with prefix
    //                     debug!("Checking if instance_id '{}' is in created_ids...", instance_id);
    //                     let id_matches = created_ids.iter().any(|id| {
    //                         let matches = id == instance_id || id.split('/').last() == Some(instance_id);
    //                         if matches {
    //                             debug!("  Found match: {}", id);
    //                         }
    //                         matches
    //                     });
    //
    //                     if id_matches {
    //                         debug!("✓ Found instance {} in commit {}", instance_id, log_entry.id);
    //                         return Ok(log_entry.id.clone());
    //                     } else {
    //                         debug!("Instance not found in this commit, continuing...");
    //                     }
    //                 }
    //                 Err(e) => {
    //                     let elapsed = start_time.elapsed();
    //                     warn!("Failed to query commit {} after {:?}: {}", log_entry.id, elapsed, e);
    //                     // Continue to next commit instead of failing entirely
    //                 }
    //             }
    //
    //             // Early exit if we've checked enough commits
    //             if commits_checked >= max_commits {
    //                 warn!("Reached max commit limit ({}) without finding instance {}", max_commits, instance_id);
    //                 break;
    //             }
    //         }
    //
    //         Err(anyhow!("Could not find commit for instance {} after checking {} commits", instance_id, commits_checked))
    //     };
    //
    //     // Apply overall timeout of 60 seconds
    //     let timeout_duration = std::time::Duration::from_secs(30);
    //     match tokio::time::timeout(timeout_duration, search_future).await {
    //         Ok(result) => {
    //             debug!("=== find_commit_for_instance END === Result: {:?}", result.as_ref().map(|s| s.as_str()));
    //             result
    //         },
    //         Err(_) => {
    //             error!("Timeout: find_commit_for_instance exceeded 60 seconds for instance {}", instance_id);
    //             Err(anyhow!("Operation timed out after 60 seconds while searching for commit containing instance {}", instance_id))
    //         }
    //     }
    // }


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

        // the default here makes stuff unfold
        let mut opts: GetOpts = Default::default();

        if Target::to_schema().should_unfold() {
            opts.unfold = true;
        }

        let json_instance_doc = self.get_document(&doc_id, spec, opts).await?;

        let res = deserializer.from_instance(json_instance_doc.clone());

        match res {
            Ok(t) => Ok(t),
            Err(err) => Err(err).context(format!(
                "TerminusHTTPClient failed to deserialize Instance. See: {}",
                super::helpers::dump_json(&json_instance_doc).display()
            )),
        }
    }

    /// Get the commit history for a strongly-typed model instance.
    ///
    /// This is a convenience method that automatically formats the instance ID
    /// according to the model's type and then retrieves the commit history.
    ///
    /// # Type Parameters
    /// * `I` - A type that implements `TerminusDBModel` (derives `TerminusDBModel`)
    ///
    /// # Arguments
    /// * `instance_id` - The instance ID (without type prefix, e.g., "abc123")
    /// * `spec` - Branch specification (branch to query history from)
    /// * `params` - Optional parameters for pagination and filtering
    ///
    /// # Returns
    /// A vector of `CommitHistoryEntry` containing commit details
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel, Serialize, Deserialize)]
    /// struct Person { name: String, age: i32 }
    ///
    /// // Get history for a Person instance
    /// let history = client.get_instance_history::<Person>(
    ///     "abc123randomkey",
    ///     &branch_spec,
    ///     None
    /// ).await?;
    ///
    /// // This is equivalent to:
    /// // client.get_document_history("Person/abc123randomkey", &branch_spec, None).await?
    /// ```
    pub async fn get_instance_history<I: TerminusDBModel>(
        &self,
        instance_id: &str,
        spec: &BranchSpec,
        params: Option<DocumentHistoryParams>,
    ) -> anyhow::Result<Vec<CommitHistoryEntry>> {
        let full_id = super::helpers::format_id::<I>(instance_id);
        self.get_document_history(&full_id, spec, params).await
    }

    pub async fn get_latest_version<I: TerminusDBModel>(
        &self,
        instance_id: &str,
        spec: &BranchSpec,
    ) -> anyhow::Result<String> {
        let history = self.get_instance_history::<I>(instance_id, spec, Some(DocumentHistoryParams::new().with_count(1u32))).await?;
        history.first()
            .map(|CommitHistoryEntry{ identifier, .. }| identifier.clone())
            .ok_or(anyhow::anyhow!("No commit history found"))
    }

    /// Retrieves and deserializes multiple strongly-typed model instances from the database.
    ///
    /// This is the **preferred method** for retrieving multiple typed models from TerminusDB.
    /// Use this instead of [`get_documents`](Self::get_documents) when working with
    /// structs that implement `TerminusDBModel`.
    ///
    /// # Type Parameters
    /// * `Target` - The type to deserialize the instances into (implements `TerminusDBModel`)
    ///
    /// # Arguments
    /// * `ids` - Vector of instance IDs (number only, no schema class prefix)
    /// * `spec` - Branch specification (for time-travel, use commit-specific specs)
    /// * `opts` - Get options for controlling query behavior (skip, count, type filter, etc.)
    /// * `deserializer` - Instance deserializer for converting from TerminusDB format
    ///
    /// # Returns
    /// A vector of deserialized instances of type `Target`
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel)]
    /// struct User { name: String, age: i32 }
    ///
    /// let ids = vec!["alice_id".to_string(), "bob_id".to_string()];
    /// let mut deserializer = DefaultDeserializer::new();
    /// let opts = GetOpts::default().with_unfold(true);
    /// let users: Vec<User> = client.get_instances(ids, &spec, opts, &mut deserializer).await?;
    /// ```
    ///
    /// # Pagination Example
    /// ```rust
    /// let opts = GetOpts::paginated(0, 10); // skip 0, take 10
    /// let users: Vec<User> = client.get_instances(ids, &spec, opts, &mut deserializer).await?;
    /// ```
    ///
    /// # Type Filtering (get all instances of type)
    /// ```rust
    /// let empty_ids = vec![]; // empty IDs means get all of the specified type
    /// let opts = GetOpts::filtered_by_type::<User>().with_count(5);
    /// let users: Vec<User> = client.get_instances(empty_ids, &spec, opts, &mut deserializer).await?;
    /// ```
    ///
    /// # Time Travel
    /// ```rust
    /// // Retrieve from a specific commit
    /// let past_spec = BranchSpec::from("main/local/commit/abc123");
    /// let old_users: Vec<User> = client.get_instances(ids, &past_spec, opts, &mut deserializer).await?;
    /// ```
    #[pseudonym::alias(get_many)]
    pub async fn get_instances<Target: TerminusDBModel>(
        &self,
        ids: Vec<String>,
        spec: &BranchSpec,
        mut opts: GetOpts,
        deserializer: &mut impl TDBInstanceDeserializer<Target>,
    ) -> anyhow::Result<Vec<Target>> {
        // Format IDs with type prefix for untyped document retrieval
        let formatted_ids = if ids.is_empty() {
            // If no IDs provided, we rely on type filtering
            if opts.type_filter.is_none() {
                // Automatically set type filter based on Target type
                opts.type_filter = Some(Target::to_schema().class_name().to_string());
            }
            vec![]
        } else {
            ids.into_iter()
                .map(|id| super::helpers::format_id::<Target>(&id))
                .collect()
        };

        // Set unfold if the target schema requires it
        if Target::to_schema().should_unfold() {
            opts.unfold = true;
        }

        debug!("Getting {} instances of type {}", 
               if formatted_ids.is_empty() { "all".to_string() } else { formatted_ids.len().to_string() }, 
               Target::to_schema().class_name());

        // Retrieve the raw JSON documents
        let json_docs = self.get_documents(formatted_ids, spec, opts).await?;

        debug!("Retrieved {} JSON documents, deserializing...", json_docs.len());

        // Deserialize each document to the target type
        let mut results = Vec::with_capacity(json_docs.len());
        for (i, json_doc) in json_docs.into_iter().enumerate() {
            match deserializer.from_instance(json_doc.clone()) {
                Ok(instance) => results.push(instance),
                Err(err) => {
                    error!("Failed to deserialize document {}: {}", i, err);
                    return Err(err).context(format!(
                        "TerminusHTTPClient failed to deserialize instance {}. See: {}",
                        i,
                        super::helpers::dump_json(&json_doc).display()
                    ));
                }
            }
        }

        debug!("Successfully deserialized {} instances", results.len());
        Ok(results)
    }


    /// Retrieves all versions of a specific instance across its commit history (simplified version).
    ///
    /// This is a convenience method that returns just the instances without commit information.
    /// Use [`get_instance_versions`](Self::get_instance_versions) if you need commit IDs.
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
    /// A vector of instances representing the full version history
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel, Serialize, Deserialize)]
    /// struct Person { name: String, age: i32 }
    ///
    /// let mut deserializer = DefaultDeserializer::new();
    /// let versions = client.list_instance_versions_simple::<Person>(
    ///     "abc123randomkey",
    ///     &branch_spec,
    ///     &mut deserializer
    /// ).await?;
    ///
    /// for person in versions {
    ///     println!("{} (age {})", person.name, person.age);
    /// }
    /// ```
    pub async fn list_instance_versions_simple<T: TerminusDBModel>(
        &self,
        instance_id: &str,
        spec: &BranchSpec,
        deserializer: &mut impl TDBInstanceDeserializer<T>,
    ) -> anyhow::Result<Vec<T>> {
        let versions = self.list_instance_versions(instance_id, spec, deserializer).await?;
        Ok(versions.into_iter().map(|(instance, _)| instance).collect())
    }

    /// Deletes a strongly-typed model instance from the database.
    ///
    /// This is the **preferred method** for deleting typed models.
    /// Use this instead of `delete_document` when working with structs that derive `TerminusDBModel`.
    ///
    /// **⚠️ Warning**: Using `DeleteOpts::nuke_all_data()` will remove ALL data from the graph.
    /// Use with extreme caution as this operation is irreversible.
    ///
    /// # Type Parameters
    /// * `T` - A type that implements `TerminusDBModel` (derives `TerminusDBModel`)
    ///
    /// # Arguments
    /// * `instance` - The strongly-typed model instance to delete
    /// * `args` - Document insertion arguments specifying the database and branch  
    /// * `opts` - Delete options controlling the deletion behavior
    ///
    /// # Returns
    /// A cloned instance of the client
    ///
    /// # Example
    /// ```rust,ignore
    /// // Delete the specific user (safe)
    /// client.delete_instance(&user, args, DeleteOpts::document_only()).await?;
    /// 
    /// // WARNING: Nuclear option - deletes ALL instances
    /// client.delete_instance(&user, args, DeleteOpts::nuke_all_data()).await?; // DANGEROUS!
    /// ```
    pub async fn delete_instance<T: TerminusDBModel>(
        &self,
        instance: &T,
        args: DocumentInsertArgs,
        opts: DeleteOpts,
    ) -> anyhow::Result<Self> {
        let instance_id = instance.instance_id()
            .ok_or_else(|| anyhow!("Instance has no ID - cannot delete"))?;
        let id = instance_id.to_string();
        let full_id = format_id::<T>(&id);
        
        debug!("Deleting instance {} (type: {})", &full_id, std::any::type_name::<T>());
        
        self.delete_document(
            Some(&full_id),
            &args.spec,
            &args.author,
            &args.message,
            &args.ty.to_string(),
            opts,
        ).await
    }

    /// Deletes a strongly-typed model instance by ID from the database.
    ///
    /// This method allows deletion by ID without needing the full instance object.
    /// 
    /// **⚠️ Warning**: Using `DeleteOpts::nuke_all_data()` will remove ALL data from the graph.
    /// Use with extreme caution as this operation is irreversible.
    ///
    /// # Type Parameters
    /// * `T` - A type that implements `TerminusDBModel` (derives `TerminusDBModel`)
    ///
    /// # Arguments
    /// * `instance_id` - The instance ID (without type prefix, e.g., "alice")
    /// * `args` - Document insertion arguments specifying the database and branch
    /// * `opts` - Delete options controlling the deletion behavior
    ///
    /// # Returns
    /// A cloned instance of the client
    ///
    /// # Example
    /// ```rust,ignore
    /// // Delete user by ID (safe)
    /// client.delete_instance_by_id::<User>("alice", args, DeleteOpts::document_only()).await?;
    /// 
    /// // WARNING: Nuclear option - deletes ALL data
    /// client.delete_instance_by_id::<User>("alice", args, DeleteOpts::nuke_all_data()).await?; // DANGEROUS!
    /// ```
    pub async fn delete_instance_by_id<T: TerminusDBModel>(
        &self,
        instance_id: &str,
        args: DocumentInsertArgs,
        opts: DeleteOpts,
    ) -> anyhow::Result<Self> {
        let full_id = format_id::<T>(instance_id);
        
        debug!("Deleting instance {} by ID (type: {})", &full_id, std::any::type_name::<T>());
        
        self.delete_document(
            Some(&full_id),
            &args.spec,
            &args.author,
            &args.message,
            &args.ty.to_string(),
            opts,
        ).await
    }
}