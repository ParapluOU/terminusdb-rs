//! Strongly-typed instance operations

use super::{
    helpers::{dedup_instances_by_id, dump_schema, format_id},
    TerminusDBModel,
};
use crate::{CommitId, DefaultTDBDeserializer, InsertInstanceResult};
use {
    crate::{
        document::{
            CommitHistoryEntry, DocumentHistoryParams, DocumentInsertArgs, DocumentType, GetOpts,
        },
        http::document::DeleteOpts,
        result::ResponseWithHeaders,
        spec::BranchSpec,
        IntoBoxedTDBInstances, TDBInsertInstanceResult, TDBInstanceDeserializer,
        debug::{OperationEntry, OperationType, QueryLogEntry},
    },
    ::tracing::{debug, error, instrument, warn},
    anyhow::{anyhow, bail, Context},
    futures_util::StreamExt,
    std::{collections::HashMap, fmt::Debug, time::Instant},
    tap::{Tap, TapFallible},
    terminusdb_schema::GraphType,
    terminusdb_schema::{EntityIDFor, Instance, Key, ToJson, ToTDBInstance, ToTDBInstances, FromTDBInstance, InstanceFromJson},
    terminusdb_woql2::prelude::Query as Woql2Query,
    terminusdb_woql_builder::prelude::{node, vars, Var, WoqlBuilder},
};

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
    #[instrument(
        name = "terminus.instance.has",
        skip(self, model),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %I::schema_name()
        )
    )]
    #[pseudonym::alias(has)]
    pub async fn has_instance<I: TerminusDBModel>(&self, model: &I, spec: &BranchSpec) -> bool {
        match model.instance_id() {
            None => false,
            Some(entity_id) => {
                // EntityIDFor implements AsRef<str>
                self.has_document(entity_id.as_ref(), spec).await
            }
        }
    }

    #[instrument(
        name = "terminus.instance.has_by_id",
        skip(self),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %I::schema_name(),
            id = %model_id
        )
    )]
    #[pseudonym::alias(has_id)]
    pub async fn has_instance_id<I: TerminusDBModel>(
        &self,
        model_id: &str,
        spec: &BranchSpec,
    ) -> bool {
        self.has_document(&format_id::<I>(model_id), spec).await
    }

    /// Prepares instances from a model for database operations
    fn prepare_instances<I: TerminusDBModel>(model: &I) -> Vec<terminusdb_schema::Instance> {
        let instance = model.to_instance(None);
        instance
            .to_instance_tree_flatten(true)
            .into_iter()
            .map(|mut i| {
                i.set_random_key_prefix();
                i.capture = true;
                i
            })
            .filter(|i| !i.is_reference())
            .collect::<Vec<_>>()
    }

    /// Common helper for processing instance creation/update operations results
    /// Finds the root ID and creates a structured result
    fn process_operation_result<I: TerminusDBModel>(
        res: crate::result::ResponseWithHeaders<
            std::collections::HashMap<String, crate::TDBInsertInstanceResult>,
        >,
    ) -> anyhow::Result<crate::InsertInstanceResult> {
        // Find the actual root ID in the results using EntityIDFor validation
        let actual_root_id = res
            .keys()
            .find(|k| EntityIDFor::<I>::new(k).is_ok())
            .cloned()
            .ok_or_else(|| anyhow!("Could not find root instance ID in operation results; instead:{:?}", res.keys().collect::<Vec<_>>()))?;

        // Create structured result
        let mut result = crate::InsertInstanceResult::new((*res).clone(), actual_root_id)?;

        // Set commit_id from response headers
        result.commit_id = res.extract_commit_id();

        Ok(result)
    }

    /// Inserts an instance into TerminusDB and returns a structured result with the root entity
    /// clearly identified, along with the commit ID that created it. This method properly handles
    /// models with sub-entities by tracking which ID is the root.
    ///
    /// # Returns
    /// A tuple of (InsertInstanceResult, commit_id) where:
    /// - InsertInstanceResult: Contains the root ID, sub-entity results, and all results
    /// - commit_id: The commit ID (e.g., "ValidCommit/...") that added this instance
    #[instrument(
        name = "terminus.instance.insert_with_commit_id",
        skip(self, model, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch,
            entity_type = %I::schema_name()
        ),
        err
    )]
    pub async fn insert_instance_with_commit_id<I: TerminusDBModel>(
        &self,
        model: &I,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<(crate::InsertInstanceResult, CommitId)> {
        // Insert the instance - save_instance now returns InsertInstanceResult directly
        let mut result = self.save_instance(model, args.clone()).await?;

        // Handle commit ID based on whether instance was inserted or already existed
        let commit_id = match &result.root_result {
            TDBInsertInstanceResult::Inserted(_) => {
                // Extract from stored commit_id for newly inserted instances
                result.extract_commit_id().ok_or_else(|| {
                    anyhow!("TerminusDB-Data-Version header not found or invalid format")
                })?
            }
            TDBInsertInstanceResult::AlreadyExists(id) => {
                // For already existing instances, fall back to commit log search
                debug!(
                    "Instance {} already exists, falling back to commit log search",
                    id
                );
                let short_id = id.split('/').last().unwrap_or(id);
                self.get_latest_version::<I>(short_id, &args.spec)
                    .await
                    .context(format!(
                        "Failed to find commit for existing instance {}",
                        id
                    ))?
            }
        };

        Ok((result, commit_id))
    }

    /// Inserts an instance into TerminusDB and retrieves the full model with server-generated IDs populated.
    ///
    /// This method is particularly useful for models with server-generated IDs (lexical or value_hash key strategies).
    /// It performs an insert followed by a retrieval to return the complete model with all fields populated,
    /// including any server-generated ID in the `id_field`.
    ///
    /// # Type Parameters
    /// * `I` - A type that implements `TerminusDBModel`, `ToTDBInstance`, `FromTDBInstance`, and `InstanceFromJson`
    ///
    /// # Arguments
    /// * `model` - The strongly-typed model instance to insert
    /// * `args` - Document insertion arguments specifying the database, branch, and options
    ///
    /// # Returns
    /// A tuple of (model, commit_id) where:
    /// - model: The fully populated model with server-generated ID field set
    /// - commit_id: The commit ID (e.g., "ValidCommit/...") that added this instance
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel, Serialize, Deserialize)]
    /// #[tdb(key = "lexical", key_fields = "email", id_field = "id")]
    /// struct User {
    ///     id: ServerIDFor<Self>,
    ///     email: String,
    ///     name: String,
    /// }
    ///
    /// let user = User {
    ///     id: ServerIDFor::new(),
    ///     email: "alice@example.com".to_string(),
    ///     name: "Alice".to_string(),
    /// };
    /// 
    /// let (saved_user, commit_id) = client.insert_instance_and_retrieve(&user, args).await?;
    /// // saved_user.id will now contain the server-generated ID
    /// assert!(saved_user.id.is_some());
    /// ```
    #[instrument(
        name = "terminus.instance.insert_and_retrieve",
        skip(self, model, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch,
            entity_type = %I::schema_name()
        ),
        err
    )]
    pub async fn insert_instance_and_retrieve<I>(
        &self,
        model: &I,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<(I, CommitId)>
    where
        I: TerminusDBModel + ToTDBInstance + FromTDBInstance + InstanceFromJson,
    {
        // First, insert the instance and get the result with commit ID
        let (result, commit_id) = self.insert_instance_with_commit_id(model, args.clone()).await?;
        
        // Extract the root ID from the result
        let id = match &result.root_result {
            TDBInsertInstanceResult::Inserted(id) | TDBInsertInstanceResult::AlreadyExists(id) => {
                // Extract just the ID part (after the type prefix)
                id.split('/').last()
                    .ok_or_else(|| anyhow!("Invalid ID format: {}", id))?
            }
        };

        // Create a default deserializer
        let mut deserializer = DefaultTDBDeserializer;
        
        // Retrieve the full instance with server-generated ID populated
        let retrieved_model = self.get_instance::<I>(id, &args.spec, &mut deserializer)
            .await
            .context("Failed to retrieve instance after insertion")?;
        
        Ok((retrieved_model, commit_id))
    }

    /// Inserts multiple instances into TerminusDB and returns the results along with the commit ID.
    ///
    /// This is the plural variant of `insert_instance_with_commit_id` for bulk operations.
    /// It inserts multiple instances and returns the commit ID that created them.
    ///
    /// # Arguments
    /// * `models` - Collection of models that can be converted to TerminusDB instances
    /// * `args` - Document insertion arguments (DocumentType will be set to Instance automatically)
    ///
    /// # Returns
    /// A tuple of (ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>, commit_id) where:
    /// - ResponseWithHeaders: Contains all instance IDs and their insert results, plus the commit_id in headers
    /// - commit_id: The commit ID (e.g., "ValidCommit/...") that added these instances
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
    /// let (result, commit_id) = client.insert_instances_with_commit_id(users, args).await?;
    /// println!("Inserted {} users in commit {}", result.len(), commit_id);
    /// ```
    #[instrument(
        name = "terminus.instance.insert_multiple_with_commit_id",
        skip(self, models, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch
        ),
        err
    )]
    pub async fn insert_instances_with_commit_id(
        &self,
        models: impl IntoBoxedTDBInstances,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<(
        ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>,
        CommitId,
    )> {
        // Insert the instances
        let result = self.insert_instances(models, args).await?;

        // Extract commit ID from the response headers
        let commit_id = result
            .extract_commit_id()
            .ok_or_else(|| anyhow!("TerminusDB-Data-Version header not found or invalid format"))?;

        Ok((result, commit_id))
    }

    /// Inserts multiple instances into TerminusDB and retrieves the full models with server-generated IDs populated.
    ///
    /// This is the plural variant of `insert_instance_and_retrieve` for bulk operations.
    /// It performs bulk insert followed by bulk retrieval to return complete models with all fields populated,
    /// including any server-generated IDs in the `id_field`.
    ///
    /// # Type Parameters
    /// * `I` - A type that implements `TerminusDBModel`, `ToTDBInstance`, `FromTDBInstance`, `InstanceFromJson`, and `Clone`
    ///
    /// # Arguments
    /// * `models` - Vector of strongly-typed model instances to insert
    /// * `args` - Document insertion arguments specifying the database, branch, and options
    ///
    /// # Returns
    /// A tuple of (models, commit_id) where:
    /// - models: Vector of fully populated models with server-generated ID fields set
    /// - commit_id: The commit ID (e.g., "ValidCommit/...") that added these instances
    ///
    /// # Example
    /// ```rust
    /// #[derive(Clone, TerminusDBModel, Serialize, Deserialize)]
    /// #[tdb(key = "lexical", key_fields = "email", id_field = "id")]
    /// struct User {
    ///     id: ServerIDFor<Self>,
    ///     email: String,
    ///     name: String,
    /// }
    ///
    /// let users = vec![
    ///     User {
    ///         id: ServerIDFor::new(),
    ///         email: "alice@example.com".to_string(),
    ///         name: "Alice".to_string(),
    ///     },
    ///     User {
    ///         id: ServerIDFor::new(),
    ///         email: "bob@example.com".to_string(),
    ///         name: "Bob".to_string(),
    ///     },
    /// ];
    /// 
    /// let (saved_users, commit_id) = client.insert_instances_and_retrieve(users, args).await?;
    /// // All saved_users will have their id fields populated
    /// for user in &saved_users {
    ///     assert!(user.id.is_some());
    /// }
    /// ```
    #[instrument(
        name = "terminus.instance.insert_multiple_and_retrieve",
        skip(self, models, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch,
            count = models.len()
        ),
        err
    )]
    pub async fn insert_instances_and_retrieve<I>(
        &self,
        models: Vec<I>,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<(Vec<I>, CommitId)>
    where
        I: TerminusDBModel + ToTDBInstance + FromTDBInstance + InstanceFromJson + Clone + 'static,
    {
        // First, insert the instances and get the results with commit ID
        let (result, commit_id) = self.insert_instances_with_commit_id(models.clone(), args.clone()).await?;
        
        // Extract all IDs from the result
        let mut all_ids: Vec<String> = Vec::new();
        for (id_with_type, insert_result) in result.iter() {
            match insert_result {
                TDBInsertInstanceResult::Inserted(_) | TDBInsertInstanceResult::AlreadyExists(_) => {
                    // Extract just the ID part (after the type prefix)
                    if let Some(id) = id_with_type.split('/').last() {
                        all_ids.push(id.to_string());
                    }
                }
            }
        }
        
        // Retrieve all instances in a single batch call
        let mut deserializer = DefaultTDBDeserializer;
        let retrieved_models = match self.get_instances::<I>(
            all_ids.clone(),
            &args.spec,
            GetOpts::default(),
            &mut deserializer
        ).await {
            Ok(models) => models,
            Err(e) => {
                // If bulk retrieval fails completely, log and try to return what we can
                warn!("Bulk retrieval failed, some instances may not have been retrieved: {}", e);
                
                // In case of complete failure, we could fall back to individual retrieval
                // but for now, we'll just return an empty vec to maintain performance
                vec![]
            }
        };
        
        // Verify we got the expected number of results
        if retrieved_models.len() != models.len() {
            return Err(anyhow!(
                "Expected {} instances but retrieved {}",
                models.len(),
                retrieved_models.len()
            ));
        }
        
        Ok((retrieved_models, commit_id))
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
    #[instrument(
        name = "terminus.instance.create",
        skip(self, model, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch,
            entity_type = %I::schema_name()
        ),
        err
    )]
    #[pseudonym::alias(create)]
    pub async fn create_instance<I: TerminusDBModel>(
        &self,
        model: &I,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<crate::InsertInstanceResult> {
        let instances = Self::prepare_instances(model);
        let models = instances.iter().collect();
        let res = self.post_documents(models, args).await?;

        Self::process_operation_result::<I>(res)
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
    #[instrument(
        name = "terminus.instance.update",
        skip(self, model, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch,
            entity_type = %I::schema_name()
        ),
        err
    )]
    #[pseudonym::alias(update, replace_instance, replace)]
    pub async fn update_instance<I: TerminusDBModel>(
        &self,
        model: &I,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<crate::InsertInstanceResult> {
        let instance = model.to_instance(None);

        // Check if instance has an ID
        if !instance.has_id() {
            return Err(anyhow!("Cannot update instance without an ID"));
        }

        let instances = Self::prepare_instances(model);
        let models = instances.iter().collect();
        // let res = self.put_documents(models, args).await?;
        let res = self.insert_documents(models, args).await?;

        Self::process_operation_result::<I>(res)
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
    #[instrument(
        name = "terminus.instance.save",
        skip(self, model, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch,
            entity_type = %I::schema_name(),
            force = args.force
        ),
        err
    )]
    #[pseudonym::alias(save, insert_instance, insert)]
    pub async fn save_instance<I: TerminusDBModel>(
        &self,
        model: &I,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<crate::InsertInstanceResult> {
        let start_time = Instant::now();
        let has = self.has_instance(model, &args.spec).await;

        // First check if instance already exists
        // if !args.force
        //     && has
        //     && let Some(entity_id) = model.instance_id()
        // {
        //     let id = entity_id.id().to_string();
        //     warn!("not inserted because it already exists");
        //
        //     // Return structured result for already existing instance
        //     // todo: make this more convenient to create
        //     let mut result = crate::InsertInstanceResult::new(
        //         HashMap::from([(
        //             id.clone(),
        //             TDBInsertInstanceResult::AlreadyExists(id.clone()),
        //         )]),
        //         id.clone(),
        //     )?;
        //
        //     result.commit_id = self
        //         .get_document_with_headers(entity_id.typed(), &args.spec, GetOpts::default())
        //         .await?
        //         .extract_commit_id();
        //
        //     return Ok(result);
        // }

        let operation_type = if has { OperationType::Update } else { OperationType::Insert };
        let endpoint = format!("/api/db/{}/document/{}", args.spec.db, I::schema_name());
        
        let result = if has {
            self.update_instance(model, args.clone()).await
        } else {
            self.create_instance(model, args.clone()).await
        };
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        
        // Create operation entry
        let mut operation = OperationEntry::new(operation_type.clone(), endpoint.clone())
            .with_context(Some(args.spec.db.clone()), args.spec.branch.clone());
        
        // Log the operation
        match &result {
            Ok(res) => {
                let count = res.sub_entities.values().count();
                operation = operation.success(Some(count), duration_ms);
                
                // Log to query log if enabled
                let logger_opt = self.query_logger.read().ok().and_then(|guard| guard.clone());
                if let Some(logger) = logger_opt {
                    let details = serde_json::json!({
                        "entity_type": I::schema_name(),
                        "instance_id": model.instance_id().map(|id| id.id().to_string()),
                        "force": args.force,
                    });
                    
                    let log_entry = QueryLogEntry {
                        timestamp: chrono::Utc::now(),
                        operation_type: operation_type.to_string(),
                        database: Some(args.spec.db.clone()),
                        branch: args.spec.branch.clone(),
                        endpoint,
                        details,
                        success: true,
                        result_count: Some(count),
                        duration_ms,
                        error: None,
                    };
                    let _ = logger.log(log_entry).await;
                }
            }
            Err(e) => {
                operation = operation.failure(e.to_string(), duration_ms);
                
                // Log to query log if enabled
                let logger_opt = self.query_logger.read().ok().and_then(|guard| guard.clone());
                if let Some(logger) = logger_opt {
                    let details = serde_json::json!({
                        "entity_type": I::schema_name(),
                        "instance_id": model.instance_id().map(|id| id.id().to_string()),
                        "force": args.force,
                    });
                    
                    let log_entry = QueryLogEntry {
                        timestamp: chrono::Utc::now(),
                        operation_type: operation_type.to_string(),
                        database: Some(args.spec.db.clone()),
                        branch: args.spec.branch.clone(),
                        endpoint,
                        details,
                        success: false,
                        result_count: None,
                        duration_ms,
                        error: Some(e.to_string()),
                    };
                    let _ = logger.log(log_entry).await;
                }
            }
        }
        
        self.operation_log.push(operation);
        
        result
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
        debug!(
            "Using collections: commit={}, db={}",
            &commit_collection, &db_collection
        );

        let id_var = vars!("id");
        let type_var = vars!("type");

        // Query for all added triples (any type)
        let query = WoqlBuilder::new()
            .added_triple(
                id_var.clone(),   // subject: variable "id"
                "rdf:type",       // predicate: "rdf:type"
                type_var.clone(), // object: any type
                GraphType::Instance.into(),
            )
            .using(commit_collection)
            .using(db_collection)
            .limit(1000)
            .finalize();

        let json_query = query.to_instance(None).to_json();

        debug!(
            "Running WOQL query: {}",
            serde_json::to_string_pretty(&json_query).unwrap_or_default()
        );

        // Add a timeout to prevent hanging forever
        let query_future = self.query_raw(Some(spec.clone()), json_query);
        let timeout_duration = std::time::Duration::from_secs(30);

        let res: crate::WOQLResult<serde_json::Value> =
            match tokio::time::timeout(timeout_duration, query_future).await {
                Ok(result) => result?,
                Err(_) => {
                    error!(
                        "Query timed out after 30 seconds for commit {}",
                        &commit.identifier
                    );
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

        debug!(
            "=== all_commit_created_entity_ids_any_type END === Found {} entities",
            result.len()
        );
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
    #[instrument(
        name = "terminus.instance.insert_multiple",
        skip(self, models, args),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch
        ),
        err
    )]
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
    /// **Note on Unfold Behavior**: This method automatically sets `unfold=true` if the
    /// target schema has the `@unfoldable` attribute. To override this behavior, use
    /// [`get_instance_with_opts`](Self::get_instance_with_opts) or explicitly control
    /// unfolding with [`get_instance_unfolded`](Self::get_instance_unfolded).
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
    #[instrument(
        name = "terminus.instance.get",
        skip(self, deserializer),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %Target::schema_name(),
            id = %id
        ),
        err
    )]
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

    /// Retrieves and deserializes a strongly-typed model instance with explicit unfold control.
    ///
    /// This method always unfolds linked documents regardless of the schema's @unfoldable attribute.
    /// Use this when you want to ensure all referenced documents are included in the response.
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
    /// The deserialized instance of type `Target` with all linked documents unfolded
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel)]
    /// struct User { 
    ///     name: String, 
    ///     age: i32,
    ///     address: Address // This will be unfolded
    /// }
    ///
    /// let mut deserializer = DefaultDeserializer::new();
    /// let user: User = client.get_instance_unfolded("12345", &spec, &mut deserializer).await?;
    /// // user.address will contain the full Address object, not just a reference
    /// ```
    #[instrument(
        name = "terminus.instance.get_unfolded",
        skip(self, deserializer),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %Target::schema_name(),
            id = %id
        ),
        err
    )]
    pub async fn get_instance_unfolded<Target: ToTDBInstance>(
        &self,
        id: &str,
        spec: &BranchSpec,
        mut deserializer: &mut impl TDBInstanceDeserializer<Target>,
    ) -> anyhow::Result<Target>
    where
        Target:,
    {
        let doc_id = format_id::<Target>(id);
        let opts = GetOpts::default().with_unfold(true);

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

    /// Retrieves and deserializes a strongly-typed model instance with full control over options.
    ///
    /// This method accepts a full `GetOpts` structure, allowing complete control over the retrieval
    /// behavior including unfold, pagination, and other options. This is the most flexible variant
    /// of the get_instance methods.
    ///
    /// # Type Parameters
    /// * `Target` - The type to deserialize the instance into (implements `ToTDBInstance`)
    ///
    /// # Arguments
    /// * `id` - The instance ID (number only, no schema class prefix)
    /// * `spec` - Branch specification (for time-travel, use commit-specific specs)
    /// * `opts` - Get options for controlling query behavior
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
    /// let opts = GetOpts::default()
    ///     .with_unfold(false)  // Override automatic unfolding
    ///     .with_as_list(true);
    /// let user: User = client.get_instance_with_opts("12345", &spec, opts, &mut deserializer).await?;
    /// ```
    #[instrument(
        name = "terminus.instance.get_with_opts",
        skip(self, deserializer),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %Target::schema_name(),
            id = %id,
            unfold = opts.unfold,
            skip = opts.skip,
            count = opts.count
        ),
        err
    )]
    pub async fn get_instance_with_opts<Target: ToTDBInstance>(
        &self,
        id: &str,
        spec: &BranchSpec,
        opts: GetOpts,
        mut deserializer: &mut impl TDBInstanceDeserializer<Target>,
    ) -> anyhow::Result<Target>
    where
        Target:,
    {
        let doc_id = format_id::<Target>(id);

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

    /// Retrieves and deserializes a strongly-typed model instance from the database with commit ID.
    ///
    /// This is a header-aware variant of [`get_instance`](Self::get_instance) that returns
    /// both the deserialized instance and the commit ID from the TerminusDB-Data-Version header.
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
    /// A `ResponseWithHeaders` containing:
    /// - `data`: The deserialized instance of type `Target`
    /// - `commit_id`: Optional commit ID from the TerminusDB-Data-Version header
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel)]
    /// struct User { name: String, age: i32 }
    ///
    /// let mut deserializer = DefaultDeserializer::new();
    /// let result = client.get_instance_with_headers::<User>("12345", &spec, &mut deserializer).await?;
    /// let user = &*result; // Access the user via Deref
    /// if let Some(commit_id) = result.extract_commit_id() {
    ///     println!("Retrieved user from commit: {}", commit_id);
    /// }
    /// ```
    ///
    /// # Time Travel
    /// ```rust
    /// // Retrieve from a specific commit
    /// let past_spec = BranchSpec::from("main/local/commit/abc123");
    /// let result = client.get_instance_with_headers::<User>("12345", &past_spec, &mut deserializer).await?;
    /// let old_user = &*result; // Access via Deref
    /// ```
    #[instrument(
        name = "terminus.instance.get_with_headers",
        skip(self, deserializer),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %Target::schema_name(),
            id = %id
        ),
        err
    )]
    pub async fn get_instance_with_headers<Target: TerminusDBModel>(
        &self,
        id: &str,
        spec: &BranchSpec,
        mut deserializer: &mut impl TDBInstanceDeserializer<Target>,
    ) -> anyhow::Result<ResponseWithHeaders<Target>>
    where
        Target:,
    {
        let doc_id = format_id::<Target>(id);

        // the default here makes stuff unfold
        let mut opts: GetOpts = GetOpts::default().with_type_filter::<Target>();

        if Target::to_schema().should_unfold() {
            opts.unfold = true;
        }

        let response = self.get_document_with_headers(&doc_id, spec, opts).await?;

        // Get the document from the response (using Deref)
        let json_instance_doc = (*response).clone();

        let res = deserializer.from_instance(json_instance_doc.clone());

        match res {
            Ok(t) => Ok(ResponseWithHeaders::new(t, response.commit_id)),
            Err(err) => Err(err).context(format!(
                "TerminusHTTPClient failed to deserialize Instance. See: {}",
                super::helpers::dump_json(&json_instance_doc).display()
            )),
        }
    }

    /// Retrieves and deserializes a strongly-typed model instance if it exists.
    ///
    /// This method is designed for cases where an instance might not exist and that's 
    /// an expected scenario (e.g., checking before create). Unlike `get_instance`, 
    /// this method returns `None` for non-existent instances without logging errors.
    ///
    /// # Type Parameters
    /// * `Target` - The type to deserialize the instance into (implements `TerminusDBModel`)
    ///
    /// # Arguments
    /// * `id` - The instance ID (number only, no schema class prefix)
    /// * `spec` - Branch specification indicating which branch to query
    /// * `deserializer` - Instance deserializer for converting from TerminusDB format
    ///
    /// # Returns
    /// * `Ok(Some(instance))` - If the instance exists and was successfully deserialized
    /// * `Ok(None)` - If the instance doesn't exist
    /// * `Err(error)` - Only for actual errors (network, parsing, deserialization, etc.)
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel)]
    /// struct User { name: String, age: i32 }
    ///
    /// let mut deserializer = DefaultDeserializer::new();
    /// match client.get_instance_if_exists::<User>("12345", &spec, &mut deserializer).await? {
    ///     Some(user) => println!("User exists: {}", user.name),
    ///     None => println!("User not found"),
    /// }
    /// ```
    #[instrument(
        name = "terminus.instance.get_if_exists",
        skip(self, deserializer),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %Target::schema_name(),
            id = %id
        )
        // Note: no 'err' attribute - we don't want to log DocumentNotFound as errors
    )]
    pub async fn get_instance_if_exists<Target: TerminusDBModel>(
        &self,
        id: &str,
        spec: &BranchSpec,
        mut deserializer: &mut impl TDBInstanceDeserializer<Target>,
    ) -> anyhow::Result<Option<Target>>
    where
        Target:,
    {
        let doc_id = format_id::<Target>(id);

        // the default here makes stuff unfold
        let mut opts: GetOpts = GetOpts::default().with_type_filter::<Target>();

        if Target::to_schema().should_unfold() {
            opts.unfold = true;
        }

        match self.get_document_if_exists(&doc_id, spec, opts).await? {
            Some(json_instance_doc) => {
                let res = deserializer.from_instance(json_instance_doc.clone());

                match res {
                    Ok(t) => Ok(Some(t)),
                    Err(err) => Err(err).context(format!(
                        "TerminusHTTPClient failed to deserialize Instance. See: {}",
                        super::helpers::dump_json(&json_instance_doc).display()
                    )),
                }
            }
            None => Ok(None),
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
    #[instrument(
        name = "terminus.instance.get_history",
        skip(self),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %I::schema_name(),
            instance_id = %instance_id,
            start = params.as_ref().and_then(|p| p.start).unwrap_or(0),
            count = params.as_ref().and_then(|p| p.count)
        ),
        err
    )]
    pub async fn get_instance_history<I: TerminusDBModel>(
        &self,
        instance_id: &str,
        spec: &BranchSpec,
        params: Option<DocumentHistoryParams>,
    ) -> anyhow::Result<Vec<CommitHistoryEntry>> {
        let full_id = super::helpers::format_id::<I>(instance_id);
        self.get_document_history(&full_id, spec, params).await
    }

    #[instrument(
        name = "terminus.instance.get_latest_version",
        skip(self),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %I::schema_name(),
            instance_id = %instance_id
        ),
        err
    )]
    pub async fn get_latest_version<I: TerminusDBModel>(
        &self,
        instance_id: &str,
        spec: &BranchSpec,
    ) -> anyhow::Result<CommitId> {
        // Use the new header-aware method to get the commit ID directly
        let mut deserializer = crate::deserialize::DefaultTDBDeserializer;
        let result = self
            .get_instance_with_headers::<I>(instance_id, spec, &mut deserializer)
            .await?;

        result
            .extract_commit_id()
            .ok_or_else(|| anyhow::anyhow!("No commit ID found in response headers"))
    }

    /// Retrieves and deserializes multiple strongly-typed model instances from the database.
    ///
    /// This is the **preferred method** for retrieving multiple typed models from TerminusDB.
    /// Use this instead of [`get_documents`](Self::get_documents) when working with
    /// structs that implement `TerminusDBModel`.
    ///
    /// **Note on Unfold Behavior**: This method automatically sets `unfold=true` if the
    /// target schema has the `@unfoldable` attribute. The `opts` parameter allows you to
    /// override this behavior. For simpler unfold control, see [`get_instances_unfolded`](Self::get_instances_unfolded).
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
    #[instrument(
        name = "terminus.instance.get_multiple",
        skip(self, ids, deserializer),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %Target::schema_name(),
            id_count = ids.len(),
            unfold = opts.unfold,
            skip = opts.skip,
            count = opts.count,
            type_filter = opts.type_filter
        ),
        err
    )]
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

        debug!(
            "Getting {} instances of type {}",
            if formatted_ids.is_empty() {
                "all".to_string()
            } else {
                formatted_ids.len().to_string()
            },
            Target::to_schema().class_name()
        );

        // Retrieve the raw JSON documents
        let json_docs = self.get_documents(formatted_ids, spec, opts).await?;

        debug!(
            "Retrieved {} JSON documents, deserializing...",
            json_docs.len()
        );

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

    /// Retrieves and deserializes multiple strongly-typed model instances with explicit unfold control.
    ///
    /// This method always unfolds linked documents regardless of the schema's @unfoldable attribute.
    /// Use this when you want to ensure all referenced documents are included in the response
    /// for bulk operations.
    ///
    /// # Type Parameters
    /// * `Target` - The type to deserialize the instances into (implements `TerminusDBModel`)
    ///
    /// # Arguments
    /// * `ids` - Vector of instance IDs (number only, no schema class prefix)
    /// * `spec` - Branch specification (for time-travel, use commit-specific specs)
    /// * `deserializer` - Instance deserializer for converting from TerminusDB format
    ///
    /// # Returns
    /// A vector of deserialized instances of type `Target` with all linked documents unfolded
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel)]
    /// struct User { 
    ///     name: String, 
    ///     age: i32,
    ///     address: Address // This will be unfolded
    /// }
    ///
    /// let ids = vec!["alice_id".to_string(), "bob_id".to_string()];
    /// let mut deserializer = DefaultDeserializer::new();
    /// let users: Vec<User> = client.get_instances_unfolded(ids, &spec, &mut deserializer).await?;
    /// // Each user.address will contain the full Address object, not just a reference
    /// ```
    #[instrument(
        name = "terminus.instance.get_multiple_unfolded",
        skip(self, ids, deserializer),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %Target::schema_name(),
            id_count = ids.len()
        ),
        err
    )]
    pub async fn get_instances_unfolded<Target: TerminusDBModel>(
        &self,
        ids: Vec<String>,
        spec: &BranchSpec,
        deserializer: &mut impl TDBInstanceDeserializer<Target>,
    ) -> anyhow::Result<Vec<Target>> {
        let opts = GetOpts::default().with_unfold(true);
        self.get_instances(ids, spec, opts, deserializer).await
    }

    /// Retrieves and deserializes multiple strongly-typed model instances with full control over options.
    ///
    /// This method accepts a full `GetOpts` structure, allowing complete control over the retrieval
    /// behavior including unfold, pagination, type filtering, and other options. This is the most 
    /// flexible variant of the get_instances methods and is an alias for the standard get_instances
    /// method, provided for API consistency.
    ///
    /// # Type Parameters
    /// * `Target` - The type to deserialize the instances into (implements `TerminusDBModel`)
    ///
    /// # Arguments
    /// * `ids` - Vector of instance IDs (number only, no schema class prefix)
    /// * `spec` - Branch specification (for time-travel, use commit-specific specs)
    /// * `opts` - Get options for controlling query behavior
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
    /// let opts = GetOpts::default()
    ///     .with_unfold(false)  // Override automatic unfolding
    ///     .with_count(10);     // Limit results
    /// let users: Vec<User> = client.get_instances_with_opts(ids, &spec, opts, &mut deserializer).await?;
    /// ```
    #[inline]
    pub async fn get_instances_with_opts<Target: TerminusDBModel>(
        &self,
        ids: Vec<String>,
        spec: &BranchSpec,
        opts: GetOpts,
        deserializer: &mut impl TDBInstanceDeserializer<Target>,
    ) -> anyhow::Result<Vec<Target>> {
        self.get_instances(ids, spec, opts, deserializer).await
    }

    /// Retrieves and deserializes multiple strongly-typed model instances from the database with commit ID.
    ///
    /// This is a header-aware variant of [`get_instances`](Self::get_instances) that returns
    /// both the deserialized instances and the commit ID from the TerminusDB-Data-Version header.
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
    /// A `ResponseWithHeaders` containing:
    /// - `data`: A vector of deserialized instances of type `Target`
    /// - `commit_id`: Optional commit ID from the TerminusDB-Data-Version header
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel)]
    /// struct User { name: String, age: i32 }
    ///
    /// let ids = vec!["alice_id".to_string(), "bob_id".to_string()];
    /// let mut deserializer = DefaultDeserializer::new();
    /// let opts = GetOpts::default().with_unfold(true);
    /// let result = client.get_instances_with_headers(ids, &spec, opts, &mut deserializer).await?;
    /// let users = &*result; // Access via Deref
    /// if let Some(commit_id) = result.extract_commit_id() {
    ///     println!("Retrieved {} users from commit: {}", users.len(), commit_id);
    /// }
    /// ```
    ///
    /// # Type Filtering (get all instances of type with commit ID)
    /// ```rust
    /// let empty_ids = vec![]; // empty IDs means get all of the specified type
    /// let opts = GetOpts::filtered_by_type::<User>().with_count(5);
    /// let result = client.get_instances_with_headers(empty_ids, &spec, opts, &mut deserializer).await?;
    /// let users = &*result; // Access via Deref
    /// ```
    #[instrument(
        name = "terminus.instance.get_multiple_with_headers",
        skip(self, ids, deserializer),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %Target::schema_name(),
            id_count = ids.len(),
            unfold = opts.unfold,
            skip = opts.skip,
            count = opts.count,
            type_filter = opts.type_filter
        ),
        err
    )]
    pub async fn get_instances_with_headers<Target: TerminusDBModel>(
        &self,
        ids: Vec<String>,
        spec: &BranchSpec,
        mut opts: GetOpts,
        deserializer: &mut impl TDBInstanceDeserializer<Target>,
    ) -> anyhow::Result<ResponseWithHeaders<Vec<Target>>> {
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

        debug!(
            "Getting {} instances of type {} with headers",
            if formatted_ids.is_empty() {
                "all".to_string()
            } else {
                formatted_ids.len().to_string()
            },
            Target::to_schema().class_name()
        );

        // Retrieve the raw JSON documents with headers
        let response = self
            .get_documents_with_headers(formatted_ids, spec, opts)
            .await?;

        // Get the documents from the response (using Deref)
        let json_docs = (*response).clone();

        debug!(
            "Retrieved {} JSON documents with commit ID {:?}, deserializing...",
            json_docs.len(),
            response.commit_id
        );

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
        Ok(ResponseWithHeaders::new(results, response.commit_id))
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
    #[instrument(
        name = "terminus.instance.list_versions_simple",
        skip(self, deserializer),
        fields(
            db = %spec.db,
            branch = ?spec.branch,
            entity_type = %T::schema_name(),
            instance_id = %instance_id
        ),
        err
    )]
    pub async fn list_instance_versions_simple<T: TerminusDBModel>(
        &self,
        instance_id: &str,
        spec: &BranchSpec,
        deserializer: &mut impl TDBInstanceDeserializer<T>,
    ) -> anyhow::Result<Vec<T>> {
        let versions = self
            .list_instance_versions(instance_id, spec, deserializer)
            .await?;
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
    #[instrument(
        name = "terminus.instance.delete",
        skip(self, instance, args, opts),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch,
            entity_type = %T::schema_name(),
            nuke = opts.is_nuke()
        ),
        err
    )]
    pub async fn delete_instance<T: TerminusDBModel>(
        &self,
        instance: &T,
        args: DocumentInsertArgs,
        opts: DeleteOpts,
    ) -> anyhow::Result<Self> {
        let instance_id = instance
            .instance_id()
            .ok_or_else(|| anyhow!("Instance has no ID - cannot delete"))?;
        let id = instance_id.to_string();
        let full_id = format_id::<T>(&id);

        debug!(
            "Deleting instance {} (type: {})",
            &full_id,
            std::any::type_name::<T>()
        );

        self.delete_document(
            Some(&full_id),
            &args.spec,
            &args.author,
            &args.message,
            &args.ty.to_string(),
            opts,
        )
        .await
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
    #[instrument(
        name = "terminus.instance.delete_by_id",
        skip(self, args, opts),
        fields(
            db = %args.spec.db,
            branch = ?args.spec.branch,
            entity_type = %T::schema_name(),
            instance_id = %instance_id,
            nuke = opts.is_nuke()
        ),
        err
    )]
    pub async fn delete_instance_by_id<T: TerminusDBModel>(
        &self,
        instance_id: &str,
        args: DocumentInsertArgs,
        opts: DeleteOpts,
    ) -> anyhow::Result<Self> {
        let full_id = format_id::<T>(instance_id);

        debug!(
            "Deleting instance {} by ID (type: {})",
            &full_id,
            std::any::type_name::<T>()
        );

        self.delete_document(
            Some(&full_id),
            &args.spec,
            &args.author,
            &args.message,
            &args.ty.to_string(),
            opts,
        )
        .await
    }
}
