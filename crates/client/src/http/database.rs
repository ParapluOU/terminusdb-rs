//! Database administration operations

use {
    crate::{
        debug::{OperationEntry, OperationType, QueryLogEntry},
        Database, TerminusDBAdapterError,
    },
    ::tracing::{debug, error, instrument},
    anyhow::Context,
    serde_json::json,
    std::time::Instant,
};

/// Database administration methods for the TerminusDB HTTP client
impl super::client::TerminusDBHttpClient {
    /// Check if a database exists by name.
    ///
    /// This queries the list of databases and checks if any database
    /// has a matching name (extracted from the path).
    ///
    /// # Arguments
    /// * `db_name` - The name of the database to check
    ///
    /// # Returns
    /// `true` if the database exists, `false` otherwise
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::TerminusDBHttpClient;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// if client.database_exists("mydb").await? {
    ///     println!("Database exists");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn database_exists(&self, db_name: &str) -> anyhow::Result<bool> {
        let databases = self.list_databases_simple().await?;
        Ok(databases
            .iter()
            .any(|db| db.database_name().as_deref() == Some(db_name)))
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
    #[instrument(
        name = "terminus.database.ensure",
        skip(self),
        fields(
            db = %db,
            org = %self.org
        ),
        err
    )]
    pub async fn ensure_database(&self, db: &str) -> anyhow::Result<Self> {
        // Check cache first
        {
            let cache = self
                .ensured_databases
                .lock()
                .map_err(|_| anyhow::anyhow!("Failed to lock database cache"))?;
            if cache.contains(db) {
                debug!("Database {} already ensured (cached)", db);
                return Ok(self.clone());
            }
        }

        let start_time = Instant::now();
        let uri = self.build_url().endpoint("db").simple_database(db).build();

        debug!("post uri: {}", &uri);

        // Create operation entry
        let mut operation =
            OperationEntry::new(OperationType::CreateDatabase, format!("/api/db/{}", db))
                .with_context(Some(db.to_string()), None);

        // Acquire concurrency permit for write operations
        let _permit = self.acquire_write_permit().await;

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

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        // todo: use parse_response()
        if ![200, 400].contains(&status) {
            error!("could not ensure database");

            let error_text = res.text().await?;
            let error_msg = format!("request failed: {:#?}", error_text);

            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation.clone());

            // Log to query log if enabled
            let logger_opt = self
                .query_logger
                .read()
                .ok()
                .and_then(|guard| guard.clone());
            if let Some(logger) = logger_opt {
                let log_entry = QueryLogEntry {
                    timestamp: chrono::Utc::now(),
                    operation_type: "create_database".to_string(),
                    database: Some(db.to_string()),
                    branch: None,
                    endpoint: format!("/api/db/{}", db),
                    details: json!({
                        "comment": "Song database specific for this node",
                        "label": db,
                        "public": true,
                        "schema": true
                    }),
                    success: false,
                    result_count: None,
                    duration_ms,
                    error: Some(error_msg.clone()),
                };
                let _ = logger.log(log_entry).await;
            }

            Err(TerminusDBAdapterError::Other(error_msg))?;
        }

        // Success (200) or already exists (400)
        let context = if status == 400 {
            "already exists"
        } else {
            "created"
        };
        operation = operation
            .success(None, duration_ms)
            .with_additional_context(context.to_string());
        self.operation_log.push(operation);

        // Log to query log if enabled
        let logger_opt = self
            .query_logger
            .read()
            .ok()
            .and_then(|guard| guard.clone());
        if let Some(logger) = logger_opt {
            let log_entry = QueryLogEntry {
                timestamp: chrono::Utc::now(),
                operation_type: "create_database".to_string(),
                database: Some(db.to_string()),
                branch: None,
                endpoint: format!("/api/db/{}", db),
                details: json!({
                    "comment": "Song database specific for this node",
                    "label": db,
                    "public": true,
                    "schema": true,
                    "status": status,
                    "context": context
                }),
                success: true,
                result_count: None,
                duration_ms,
                error: None,
            };
            let _ = logger.log(log_entry).await;
        }

        // Add to cache on success
        {
            let mut cache = self
                .ensured_databases
                .lock()
                .map_err(|_| anyhow::anyhow!("Failed to lock database cache"))?;
            cache.insert(db.to_string());
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
    #[instrument(
        name = "terminus.database.delete",
        skip(self),
        fields(
            db = %db,
            org = %self.org
        ),
        err
    )]
    #[pseudonym::alias(drop_database)]
    pub async fn delete_database(&self, db: &str) -> anyhow::Result<Self> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("db").simple_database(db).build();

        debug!("deleting database {}", db);

        // Create operation entry
        let mut operation =
            OperationEntry::new(OperationType::DeleteDatabase, format!("/api/db/{}", db))
                .with_context(Some(db.to_string()), None);

        // Acquire concurrency permit for write operations
        let _permit = self.acquire_write_permit().await;

        let result = self
            .http
            .delete(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to delete database");

        let duration_ms = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(_) => {
                operation = operation.success(None, duration_ms);
                self.operation_log.push(operation);

                // Remove from cache on successful deletion
                {
                    let mut cache = self
                        .ensured_databases
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Failed to lock database cache"))?;
                    cache.remove(db);
                }

                // Log to query log if enabled
                let logger_opt = self
                    .query_logger
                    .read()
                    .ok()
                    .and_then(|guard| guard.clone());
                if let Some(logger) = logger_opt {
                    let log_entry = QueryLogEntry {
                        timestamp: chrono::Utc::now(),
                        operation_type: "delete_database".to_string(),
                        database: Some(db.to_string()),
                        branch: None,
                        endpoint: format!("/api/db/{}", db),
                        details: json!({}),
                        success: true,
                        result_count: None,
                        duration_ms,
                        error: None,
                    };
                    let _ = logger.log(log_entry).await;
                }

                Ok(self.clone())
            }
            Err(e) => {
                operation = operation.failure(e.to_string(), duration_ms);
                self.operation_log.push(operation);

                // Log to query log if enabled
                let logger_opt = self
                    .query_logger
                    .read()
                    .ok()
                    .and_then(|guard| guard.clone());
                if let Some(logger) = logger_opt {
                    let log_entry = QueryLogEntry {
                        timestamp: chrono::Utc::now(),
                        operation_type: "delete_database".to_string(),
                        database: Some(db.to_string()),
                        branch: None,
                        endpoint: format!("/api/db/{}", db),
                        details: json!({}),
                        success: false,
                        result_count: None,
                        duration_ms,
                        error: Some(e.to_string()),
                    };
                    let _ = logger.log(log_entry).await;
                }

                Err(e)
            }
        }
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
    #[instrument(
        name = "terminus.database.reset",
        skip(self),
        fields(
            db = %db,
            org = %self.org
        ),
        err
    )]

    pub async fn reset_database(&self, db: &str) -> anyhow::Result<Self> {
        debug!("resetting database {}", db);

        self.delete_database(db)
            .await
            .context("failed to delete database during reset")?;

        self.ensure_database(db)
            .await
            .context("failed to recreate database during reset")
    }

    /// Lists all databases available to the authenticated user.
    ///
    /// This function retrieves a list of all databases that the current user has access to.
    /// The list includes database metadata such as name, type, creation date, and state.
    ///
    /// # Arguments
    /// * `branches` - Whether to include branch information (default: false)
    /// * `verbose` - Whether to return all available information (default: false)
    ///
    /// # Returns
    /// A vector of `Database` objects containing information about each available database
    ///
    /// # Example
    /// ```rust
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let databases = client.list_databases(false, false).await?;
    /// for db in databases {
    ///     println!("Database: {} ({})", db.name, db.id);
    /// }
    /// ```
    #[instrument(
        name = "terminus.database.list",
        skip(self),
        fields(
            org = %self.org,
            branches = %branches,
            verbose = %verbose
        ),
        err
    )]
    pub async fn list_databases(
        &self,
        branches: bool,
        verbose: bool,
    ) -> anyhow::Result<Vec<Database>> {
        let uri = self
            .build_url()
            .endpoint("db")
            .query("branches", &branches.to_string())
            .query("verbose", &verbose.to_string())
            .build();

        debug!("Listing databases with URI: {}", &uri);

        // Acquire concurrency permit for read operations
        let _permit = self.acquire_read_permit().await;

        let res = self
            .http
            .get(uri.clone())
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context(format!("Failed to list databases from {}", &uri))?;

        debug!("Received response from TerminusDB, parsing database list...");

        // The /db endpoint returns a direct array, not wrapped in ApiResponse
        let databases: Vec<Database> = res
            .json()
            .await
            .context("Failed to parse database list response")?;

        Ok(databases)
    }

    /// Lists all databases with default options (no branches, not verbose).
    ///
    /// This is a convenience method that calls `list_databases(false, false)`.
    ///
    /// # Returns
    /// A vector of `Database` objects
    ///
    /// # Example
    /// ```rust
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let databases = client.list_databases_simple().await?;
    /// ```
    #[instrument(
        name = "terminus.database.list_simple",
        skip(self),
        fields(
            org = %self.org
        ),
        err
    )]
    pub async fn list_databases_simple(&self) -> anyhow::Result<Vec<Database>> {
        self.list_databases(false, false).await
    }

    /// Clears the ensured databases cache.
    ///
    /// This forces all subsequent `ensure_database()` calls to check with the server
    /// rather than relying on the cache. Useful when external changes might have
    /// occurred to the database.
    ///
    /// # Example
    /// ```rust
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.clear_database_cache()?;
    /// ```
    pub fn clear_database_cache(&self) -> anyhow::Result<()> {
        let mut cache = self
            .ensured_databases
            .lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock database cache"))?;
        cache.clear();
        debug!("Cleared ensured databases cache");
        Ok(())
    }

    /// Returns the list of databases currently in the cache.
    ///
    /// This shows which databases have been ensured and won't require server
    /// verification on the next `ensure_database()` call.
    ///
    /// # Example
    /// ```rust
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let cached = client.get_cached_databases()?;
    /// println!("Cached databases: {:?}", cached);
    /// ```
    pub fn get_cached_databases(&self) -> anyhow::Result<Vec<String>> {
        let cache = self
            .ensured_databases
            .lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock database cache"))?;
        Ok(cache.iter().cloned().collect())
    }

    /// Updates database metadata (label and comment).
    ///
    /// # Arguments
    /// * `db` - Database name
    /// * `label` - New label for the database
    /// * `comment` - New comment for the database
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.update_database(
    ///     "mydb",
    ///     Some("My Updated Database"),
    ///     Some("Updated description")
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.database.update",
        skip(self),
        fields(
            db = %db,
            label = ?label,
            comment = ?comment
        ),
        err
    )]
    pub async fn update_database(
        &self,
        db: &str,
        label: Option<&str>,
        comment: Option<&str>,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("db").simple_database(db).build();

        debug!("PUT {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("update_database".to_string()),
            format!("/api/db/{}", db),
        )
        .with_context(Some(db.to_string()), None);

        let mut body = json!({});
        if let Some(l) = label {
            body["label"] = json!(l);
        }
        if let Some(c) = comment {
            body["comment"] = json!(c);
        }

        // Acquire concurrency permit for write operations
        let _permit = self.acquire_write_permit().await;

        let res = self
            .http
            .put(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("failed to update database")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("update database operation failed with status {}", status);

            let error_text = res.text().await?;
            let error_msg = format!("update database failed: {:#?}", error_text);

            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);

            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;

        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!(
            "Successfully updated database in {:?}",
            start_time.elapsed()
        );

        Ok(response)
    }

    /// Optimizes a database by removing unreachable data.
    ///
    /// # Arguments
    /// * `path` - Path to optimize (e.g., "admin/mydb/_meta" or "admin/mydb/local/branch/main")
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// client.optimize("admin/mydb/_meta").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.database.optimize",
        skip(self),
        fields(
            path = %path
        ),
        err
    )]
    pub async fn optimize(
        &self,
        path: &str,
        timeout: Option<std::time::Duration>,
    ) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("optimize").add_path(path).build();

        debug!("POST {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("optimize".to_string()),
            format!("/api/optimize/{}", path),
        )
        .with_context(None, None);

        // Acquire concurrency permit for write operations
        let _permit = self.acquire_write_permit().await;

        let mut request = self.http.post(uri).basic_auth(&self.user, Some(&self.pass));

        // Apply timeout if provided
        if let Some(timeout) = timeout {
            request = request.timeout(timeout);
        }

        let res = request
            .send()
            .await
            .context("failed to optimize database")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("optimize operation failed with status {}", status);

            let error_text = res.text().await?;
            let error_msg = format!("optimize failed: {:#?}", error_text);

            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);

            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;

        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!(
            "Successfully optimized database in {:?}",
            start_time.elapsed()
        );

        Ok(response)
    }

    /// Gets the list of prefixes for a database.
    ///
    /// # Arguments
    /// * `path` - Path to get prefixes for (e.g., "admin/mydb/local/branch/main")
    ///
    /// # Example
    /// ```rust,no_run
    /// # use terminusdb_client::*;
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let prefixes = client.get_prefixes("admin/mydb/local/branch/main").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.database.prefixes",
        skip(self),
        fields(
            path = %path
        ),
        err
    )]
    pub async fn get_prefixes(&self, path: &str) -> anyhow::Result<serde_json::Value> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("prefixes").add_path(path).build();

        debug!("GET {}", &uri);

        let mut operation = OperationEntry::new(
            OperationType::Other("get_prefixes".to_string()),
            format!("/api/prefixes/{}", path),
        )
        .with_context(None, None);

        // Acquire concurrency permit for read operations
        let _permit = self.acquire_read_permit().await;

        let res = self
            .http
            .get(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context("failed to get prefixes")?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let status = res.status().as_u16();

        if !res.status().is_success() {
            error!("get prefixes operation failed with status {}", status);

            let error_text = res.text().await?;
            let error_msg = format!("get prefixes failed: {:#?}", error_text);

            operation = operation.failure(error_msg.clone(), duration_ms);
            self.operation_log.push(operation);

            return Err(anyhow::anyhow!(error_msg));
        }

        let response = self.parse_response::<serde_json::Value>(res).await?;

        operation = operation.success(None, duration_ms);
        self.operation_log.push(operation);

        debug!(
            "Successfully retrieved prefixes in {:?}",
            start_time.elapsed()
        );

        Ok(response)
    }
}
