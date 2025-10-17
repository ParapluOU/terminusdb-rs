//! Core HTTP client struct and constructors

#[cfg(not(target_arch = "wasm32"))]
use reqwest::Client;

use {
    crate::{Info, TerminusDBAdapterError, debug::{DebugConfig, OperationLog, QueryLogger, QueryLogEntry, OperationFilter}},
    ::tracing::{debug, instrument},
    anyhow::Context,
    serde::{de::DeserializeOwned, Deserialize, Serialize},
    std::{env, fmt::Debug, sync::{Arc, Mutex, RwLock}, collections::HashSet, time::Duration},
    terminusdb_schema::{ToTDBInstance, ToJson},
    url::Url,
};

use super::url_builder::UrlBuilder;

#[derive(Clone)]
pub struct TerminusDBHttpClient {
    pub endpoint: Url,
    // Use conditional compilation for the http client
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) http: Client,
    /// user auth for this user
    pub(crate) user: String,
    /// this user's password
    pub(crate) pass: String,
    /// organization that we are logging in for
    pub(crate) org: String,
    /// Operation log for debugging
    pub(crate) operation_log: OperationLog,
    /// Query logger for persistent logging
    pub(crate) query_logger: Arc<RwLock<Option<QueryLogger>>>,
    /// Debug configuration
    pub(crate) debug_config: Arc<RwLock<DebugConfig>>,
    /// Cache of ensured databases to avoid repeated ensure_database calls
    pub(crate) ensured_databases: Arc<Mutex<HashSet<String>>>,
    /// Centralized SSE manager for change listeners (lazily initialized)
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) sse_manager: Arc<RwLock<Option<Arc<super::sse_manager::SseManager>>>>,
}

// Wrap the entire impl block with a conditional compilation attribute
#[cfg(not(target_arch = "wasm32"))]
impl TerminusDBHttpClient {
    /// Creates a client connected to a local TerminusDB instance.
    ///
    /// This is a convenience constructor that connects to `http://localhost:6363`
    /// using default admin credentials. Ideal for development and testing.
    ///
    /// The password is determined by checking environment variables in order:
    /// 1. `TERMINUSDB_ADMIN_PASS` - for Docker image compatibility
    /// 2. `TERMINUSDB_PASS` - existing convention
    /// 3. Falls back to "root" if neither is set
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
    #[instrument(name = "terminus.client.local_node")]
    pub async fn local_node() -> Self {
        // Check for password in environment variables
        let password = env::var("TERMINUSDB_ADMIN_PASS")
            .or_else(|_| env::var("TERMINUSDB_PASS"))
            .unwrap_or_else(|_| "root".to_string());
        
        Self::new(
            Url::parse("http://localhost:6363").unwrap(),
            "admin",
            &password,
            "admin",
        )
        .await
        .unwrap()
    }

    #[instrument(name = "terminus.client.local_node_with_database", fields(db = %db))]
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
    #[instrument(name = "terminus.client.local_node_test")]
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
    #[instrument(
        name = "terminus.client.new",
        skip(pass),
        fields(
            endpoint = %endpoint,
            user = %user,
            org = %org
        ),
        err
    )]
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
            operation_log: OperationLog::default(),
            query_logger: Arc::new(RwLock::new(None)),
            debug_config: Arc::new(RwLock::new(DebugConfig::default())),
            ensured_databases: Arc::new(Mutex::new(HashSet::new())),
            sse_manager: Arc::new(RwLock::new(None)),
        })
    }

    #[instrument(
        name = "terminus.client.new_with_database",
        skip(pass),
        fields(
            endpoint = %endpoint,
            user = %user,
            org = %org,
            db = %db
        ),
        err
    )]
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

    /// Creates a TerminusDB client from environment variables.
    ///
    /// Reads the following environment variables:
    /// - `TERMINUSDB_HOST`: The TerminusDB server URL (default: "http://localhost:6363")
    /// - `TERMINUSDB_USER`: Username for authentication (default: "admin")
    /// - `TERMINUSDB_PASS`: Password for authentication (default: "root")
    /// - `TERMINUSDB_ORG`: Organization name (default: "admin")
    /// - `TERMINUSDB_DB`: Optional database name to ensure exists
    /// - `TERMINUSDB_BRANCH`: Optional branch name (default: "main")
    ///
    /// # Returns
    /// A configured client instance
    ///
    /// # Example
    /// ```bash
    /// export TERMINUSDB_HOST="https://cloud.terminusdb.com"
    /// export TERMINUSDB_USER="my_user"
    /// export TERMINUSDB_PASS="my_password"
    /// export TERMINUSDB_ORG="my_org"
    /// export TERMINUSDB_DB="my_database"
    /// export TERMINUSDB_BRANCH="develop"
    /// ```
    ///
    /// ```rust
    /// let client = TerminusDBHttpClient::from_env().await?;
    /// ```
    #[instrument(name = "terminus.client.from_env", err)]
    pub async fn from_env() -> anyhow::Result<Self> {
        use std::env;
        
        let endpoint = env::var("TERMINUSDB_HOST")
            .unwrap_or_else(|_| "http://localhost:6363".to_string());
        let user = env::var("TERMINUSDB_USER")
            .unwrap_or_else(|_| "admin".to_string());
        let password = env::var("TERMINUSDB_PASS")
            .unwrap_or_else(|_| "root".to_string());
        let org = env::var("TERMINUSDB_ORG")
            .unwrap_or_else(|_| "admin".to_string());
        
        let endpoint_url = Url::parse(&endpoint)
            .context(format!("Invalid TERMINUSDB_HOST URL: {}", endpoint))?;
        
        debug!("Creating client from environment variables: endpoint={}, user={}, org={}", 
               endpoint, user, org);
        
        let client = Self::new(endpoint_url, &user, &password, &org).await?;
        
        // If TERMINUSDB_DB is set, ensure the database exists
        if let Ok(db) = env::var("TERMINUSDB_DB") {
            debug!("Ensuring database '{}' exists", db);
            let mut client = client.ensure_database(&db).await?;
            
            // If TERMINUSDB_BRANCH is set, check it out (note: this would require adding branch support)
            if let Ok(branch) = env::var("TERMINUSDB_BRANCH") {
                debug!("Branch '{}' specified via TERMINUSDB_BRANCH", branch);
                // TODO: Add branch checkout functionality if needed
                // For now, just log it
            }
            
            return Ok(client);
        }
        
        Ok(client)
    }

    /// Returns a clone of the last executed WOQL query for debugging purposes.
    ///
    /// This method provides access to the most recently executed query, which can be
    /// useful for debugging, logging, or re-executing queries.
    ///
    /// # Returns
    /// `Some(Query)` if a query has been executed, `None` otherwise
    ///
    /// # Example
    /// ```ignore
    /// let client = TerminusDBHttpClient::local_node().await;
    /// 
    /// // Execute a query
    /// let query = Query::select().triple("v:Subject", "rdf:type", "owl:Class");
    /// client.query(Some(spec), query).await?;
    /// 
    /// // Retrieve the last executed query
    /// if let Some(last_query) = client.last_query() {
    ///     println!("Last query: {:?}", last_query);
    /// }
    /// ```
    pub fn last_query(&self) -> Option<terminusdb_woql2::prelude::Query> {
        self.operation_log.get_last_query()
    }

    /// Returns the last executed WOQL query as JSON for debugging purposes.
    ///
    /// This method converts the last executed query to its JSON-LD representation,
    /// which can be useful for debugging, API inspection, or external tools.
    ///
    /// # Returns
    /// `Some(serde_json::Value)` if a query has been executed, `None` otherwise
    ///
    /// # Example
    /// ```ignore
    /// let client = TerminusDBHttpClient::local_node().await;
    /// 
    /// // Execute a query
    /// let query = Query::select().triple("v:Subject", "rdf:type", "owl:Class");
    /// client.query(Some(spec), query).await?;
    /// 
    /// // Retrieve the last executed query as JSON
    /// if let Some(last_query_json) = client.last_query_json() {
    ///     println!("Last query JSON: {}", serde_json::to_string_pretty(&last_query_json).unwrap());
    /// }
    /// ```
    pub fn last_query_json(&self) -> Option<serde_json::Value> {
        use terminusdb_schema::{ToTDBInstance, ToJson};
        self.last_query().map(|query| query.to_instance(None).to_json())
    }


    /// Centralized URL builder for TerminusDB API endpoints.
    /// Handles all URL construction patterns and eliminates duplication.
    pub(crate) fn build_url(&self) -> UrlBuilder {
        UrlBuilder::new(&self.endpoint, &self.org)
    }

    #[instrument(
        name = "terminus.client.info",
        skip(self),
        fields(
            endpoint = %self.endpoint,
            org = %self.org
        ),
        err
    )]
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

    #[instrument(
        name = "terminus.client.is_running",
        skip(self),
        fields(
            endpoint = %self.endpoint
        )
    )]
    pub async fn is_running(&self) -> bool {
        self.info().await.is_ok()
    }

    // ===== Debug/Logging Methods =====

    /// Get the operation log for debugging
    pub fn get_operation_log(&self) -> Vec<crate::debug::OperationEntry> {
        self.operation_log.get_all()
    }

    /// Get the most recent N operations
    pub fn get_recent_operations(&self, n: usize) -> Vec<crate::debug::OperationEntry> {
        self.operation_log.get_recent(n)
    }

    /// Clear the operation log
    pub fn clear_operation_log(&self) {
        self.operation_log.clear()
    }

    /// Set the maximum size of the operation log
    pub async fn set_operation_log_size(&self, size: usize) {
        if let Ok(mut config) = self.debug_config.write() {
            config.operation_log_size = size;
        }
        // Note: We'd need to make operation_log mutable or use interior mutability
        // to actually update the size. For now, new size applies to new logs.
    }

    /// Enable query logging to a file
    pub async fn enable_query_log<P: AsRef<std::path::Path>>(&self, path: P) -> anyhow::Result<()> {
        let logger = QueryLogger::new(path.as_ref()).await?;
        
        if let Ok(mut logger_guard) = self.query_logger.write() {
            *logger_guard = Some(logger);
        }
        
        if let Ok(mut config) = self.debug_config.write() {
            config.query_log_path = Some(path.as_ref().to_string_lossy().into_owned());
            config.enabled = true;
        }
        
        debug!("Query logging enabled");
        Ok(())
    }

    /// Disable query logging
    pub async fn disable_query_log(&self) {
        if let Ok(mut logger_guard) = self.query_logger.write() {
            *logger_guard = None;
        }
        
        if let Ok(mut config) = self.debug_config.write() {
            config.query_log_path = None;
        }
        
        debug!("Query logging disabled");
    }

    /// Check if query logging is enabled
    pub async fn is_query_log_enabled(&self) -> bool {
        if let Ok(logger_guard) = self.query_logger.read() {
            logger_guard.is_some()
        } else {
            false
        }
    }

    /// Rotate the query log file
    pub async fn rotate_query_log(&self) -> anyhow::Result<()> {
        if let Ok(logger_guard) = self.query_logger.read() {
            if let Some(logger) = logger_guard.as_ref() {
                logger.rotate().await?;
            }
        }
        Ok(())
    }

    /// Get slow queries from the query log
    /// 
    /// # Arguments
    /// 
    /// * `threshold` - Duration threshold for slow operations (default: 1 second)
    /// * `filter` - Filter by operation type (default: All)
    /// * `limit` - Maximum number of entries to return (default: unlimited)
    /// 
    /// # Returns
    /// 
    /// A vector of log entries sorted by duration (slowest first), or an error if query logging is not enabled
    pub async fn get_slow_queries(
        &self,
        threshold: Option<Duration>,
        filter: Option<OperationFilter>,
        limit: Option<usize>,
    ) -> anyhow::Result<Vec<QueryLogEntry>> {
        if let Ok(logger_guard) = self.query_logger.read() {
            if let Some(logger) = logger_guard.as_ref() {
                return logger.get_slow_entries(threshold, filter, limit).await;
            }
        }
        
        anyhow::bail!("Query logging is not enabled. Call enable_query_log() first.")
    }

    // ===== Change Listener =====

    /// Create a new ChangeListener for monitoring real-time database changes via SSE
    ///
    /// The ChangeListener connects to TerminusDB's SSE changeset stream and dispatches
    /// typed callbacks when documents are added, updated, or deleted.
    ///
    /// All listeners for the same client share a single SSE connection, which is
    /// automatically managed and routes events based on the resource path.
    ///
    /// # Arguments
    /// * `spec` - Branch specification indicating which database and branch to monitor
    ///
    /// # Returns
    /// A new ChangeListener instance that can be configured with typed callbacks
    ///
    /// # Example
    /// ```rust,ignore
    /// use terminusdb_client::*;
    ///
    /// #[derive(TerminusDBModel, FromTDBInstance, InstanceFromJson)]
    /// struct User {
    ///     name: String,
    ///     email: String,
    /// }
    ///
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let spec = BranchSpec::new("mydb", Some("main"));
    ///
    /// let listener = client.change_listener(spec)?;
    ///
    /// // Register callbacks - listener automatically receives events in background
    /// listener
    ///     .on_added::<User>(|user| {
    ///         println!("User added: {} - {}", user.name, user.email);
    ///     })
    ///     .on_deleted::<User>(|iri| {
    ///         println!("User deleted: {}", iri);
    ///     });
    ///
    /// // Listener is active and will receive events
    /// // Keep listener in scope to continue receiving events
    /// ```
    #[instrument(
        name = "terminus.client.change_listener",
        skip(self),
        fields(
            db = %spec.db,
            branch = ?spec.branch
        )
    )]
    pub fn change_listener(&self, spec: crate::spec::BranchSpec) -> anyhow::Result<super::change_listener::ChangeListener> {
        // Get or create the SSE manager
        let manager = {
            let mut manager_lock = self.sse_manager.write().unwrap();

            if manager_lock.is_none() {
                debug!("Initializing SSE manager for client");
                let manager = Arc::new(super::sse_manager::SseManager::new(
                    self.endpoint.to_string(),
                    self.user.clone(),
                    self.pass.clone(),
                ));
                *manager_lock = Some(manager.clone());
                manager
            } else {
                manager_lock.as_ref().unwrap().clone()
            }
        };

        // Create the listener with the shared SSE manager
        super::change_listener::ChangeListener::new(self.clone(), spec, manager)
    }
}

// Manual Debug implementation
impl std::fmt::Debug for TerminusDBHttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminusDBHttpClient")
            .field("endpoint", &self.endpoint)
            .field("user", &self.user)
            .field("org", &self.org)
            .field("operation_log_size", &self.operation_log.len())
            .field("query_log_enabled", &self.query_logger.read().map(|g| g.is_some()).unwrap_or(false))
            .field("ensured_databases_count", &self.ensured_databases.lock().map(|g| g.len()).unwrap_or(0))
            .finish()
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
            operation_log: OperationLog::default(),
            query_logger: Arc::new(RwLock::new(None)),
            debug_config: Arc::new(RwLock::new(DebugConfig::default())),
            ensured_databases: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    /// Creates a TerminusDB client from environment variables (WASM version).
    ///
    /// Note: In WASM environments, environment variables might not be available
    /// in the same way as native environments. This implementation uses the same
    /// defaults as the native version.
    pub async fn from_env() -> anyhow::Result<Self> {
        // In WASM, we might not have access to std::env
        // You might need to use a different mechanism to get configuration
        // For now, we'll use the same defaults
        let endpoint = Url::parse("http://localhost:6363")?;
        let user = "admin";
        let pass = "root";
        let org = "admin";
        
        Self::new(endpoint, user, pass, org).await
    }

    // Implement other methods as needed for WASM
}
