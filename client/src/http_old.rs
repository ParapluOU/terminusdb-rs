//! TerminusDB HTTP Client
//!
//! ## Terminology
//!
//! This module uses the following terminology consistently:
//!
//! - **Document**: Untyped document/struct that may be formatted according to a schema.
//!   These are represented as `serde_json::Value` or similar generic types.
//!   Use `*_document` methods for working with untyped data.
//!
//! - **Model/Instance**: Strongly typed structure that adheres to a TerminusDB schema.
//!   These are Rust structs that derive `TerminusDBModel` and implement `ToTDBInstance`.
//!   Use `*_instance` methods for working with strongly typed models.
//!
//! **Recommendation**: Always prefer `*_instance` methods over `*_document` methods when
//! working with structs that derive `TerminusDBModel`, as they provide type safety and
//! better error handling.

#[cfg(not(target_arch = "wasm32"))]
use {
    futures_util::Stream,
    glob::glob,
    itertools::Itertools,
    reqwest::{Client, Response, StatusCode},
    std::process::Command,
    subprocess::{Exec, Redirection},
    tempfile::tempfile,
};

use {
    crate::{
        document::{DocumentInsertArgs, DocumentType, GetOpts},
        result::ResponseWithHeaders,
        spec::BranchSpec,
        *,
    },
    ::log::{debug, error, trace, warn},
    anyhow::{anyhow, bail, Context},
    enum_variant_macros::*,
    serde::{de::DeserializeOwned, Deserialize, Serialize},
    serde_json::{json, Value},
    std::{
        collections::{HashMap, HashSet},
        fmt::Debug,
        fs::File,
        hash::Hash,
        io::Write,
        path::PathBuf,
        time::Instant,
    },
    tap::{tap::Tap, Pipe, TapFallible},
    url::Url,
};

#[cfg(not(target_arch = "wasm32"))]
use crate::log::{CommitLogIterator, CommitState, LogEntry, LogOpts};

use terminusdb_schema::{GraphType, ToJson, ToTDBInstance};
// Add imports for woql2 and builder
use terminusdb_woql2::prelude::Query as Woql2Query;
use terminusdb_woql_builder::prelude::{node, vars, Var, WoqlBuilder}; // Import WoqlBuilder too

pub type EntityID = String;

/// Trait alias for strongly typed TerminusDB models.
///
/// This represents any type that can be converted to a TerminusDB instance,
/// is debuggable, and serializable. Use this for functions that accept
/// TerminusDB models to make the API clearer and distinguish from untyped documents.
///
/// # Example
/// ```rust
/// use terminusdb_client::TerminusDBModel;
/// 
/// async fn insert_my_model<T: TerminusDBModel>(client: &TerminusDBHttpClient, model: &T) {
///     client.insert_instance(model, args).await.unwrap();
/// }
/// ```
pub trait TerminusDBModel = ToTDBInstance + Debug + Serialize;

/// Centralized URL builder for TerminusDB API endpoints.
/// Eliminates duplication and provides consistent URL construction.
#[derive(Debug)]
struct UrlBuilder<'a> {
    endpoint: &'a Url,
    org: &'a str,
    parts: Vec<String>,
    query_params: Vec<(String, String)>,
}

impl<'a> UrlBuilder<'a> {
    fn new(endpoint: &'a Url, org: &'a str) -> Self {
        Self {
            endpoint,
            org,
            parts: Vec::new(),
            query_params: Vec::new(),
        }
    }

    /// Add an API endpoint type (db, document, woql, log, etc.)
    fn endpoint(mut self, endpoint: &str) -> Self {
        self.parts.push(endpoint.to_string());
        self
    }

    /// Add a database path (handles both normal and commit-based paths)
    fn database(mut self, spec: &BranchSpec) -> Self {
        if let Some(commit_id) = spec.commit_id() {
            self.parts.push(format!("{}/{}/local/commit/{}", self.org, spec.db, commit_id));
        } else {
            self.parts.push(format!("{}/{}", self.org, spec.db));
        }
        self
    }

    /// Add a simple database path for management operations
    fn simple_database(mut self, db: &str) -> Self {
        self.parts.push(format!("{}/{}", self.org, db));
        self
    }

    /// Add a query parameter
    fn query(mut self, key: &str, value: &str) -> Self {
        self.query_params.push((key.to_string(), value.to_string()));
        self
    }

    /// Add a query parameter with URL encoding
    fn query_encoded(mut self, key: &str, value: &str) -> Self {
        self.query_params.push((key.to_string(), urlencoding::encode(value).to_string()));
        self
    }

    /// Add multiple common document query parameters
    fn document_params(mut self, author: &str, message: &str, graph_type: &str, create: bool) -> Self {
        self.query_params.extend([
            ("author".to_string(), author.to_string()),
            ("message".to_string(), urlencoding::encode(message).to_string()),
            ("graph_type".to_string(), graph_type.to_string()),
            ("create".to_string(), create.to_string()),
        ]);
        self
    }

    /// Add document retrieval query parameters
    fn document_get_params(mut self, id: &str, unfold: bool, as_list: bool) -> Self {
        self.query_params.extend([
            ("id".to_string(), id.to_string()),
            ("unfold".to_string(), unfold.to_string()),
            ("as_list".to_string(), as_list.to_string()),
        ]);
        self
    }

    /// Add log query parameters
    fn log_params(mut self, start: usize, count: usize, verbose: bool) -> Self {
        self.query_params.extend([
            ("start".to_string(), start.to_string()),
            ("count".to_string(), count.to_string()),
            ("verbose".to_string(), verbose.to_string()),
        ]);
        self
    }

    /// Build the final URL string
    fn build(self) -> String {
        let mut url = format!("{}/{}", self.endpoint, self.parts.join("/"));
        
        if !self.query_params.is_empty() {
            let query_string = self.query_params
                .into_iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            url.push('?');
            url.push_str(&query_string);
        }
        
        url
    }
}

#[derive(Clone, Debug)]
pub struct TerminusDBHttpClient {
    pub endpoint: Url,
    // Use conditional compilation for the http client
    #[cfg(not(target_arch = "wasm32"))]
    http: Client,
    /// user auth for this user
    user: String,
    /// this user's password
    pass: String,
    /// organization that we are logging in for
    org: String,
}

// Wrap the entire impl block with a conditional compilation attribute
#[cfg(not(target_arch = "wasm32"))]
impl TerminusDBHttpClient {
    /// Creates a client connected to a local TerminusDB instance.
    ///
    /// This is a convenience constructor that connects to `http://localhost:6363`
    /// using default admin credentials. Ideal for development and testing.
    ///
    /// # Returns
    /// A client instance connected to the local TerminusDB server
    ///
    /// # Example
    /// ```rust
    /// let client = TerminusDBHttpClient::local_node().await;
    /// ```
    ///
    /// # Equivalent to
    /// ```rust
    /// TerminusDBHttpClient::new(
    ///     Url::parse("http://localhost:6363").unwrap(),
    ///     "admin", "root", "admin"
    /// ).await.unwrap()
    /// ```
    pub async fn local_node() -> Self {
        Self::new(
            Url::parse("http://localhost:6363").unwrap(),
            "admin",
            "root",
            "admin",
        )
        .await
        .unwrap()
    }

    pub async fn local_node_with_database(db: &str) -> anyhow::Result<Self> {
        let client = Self::local_node().await;
        client.ensure_database(db).await
    }

    /// Creates a client connected to a local TerminusDB instance with a test database.
    ///
    /// This is a convenience constructor that connects to a local TerminusDB server
    /// and ensures a "test" database exists. Ideal for integration tests and development.
    ///
    /// # Returns
    /// A client instance connected to the local TerminusDB server with "test" database
    ///
    /// # Example
    /// ```rust
    /// let client = TerminusDBHttpClient::local_node_test().await?;
    /// // Ready to use with "test" database
    /// ```
    pub async fn local_node_test() -> anyhow::Result<Self> {
        let client = Self::local_node().await;
        client.ensure_database("test").await
    }

    /// Creates a new TerminusDB HTTP client with custom connection parameters.
    ///
    /// # Arguments
    /// * `endpoint` - The TerminusDB server endpoint URL (will have "/api" appended)
    /// * `user` - Username for authentication
    /// * `pass` - Password for authentication  
    /// * `org` - Organization name
    ///
    /// # Returns
    /// A configured client instance
    ///
    /// # Example
    /// ```rust
    /// use url::Url;
    /// 
    /// let client = TerminusDBHttpClient::new(
    ///     Url::parse("https://my-terminusdb.com").unwrap(),
    ///     "my_user",
    ///     "my_password", 
    ///     "my_org"
    /// ).await?;
    /// ```
    pub async fn new(mut endpoint: Url, user: &str, pass: &str, org: &str) -> anyhow::Result<Self> {
        let err = format!("Cannot modify segments for endpoint: {}", &endpoint);

        endpoint.path_segments_mut().expect(&err).push("api");

        Ok(Self {
            user: user.to_string(),
            pass: pass.to_string(),
            endpoint,
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()?,
            org: org.to_string(),
        })
    }

    pub async fn new_with_database(
        endpoint: Url,
        user: &str,
        pass: &str,
        db: &str,
        org: &str,
    ) -> anyhow::Result<Self> {
        let client = Self::new(endpoint, user, pass, org).await?;
        client.ensure_database(db).await
    }

    // pub fn doc_replace_all_json_files_stream(
    //     &self,
    //     docs: std::path::PathBuf,
    //     graph_type: GraphType,
    //     db: String,
    //     insert_if_not_exists: bool,
    // ) -> impl Stream<Item = TerminusDBResult<std::path::PathBuf>> + '_ {
    //     async_stream::stream! {
    //         for e in glob(format!("{}/*.json", docs.display()).as_str()).expect("Failed to read glob pattern") {
    //             let e2 = e.expect("PathBuf was empty");
    //             yield self.doc_replace_all_json_file(
    //                 e2.clone(),
    //                 graph_type,
    //                 db.clone(),
    //                 insert_if_not_exists).map(|_| e2);
    //         }
    //         for e in glob(format!("{}/*.json.zip", docs.display()).as_str()).expect("Failed to read glob pattern") {
    //             let e2 = e.expect("PathBuf was empty");
    //             yield self.doc_replace_all_json_file(
    //                 e2.clone(),
    //                 graph_type,
    //                 db.clone(),
    //                 insert_if_not_exists).map(|_| e2);
    //         }
    //     }
    // }

    /// Centralized URL builder for TerminusDB API endpoints.
    /// Handles all URL construction patterns and eliminates duplication.
    fn build_url(&self) -> UrlBuilder {
        UrlBuilder::new(&self.endpoint, &self.org)
    }


    /// Ensures a database exists, creating it if it doesn't exist.
    ///
    /// This function will create a new database with the given name if it doesn't already exist.
    /// If the database already exists, this function succeeds without modification.
    ///
    /// # Arguments
    /// * `db` - The name of the database to ensure exists
    ///
    /// # Returns
    /// A cloned instance of the client configured for the database
    ///
    /// # Example
    /// ```rust
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let client_with_db = client.ensure_database("my_database").await?;
    /// ```
    pub async fn ensure_database(&self, db: &str) -> anyhow::Result<Self> {
        let uri = self.build_url().endpoint("db").simple_database(db).build();

        debug!("post uri: {}", &uri);

        // todo: author should probably be node name
        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(
                json!({
                    "comment": "Song database specific for this node",
                    "label": db,
                    "public": true,
                    "schema": true
                })
                .to_string(),
            )
            .send()
            .await
            .context("failed to ensure database")?;

        // todo: use parse_response()
        if ![200, 400].contains(&res.status().as_u16()) {
            error!("could not ensure database");

            Err(TerminusDBAdapterError::Other(format!(
                "request failed: {:#?}",
                res.text().await?
            )))?;
        }

        // todo: dont print if it already existed
        debug!("ensured database {}", db);

        Ok(self.clone())
    }

    /// Deletes a database permanently.
    ///
    /// **Warning**: This operation is irreversible and will permanently delete
    /// all data in the specified database.
    ///
    /// # Arguments
    /// * `db` - The name of the database to delete
    ///
    /// # Returns
    /// A cloned instance of the client
    ///
    /// # Example
    /// ```rust
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.delete_database("old_database").await?;
    /// ```
    pub async fn delete_database(&self, db: &str) -> anyhow::Result<Self> {
        let uri = self.build_url().endpoint("db").simple_database(db).build();

        debug!("deleting database {}", db);

        self.http
            .delete(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to delete database")?;

        Ok(self.clone())
    }

    /// Resets a database by deleting it and recreating it.
    ///
    /// This is useful when you encounter schema failures due to model structure changes.
    /// It performs a `delete_database()` followed by `ensure_database()`.
    ///
    /// # Arguments
    /// * `db` - The name of the database to reset
    ///
    /// # Example
    /// ```rust
    /// // Reset the database to clear old schemas
    /// client.reset_database("my_db").await?;
    /// ```
    pub async fn reset_database(&self, db: &str) -> anyhow::Result<Self> {
        debug!("resetting database {}", db);
        
        self.delete_database(db)
            .await
            .context("failed to delete database during reset")?;
            
        self.ensure_database(db)
            .await
            .context("failed to recreate database during reset")
    }

    /// Inserts the schema for a strongly-typed model into the database.
    ///
    /// This function automatically generates and inserts the schema definition
    /// for a type that implements `ToTDBSchema` (typically via `#[derive(TerminusDBModel)]`).
    ///
    /// # Type Parameters
    /// * `S` - A type that implements `ToTDBSchema` (derives `TerminusDBModel`)
    ///
    /// # Arguments
    /// * `args` - Document insertion arguments specifying the database and branch
    ///
    /// # Example
    /// ```rust
    /// #[derive(TerminusDBModel)]
    /// struct User { name: String, age: i32 }
    ///
    /// // Insert the schema for the User type
    /// client.insert_entity_schema::<User>(args).await?;
    /// 
    /// // Now you can insert User instances
    /// let user = User { name: "Alice".to_string(), age: 30 };
    /// client.insert_instance(&user, args).await?;
    /// ```
    ///
    /// # Note
    /// The database will be automatically created if it doesn't exist.
    #[pseudonym::alias(schema)]
    pub async fn insert_entity_schema<S: ToTDBSchema>(
        &self,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<()> {
        self.ensure_database(&args.spec.db)
            .await
            .context("ensuring database")?;

        let root = S::to_schema();

        debug!("inserting entity schema for {}...", root.class_name());

        let subs = S::to_schema_tree();

        // panic!("{:#?}", &subs);

        self.insert_documents(subs.iter().collect(), args.as_schema())
            .await
            .context("insert_documents()")?;

        debug!("inserted schema into TerminusDB");

        Ok(())
    }

    /// Inserts a raw schema definition into the database.
    ///
    /// **⚠️ Consider using the strongly-typed alternative instead:**
    /// - [`insert_entity_schema`](Self::insert_entity_schema) for typed model schemas
    ///
    /// This function inserts a manually constructed schema definition. It's typically
    /// used for advanced scenarios or when working with dynamic schemas.
    ///
    /// # Arguments
    /// * `schema` - The schema definition to insert
    /// * `args` - Document insertion arguments specifying the database and branch
    ///
    /// # Returns
    /// A cloned instance of the client
    ///
    /// # Example
    /// ```rust
    /// use terminusdb_schema::Schema;
    /// 
    /// let schema = Schema::Class { /* schema definition */ };
    /// client.insert_schema(&schema, args).await?;
    /// ```
    pub async fn insert_schema(
        &self,
        schema: &crate::Schema,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<Self> {
        self.insert_document(
            schema,
            args.tap_mut(|a| {
                a.ty = DocumentType::Schema;
            }),
        )
        .await
    }

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
        args: DocumentInsertArgs,
    ) -> bool {
        match model.to_instance(None).gen_id() {
            None => {
                return false;
            }
            Some(id) => self.has_document(&id, args.as_ref()).await,
        }
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

        if !args.force && self.has_instance(model, args.clone()).await {
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
    async fn find_commit_for_instance<I: TerminusDBModel>(
        &self,
        instance_id: &str,
        spec: &BranchSpec,
    ) -> anyhow::Result<String> {
        use futures_util::StreamExt;
        
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

    /// Helper method to get all entity IDs created in a commit, regardless of type
    async fn all_commit_created_entity_ids_any_type(
        &self,
        spec: &BranchSpec,
        commit: &LogEntry,
    ) -> anyhow::Result<Vec<EntityID>> {
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
        
        let res: WOQLResult<Value> = match tokio::time::timeout(timeout_duration, query_future).await {
            Ok(result) => result?,
            Err(_) => {
                error!("Query timed out after 30 seconds for commit {}", &commit.identifier);
                return Err(anyhow!("Query timed out"));
            }
        };

        debug!("Query returned {} bindings", res.bindings.len());

        let err = format!("failed to deserialize from Value: {:#?}", &res);

        #[derive(Deserialize)]
        struct ObjectFormat {
            pub id: String,
        }

        let result: Vec<EntityID> = res
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

    // pub fn insert_instance_chunked<I: TerminusDBModel>(
    //     &self,
    //     model: &I,
    //     args: DocumentInsertArgs,
    // ) -> anyhow::Result<&Self> {
    //     let mut instances = model.to_instance_tree();
    //
    //     for instance in &mut instances {
    //         instance.set_random_key_prefix();
    //     }
    //
    //     let mut models = instances.iter().collect();
    //
    //     dedup_instances_by_id(&mut models);
    //
    //     for instance in instances.iter().rev() {
    //         self.insert_document(instance, args.clone())?;
    //     }
    //
    //     Ok(self)
    // }

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

    pub async fn insert_documents_by_schema_type<T: ToTDBSchema>(
        &self,
        mut model: Vec<&Instance>,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>> {
        // dedup_instances_by_id(&mut model);

        let selection = model
            .into_iter()
            .filter(|instance| instance.schema.class_name() == &T::schema_name())
            .collect::<Vec<_>>();

        self.insert_documents(selection, args).await
    }

    /// Inserts multiple untyped documents into the database.
    ///
    /// **⚠️ Consider using strongly-typed alternatives instead:**
    /// - [`insert_instances`](Self::insert_instances) for multiple typed models
    /// - [`insert_instance`](Self::insert_instance) for single typed models
    ///
    /// This function accepts any type that implements `ToJson` (like `serde_json::Value`
    /// or schema definitions) and inserts them as untyped documents.
    ///
    /// # Arguments
    /// * `model` - Vector of references to objects that can be converted to JSON
    /// * `args` - Document insertion arguments specifying the database, branch, and options
    ///
    /// # Returns
    /// A `ResponseWithHeaders` containing:
    /// - `data`: HashMap with document IDs and insert results
    /// - `commit_id`: Optional commit ID from the TerminusDB-Data-Version header
    ///
    /// # Example
    /// ```rust
    /// use serde_json::json;
    /// 
    /// let docs = vec![
    ///     &json!({"@type": "Person", "name": "Alice"}),
    ///     &json!({"@type": "Person", "name": "Bob"}),
    /// ];
    /// let result = client.insert_documents(docs, args).await?;
    /// ```
    pub async fn insert_documents(
        &self,
        model: Vec<&impl ToJson>,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>>> {
        let ty = args.ty.to_string().to_lowercase();

        let uri = self.build_url()
            .endpoint("document")
            .database(&args.spec)
            .document_params(&args.author, &args.message, &ty, true)
            .build();

        let mut to_jsoned = model
            .into_iter()
            .map(|t| t.to_json())
            .rev()
            .collect::<Vec<_>>();

        // for (i, j) in &mut to_jsoned.iter_mut().enumerate() {
        //     j.as_object_mut()
        //         .unwrap()
        //         .insert("@capture".to_string(), format!("v{}", i).into());
        // }

        dedup_documents_by_id(&mut to_jsoned);

        eprintln!("inserting document(s): {:#?}", &to_jsoned);

        let json = serde_json::to_string(&to_jsoned).unwrap();

        trace!(
            "about to insert {} at URI {}: {:#?}",
            &ty,
            &uri,
            // todo: add trailing "..." or "(truncated)" if message was longer
            &json[0..(10000.min(json.len()))]
        );

        if args.ty.is_instance() {
            // panic!("{:#?}", &to_jsoned);
        }

        // todo: author should probably be node name
        // todo: dont clone just for the failed paload dumping
        let res = self
            .http
            .put(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(json.clone())
            .send()
            .await?;

        let parsed = self.parse_response_with_headers::<Vec<String>>(res).await;

        trace!("parsed response from insert_documents(): {:#?}", &parsed);

        if let Err(e) = &parsed {
            error!("request error: {:#?}", &e);
            dump_failed_payload(&json);
            return Err(anyhow!("Insert request failed: {}", e));
        }

        debug!("inserted {} into TerminusDB", ty);

        let parsed = parsed?;
        let version_header = parsed.commit_id.clone();
        let data = parsed.into_inner();
        let result_map = data
            .into_iter()
            .map(|id| (id.clone(), TDBInsertInstanceResult::Inserted(id)))
            .collect();

        Ok(ResponseWithHeaders::new(result_map, version_header))
    }

    /// Inserts a single untyped document into the database.
    ///
    /// **⚠️ Consider using the strongly-typed alternative instead:**
    /// - [`insert_instance`](Self::insert_instance) for typed models
    ///
    /// This function accepts any type that implements `ToJson` (like `serde_json::Value`
    /// or schema definitions) and inserts it as an untyped document.
    ///
    /// # Arguments
    /// * `model` - A reference to an object that can be converted to JSON
    /// * `args` - Document insertion arguments specifying the database, branch, and options
    ///
    /// # Returns
    /// A cloned instance of the client (note: does not include commit ID information)
    ///
    /// # Example
    /// ```rust
    /// use serde_json::json;
    /// 
    /// let doc = json!({"@type": "Person", "name": "Alice"});
    /// client.insert_document(&doc, args).await?;
    /// ```
    ///
    /// # Note
    /// This function returns the client instance but does not provide access to
    /// the commit ID. Use [`insert_documents`](Self::insert_documents) if you need
    /// commit information from headers.
    pub async fn insert_document(
        &self,
        model: &impl ToJson,
        args: DocumentInsertArgs,
    ) -> anyhow::Result<Self> {
        let json = model.to_json();

        let ty = args.ty.to_string().to_lowercase();

        let uri = self.build_url()
            .endpoint("document")
            .database(&args.spec)
            .document_params(&args.author, &args.message, &ty, true)
            .build();

        debug!("about to insert {} at URI {}: {:#?}", &ty, &uri, &json);

        // todo: author should probably be node name
        let res = self
            .http
            .put(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(json.to_string())
            .send()
            .await?;

        self.parse_response::<Value>(res).await?;

        debug!("inserted {} into TerminusDB", ty);

        Ok(self.clone())
    }

    #[pseudonym::alias(verify_connection)]
    pub async fn info(&self) -> anyhow::Result<Info> {
        let uri = self.build_url().endpoint("info").build();
        debug!(
            "📡 Making HTTP request to TerminusDB info endpoint: {}",
            &uri
        );

        let res = self
            .http
            .get(uri.clone())
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context(format!("failed to parse response for {}", &uri))?;

        debug!("📨 Received response from TerminusDB, parsing...");
        self.parse_response(res).await
    }

    pub async fn is_running(&self) -> bool {
        self.info().await.is_ok()
    }

    // returns commit log entries from new to old
    // todo: accept parameter to define ordering
    pub async fn log(&self, spec: &BranchSpec, opts: LogOpts) -> anyhow::Result<Vec<LogEntry>> {
        let LogOpts {
            offset,
            verbose,
            count,
        } = opts;

        let uri = self.build_url()
            .endpoint("log")
            .simple_database(&spec.db)
            .log_params(offset.unwrap_or_default(), count.unwrap_or(10), verbose)
            .build();

        debug!("retrieving log at {}...", &uri);

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await?;

        self.parse_response(res).await
    }

    async fn parse_response<T: DeserializeOwned + Debug>(
        &self,
        res: Response,
    ) -> anyhow::Result<T> {
        let json = res.json::<serde_json::Value>().await?;

        trace!("[TerminusDBHttpClient] response: {:#?}", &json);
        eprintln!("parsed response: {:#?}", &json);

        let response_has_error_prop = json.get("api:error").is_some();
        let err = format!("failed to deserialize into ApiResponse: {:#?}", &json);
        let res: ApiResponse<T> = serde_json::from_value(json).context(err.clone())?;

        eprintln!("parsed typed response: {:#?}", &res);

        match res {
            ApiResponse::Success(r) => {
                assert!(!response_has_error_prop, "{}", err);
                Ok(r)
            }
            ApiResponse::Error(err) => return Err(err.into()),
        }
    }

    async fn parse_response_with_headers<T: DeserializeOwned + Debug>(
        &self,
        res: Response,
    ) -> anyhow::Result<ResponseWithHeaders<T>> {
        // Extract the TerminusDB-Data-Version header before consuming the response
        let terminusdb_data_version = res
            .headers()
            .get("TerminusDB-Data-Version")
            .and_then(|value| value.to_str().ok())
            .map(|s| s.to_string());

        trace!("[TerminusDBHttpClient] TerminusDB-Data-Version header: {:?}", terminusdb_data_version);

        let json = res.json::<serde_json::Value>().await?;

        trace!("[TerminusDBHttpClient] response: {:#?}", &json);
        eprintln!("parsed response: {:#?}", &json);

        let response_has_error_prop = json.get("api:error").is_some();
        let err = format!("failed to deserialize into ApiResponse: {:#?}", &json);
        let res: ApiResponse<T> = serde_json::from_value(json).context(err.clone())?;

        eprintln!("parsed typed response: {:#?}", &res);

        match res {
            ApiResponse::Success(r) => {
                assert!(!response_has_error_prop, "{}", err);
                Ok(ResponseWithHeaders::new(r, terminusdb_data_version))
            }
            ApiResponse::Error(err) => return Err(err.into()),
        }
    }

    pub async fn log_iter(&self, db: BranchSpec, opts: LogOpts) -> CommitLogIterator {
        CommitLogIterator::new(self.clone(), db, opts)
    }

    pub async fn entity_iter<
        T: TerminusDBModel + 'static,
        Deser: TDBInstanceDeserializer<T> + 'static,
    >(
        &self,
        spec: BranchSpec,
        deserializer: Deser,
        opts: LogOpts,
    ) -> EntityIterator<T, Deser>
    where
        T: Send,
        Deser: Send,
    {
        EntityIterator::new(
            CommitLogIterator::new(self.clone(), spec, opts),
            deserializer,
        )
    }

    pub async fn commit_added_entities_query<T: ToTDBInstance>(
        &self,
        spec: &BranchSpec,
        commit: &LogEntry,
        limit: Option<usize>,
    ) -> Woql2Query {
        // Build the query using WoqlBuilder
        let db_collection = format!("{}/{}", &self.org, &spec.db);
        let commit_collection = format!("commit/{}", &commit.identifier);
        let type_node_str = format!("@schema:{}", T::schema_name());

        let id_var = vars!("id"); // Define the variable 'id'

        // Start from the innermost query and wrap outwards
        let query_builder = WoqlBuilder::new()
            .added_triple(
                id_var.clone(),             // subject: variable "id"
                "rdf:type",                 // predicate: node "rdf:type"
                node(&type_node_str),       // object: node "@schema:MyType" - Use T::schema_name()
                GraphType::Instance.into(), // graph: "instance" - This makes it AddedQuad conceptually
            )
            .using(commit_collection) // Wrap in commit collection Using
            .using(db_collection) // Wrap in db collection Using
            .limit(limit.unwrap_or(1000) as u64); // Apply the limit

        // Finalize the query into the woql2 structure
        query_builder.finalize()
    }

    pub async fn commit_added_entities_ids<T: ToTDBInstance>(
        &self,
        spec: &BranchSpec,
        commit: &LogEntry,
        limit: Option<usize>,
    ) -> anyhow::Result<Vec<EntityID>> {
        // form the query using the builder
        let woql_query = self
            .commit_added_entities_query::<T>(spec, commit, limit)
            .await;

        // Serialize the woql2::Query to JSON-LD
        let json_query = woql_query.to_instance(None).to_json();

        // perform the query using the serialized JSON
        let res = self.query_raw(Some(spec.clone()), json_query).await?;

        // format the error message in case there is a mismatch in deserialization format
        let err = format!("failed to deserialize from Value: {:#?}", &res);

        #[derive(Deserialize)]
        struct ObjectFormat {
            pub id: String,
        }

        // try deserialization
        Ok(res
            .bindings
            .into_iter()
            .map(|bind| serde_json::from_value::<ObjectFormat>(bind))
            .collect::<Result<Vec<_>, _>>()
            .context(err)?
            .into_iter()
            // this is a bit hacky to shave off the schema name, but we dont use that internally
            // todo: when the WOQL query stuff has matured more this should be a helper on
            // the result wrapper struct
            .map(|obj| obj.id.split("/").last().unwrap().to_string())
            .collect())
    }

    /// return ID for first entity of given type that was created by the given commit
    pub async fn first_commit_created_entity_id<T: ToTDBInstance>(
        &self,
        db: &BranchSpec,
        commit: &LogEntry,
    ) -> anyhow::Result<Option<String>> {
        Ok(self
            .commit_added_entities_ids::<T>(db, commit, Some(1))
            .await?
            .first()
            .cloned())
    }

    pub async fn first_commit_created_entity<T: ToTDBInstance>(
        &self,
        spec: &BranchSpec,
        commit: &LogEntry,
        deserializer: &mut impl TDBInstanceDeserializer<T>,
    ) -> anyhow::Result<Option<T>> {
        match self
            .first_commit_created_entity_id::<T>(spec, commit)
            .await?
        {
            None => Ok(None),
            Some(id) => self
                .get_instance::<T>(&id, spec, deserializer)
                .await
                .map(Some),
        }
    }

    pub async fn all_commit_created_entity_ids<T: ToTDBInstance>(
        &self,
        db: &BranchSpec,
        commit: &LogEntry,
    ) -> anyhow::Result<Vec<EntityID>> {
        self.commit_added_entities_ids::<T>(db, commit, Some(1000))
            .await
    }

    pub async fn all_commit_created_entities<T: ToTDBInstance>(
        &self,
        spec: &BranchSpec,
        commit: &LogEntry,
        deserializer: &mut impl TDBInstanceDeserializer<T>,
    ) -> anyhow::Result<Vec<T>> {
        let entity_ids = self
            .all_commit_created_entity_ids::<T>(spec, commit)
            .await?;

        let mut results = Vec::with_capacity(entity_ids.len());
        for id in entity_ids {
            results.push(
                self.get_instance::<T>(&id, spec, deserializer)
                    .await
                    .context(format!(
                        "retrieving entity {} #{} from TDB",
                        std::any::type_name::<T>(),
                        &id
                    ))?,
            );
        }

        Ok(results)
    }

    // pub fn resolve_objects_from_commit<T: TerminusDBModel>(
    //     &self,
    //     spec: &BranchSpec,
    //     deserializer: &mut impl TDBInstanceDeserializer<T>,
    //     commit: &CommitState,
    // ) -> anyhow::Result<Vec<T>> {
    //     self.resolve_objects(spec, deserializer, commit.all_added_entities::<T>())
    // }

    // pub fn resolve_objects<T: TerminusDBModel>(
    //     &self,
    //     spec: &BranchSpec,
    //     deserializer: &mut impl TDBInstanceDeserializer<T>,
    //     objects: Vec<&ObjectState>,
    // ) -> anyhow::Result<Vec<T>> {
    //     objects
    //         .into_iter()
    //         .map(|b| self.get_instance::<T>(&b.id, spec, deserializer))
    //         .collect()
    // }

    /// Checks if an untyped document exists in the database by ID.
    ///
    /// **⚠️ Consider using the strongly-typed alternative instead:**
    /// - [`has_instance`](Self::has_instance) for typed models
    ///
    /// This function checks for the existence of a document by its raw ID string.
    /// It works with any document type but provides no type safety.
    ///
    /// # Arguments
    /// * `id` - The document ID (number only, no schema class prefix)
    /// * `spec` - Branch specification indicating which branch to check
    ///
    /// # Returns
    /// `true` if the document exists, `false` otherwise
    ///
    /// # Example
    /// ```rust
    /// let exists = client.has_document("12345", &branch_spec).await;
    /// ```
    pub async fn has_document(
        &self, // number only, no schema class prefix
        id: &str,
        spec: &BranchSpec,
    ) -> bool {
        let r: anyhow::Result<_> = async {
            let uri = self.build_url()
                .endpoint("document")
                .database(spec)
                .document_get_params(id, false, false)
                .build();

            Ok::<Value, anyhow::Error>(
                self.parse_response::<Value>(
                    self.http
                        .get(uri)
                        .basic_auth(&self.user, Some(&self.pass))
                        .send()
                        .await?,
                )
                .await?,
            )
        }
        .await;

        r.is_ok()
    }

    /// ID here should manually include the type, like
    /// "Song/myid"
    /// Retrieves an untyped document from the database by ID.
    ///
    /// **⚠️ Consider using the strongly-typed alternative instead:**
    /// - [`get_instance`](Self::get_instance) for typed models with deserialization
    ///
    /// This function retrieves a document by its raw ID string and returns it
    /// as an untyped `serde_json::Value`. It provides no type safety or automatic
    /// deserialization.
    ///
    /// # Arguments
    /// * `id` - The document ID (number only, no schema class prefix)
    /// * `spec` - Branch specification indicating which branch to query
    /// * `opts` - Get options for controlling the query behavior
    ///
    /// # Returns
    /// The document as a `serde_json::Value`
    ///
    /// # Example
    /// ```rust
    /// let doc = client.get_document("12345", &branch_spec, GetOpts::default()).await?;
    /// let name = doc["name"].as_str().unwrap();
    /// ```
    ///
    /// # Note
    /// For time-travel queries, use a branch specification with a commit ID:
    /// ```rust
    /// let past_spec = BranchSpec::from("main/local/commit/abc123");
    /// let old_doc = client.get_document("12345", &past_spec, GetOpts::default()).await?;
    /// ```
    pub async fn get_document(
        &self, // number only, no schema class prefix
        id: &str,
        spec: &BranchSpec,
        opts: GetOpts,
    ) -> anyhow::Result<serde_json::Value> {
        if !self.has_document(id, spec).await {
            Err(anyhow!("document #{} does not exist", id))?
        }

        let uri = self.build_url()
            .endpoint("document")
            .database(spec)
            .document_get_params(id, opts.unfold, opts.as_list)
            .build();

        debug!("retrieving document at {}...", &uri);

        let start = Instant::now();

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await?;

        debug!("retrieved TDB document with status code: {}", res.status());

        let res = self.parse_response::<Value>(res).await?;

        debug!("retrieved TDB document in {:?}", start.elapsed());

        Ok(res)
    }

    pub fn format_id<T: ToTDBInstance>(id: &str) -> String {
        if id.contains("/") {
            id.to_string()
        } else {
            format!("{}/{}", T::schema_name(), id)
        }
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
        let doc_id = Self::format_id::<Target>(id);

        let json_instance_doc = self.get_document(&doc_id, spec, Default::default()).await?;

        let res = deserializer.from_instance(json_instance_doc.clone());

        match res {
            Ok(t) => Ok(t),
            Err(err) => Err(err).context(format!(
                "TerminusHTTPClient failed to deserialize Instance. See: {}",
                dump_json(&json_instance_doc).display()
            )),
        }
    }

    // Refactor query to accept Woql2Query
    /// Executes a WOQL query and returns typed results.
    ///
    /// This function executes Web Object Query Language (WOQL) queries against TerminusDB
    /// and deserializes the results into the specified type.
    ///
    /// # Type Parameters
    /// * `T` - The type to deserialize query results into
    ///
    /// # Arguments
    /// * `db` - Optional branch specification (if None, uses client's default)
    /// * `query` - The WOQL query to execute
    ///
    /// # Returns
    /// WOQL query results deserialized to type `T`
    ///
    /// # Example
    /// ```rust
    /// use terminusdb_woql2::Query;
    /// 
    /// let query = Query::select().triple("v:Subject", "v:Predicate", "v:Object");
    /// let results: WOQLResult<HashMap<String, Value>> = client.query(Some(spec), query).await?;
    /// ```
    pub async fn query<T: Debug + DeserializeOwned>(
        &self,
        db: Option<BranchSpec>,
        query: Woql2Query, // Changed input type
    ) -> anyhow::Result<WOQLResult<T>> {
        // Serialize the query to JSON-LD here
        let json_query = query.to_instance(None).to_json();
        let woql_context = crate::Context::woql().to_json();
        // self.query_raw(db, Value::Array(vec!(woql_context, json_query))).await // Pass serialized JSON to query_raw
        self.query_raw(db, json_query).await // Pass serialized JSON to query_raw
    }

    // query_raw remains the same, accepting serde_json::Value
    pub async fn query_raw<T: Debug + DeserializeOwned>(
        &self,
        spec: Option<BranchSpec>,
        query: serde_json::Value,
    ) -> anyhow::Result<WOQLResult<T>> {
        let uri = match spec {
            None => {
                self.build_url().endpoint("woql").build()
            }
            Some(spc) => {
                self.build_url().endpoint("woql").simple_database(&spc.db).build()
            }
        };

        eprintln!("querying at {}...: {:#?}", &uri, &query);

        let json = json!({
            "query": query
        });

        let json = serde_json::to_string(&json).unwrap();

        trace!("payload: {}", &json);

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(json)
            .send()
            .await?;

        let json = self.parse_response(res).await?;

        trace!("query result: {:#?}", &json);

        Ok(json)
    }

    // query_raw_with_headers - similar to query_raw but captures TerminusDB-Data-Version header
    pub async fn query_raw_with_headers<T: Debug + DeserializeOwned>(
        &self,
        spec: Option<BranchSpec>,
        query: serde_json::Value,
    ) -> anyhow::Result<ResponseWithHeaders<WOQLResult<T>>> {
        let uri = match spec {
            None => {
                self.build_url().endpoint("woql").build()
            }
            Some(spc) => {
                self.build_url().endpoint("woql").simple_database(&spc.db).build()
            }
        };

        eprintln!("querying at {}...: {:#?}", &uri, &query);

        let json = json!({
            "query": query
        });

        let json = serde_json::to_string(&json).unwrap();

        trace!("payload: {}", &json);

        let res = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(json)
            .send()
            .await?;

        let json = self.parse_response_with_headers(res).await?;

        trace!("query result: {:#?}", &json);

        Ok(json)
    }

    // todo: roll into ORM-like model
    pub async fn query_instances<T: TdbModel + InstanceFromJson>(
        &self,
        spec: &BranchSpec,
        limit: Option<usize>,
        offset: Option<usize>,
        query: impl InstanceQueryable<Model = T>,
    ) -> anyhow::Result<Vec<T>> {
        query.apply(self, spec, limit, offset).await
    }

    // todo: roll into ORM-like model
    pub async fn list_instances<T: TdbModel + InstanceFromJson>(
        &self,
        spec: &BranchSpec,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> anyhow::Result<Vec<T>> {
        self.query_instances(spec, limit, offset, ListModels::<T>::default())
            .await
    }

    /// Count the total number of instances of a specific type in the database
    pub async fn count_instances<T: ToTDBSchema>(
        &self,
        spec: &BranchSpec,
    ) -> anyhow::Result<usize> {
        let count_var = vars!("Count");
        let instance_var = vars!("Instance");

        // Build a query to count instances of the specific type using the isa2 shortcut
        let query = WoqlBuilder::new()
            .isa2::<T>(&instance_var)
            .count(count_var.clone())
            .select(vec![count_var.clone()])
            .finalize();

        #[derive(Deserialize, Debug)]
        struct CountResultBinding {
            #[serde(rename = "@value")]
            value: u64,
        }

        // Execute the query
        let result = self
            .query::<std::collections::HashMap<String, CountResultBinding>>(
                Some(spec.clone()),
                query,
            )
            .await?;

        // Extract count from the result
        /*
            parsed typed response: Success(
            WOQLResult {
                api_status: Success,
                api_variable_names: [
                    "Count",
                ],
                bindings: [
                    {
                        "Count": Object {
                            "@type": String("xsd:decimal"),
                            "@value": Number(1),
                        },
                    },
                ],
                deletes: 0,
                inserts: 0,
                transaction_retry_count: 0,
            },
        )
         */
        if let Some(binding) = result.bindings.first() {
            let CountResultBinding { value } = binding
                .get(&*count_var)
                .ok_or_else(|| anyhow!("Count variable not found in result"))?;

            return Ok(*value as usize);
        }

        Ok(0)
    }
}

// Add a separate impl block for WASM
#[cfg(target_arch = "wasm32")]
impl TerminusDBHttpClient {
    // Implement a stub or alternative implementation for WASM
    // This is just a basic example, you'll need to adjust based on your needs
    pub async fn new(endpoint: Url, user: &str, pass: &str, org: &str) -> anyhow::Result<Self> {
        Ok(Self {
            endpoint,
            user: user.to_string(),
            pass: pass.to_string(),
            org: org.to_string(),
        })
    }

    // Implement other methods as needed for WASM
}

#[derive(Debug, Clone, PartialEq)]
pub enum TDBInsertInstanceResult {
    /// inserted entity, returning ID
    Inserted(String),
    /// entity already exists, returning ID
    AlreadyExists(String),
}

//
// HELPERS
// todo: move to helpers module
//

pub fn dedup_instances_by_id(instances: &mut Vec<&Instance>) {
    let mut seen_ids = HashSet::new();
    instances.retain(|item| {
        match &item.id {
            Some(id) => seen_ids.insert(id.clone()), // insert returns true if the value was not present in the set
            None => true,                            // keep items with None id
        }
    });
}

fn dedup_documents_by_id(values: &mut Vec<Value>) {
    let mut seen_ids = HashSet::new();
    values.retain(|value| {
        if let Some(id) = value.get("@id").and_then(|id| id.as_str()) {
            seen_ids.insert(id.to_string())
        } else {
            true
        }
    });
}

fn dump_failed_payload(payload: &str) {
    // Get the current datetime
    let current_datetime = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();

    // Define the log filename with the datetime
    let log_filename = format!("tdb-failed-request-{}.log.json", current_datetime);

    // Write the string to the log file
    let mut file = match File::create(&log_filename) {
        Ok(file) => file,
        Err(e) => panic!("Could not create file: {}", e),
    };

    match file.write_all(payload.as_bytes()) {
        Ok(_) => debug!(
            "Successfully dumped failed request payload to file {}",
            log_filename
        ),
        Err(e) => panic!("Could not write to file: {}", e),
    };
}

fn dump_schema<S: ToTDBSchema>() {
    // Get the current datetime
    let current_datetime = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();

    // Define the log filename with the datetime
    let log_filename = format!("tdb-failed-schema-{}.log.json", current_datetime);

    let schema_json = serde_json::Value::Array(
        S::to_schema_tree()
            .into_iter()
            .map(|s| s.to_json())
            .collect(),
    );

    let payload = serde_json::to_string_pretty(&schema_json).unwrap();

    // Write the string to the log file
    let mut file = match File::create(&log_filename) {
        Ok(file) => file,
        Err(e) => panic!("Could not create file: {}", e),
    };

    match file.write_all(payload.as_bytes()) {
        Ok(_) => debug!(
            "Successfully dumped failed request payload to file {}",
            log_filename
        ),
        Err(e) => panic!("Could not write to file: {}", e),
    };
}

fn dump_json(json: &Value) -> PathBuf {
    // Get the current datetime
    let current_datetime = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();

    // Define the log filename with the datetime
    let log_filename = format!("tdb-retrieved-json-{}.log.json", current_datetime);

    let payload = serde_json::to_string_pretty(json).unwrap();

    // Write the string to the log file
    let mut file = match File::create(&log_filename) {
        Ok(file) => file,
        Err(e) => panic!("Could not create file: {}", e),
    };

    match file.write_all(payload.as_bytes()) {
        Ok(_) => {
            debug!(
                "Successfully dumped success response to file {}",
                log_filename
            );
            PathBuf::from(log_filename)
        }
        Err(e) => panic!("Could not write to file: {}", e),
    }
}

// #[test]
// fn test_remote_query() {
//     let client =
//         TerminusDBHttpClient::new("http://51.68.146.185:6363", "admin", "root", "admin").unwrap();
//
//     let cnt = var!(Cnt);
//     let n = var!(N);
//
//     let q = { select!([cnt], count(distinct!([n], is_a(n, "ScoreTree")), cnt)) };
//
//     println!("{}", &q.to_rest_query());
//
//     // (count(distinct([N], isa(N, 'ScoreTree')), Cnt))
//     let res = client.query("admin/scores", q);
//
//     dbg!(&res);
//
//     assert!(res.is_ok());
// }
