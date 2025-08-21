//! Database administration operations

use {
    crate::{Database, TerminusDBAdapterError, debug::{OperationEntry, OperationType, QueryLogEntry}},
    ::tracing::{debug, error, instrument},
    anyhow::Context,
    serde_json::json,
    std::time::Instant,
};

/// Database administration methods for the TerminusDB HTTP client
impl super::client::TerminusDBHttpClient {
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
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("db").simple_database(db).build();

        debug!("post uri: {}", &uri);

        // Create operation entry
        let mut operation = OperationEntry::new(
            OperationType::CreateDatabase,
            format!("/api/db/{}", db)
        ).with_context(Some(db.to_string()), None);

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
            let logger_opt = self.query_logger.read().ok().and_then(|guard| guard.clone());
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
        let context = if status == 400 { "already exists" } else { "created" };
        operation = operation.success(None, duration_ms)
            .with_additional_context(context.to_string());
        self.operation_log.push(operation);
        
        // Log to query log if enabled
        let logger_opt = self.query_logger.read().ok().and_then(|guard| guard.clone());
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
    pub async fn delete_database(&self, db: &str) -> anyhow::Result<Self> {
        let start_time = Instant::now();
        let uri = self.build_url().endpoint("db").simple_database(db).build();

        debug!("deleting database {}", db);

        // Create operation entry
        let mut operation = OperationEntry::new(
            OperationType::DeleteDatabase,
            format!("/api/db/{}", db)
        ).with_context(Some(db.to_string()), None);

        let result = self.http
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
                
                // Log to query log if enabled
                let logger_opt = self.query_logger.read().ok().and_then(|guard| guard.clone());
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
                let logger_opt = self.query_logger.read().ok().and_then(|guard| guard.clone());
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
    pub async fn list_databases(&self, branches: bool, verbose: bool) -> anyhow::Result<Vec<Database>> {
        let uri = self
            .build_url()
            .endpoint("db")
            .query("branches", &branches.to_string())
            .query("verbose", &verbose.to_string())
            .build();
        
        debug!("Listing databases with URI: {}", &uri);
        
        let res = self
            .http
            .get(uri.clone())
            .basic_auth(&self.user, Some(&self.pass))
            .send()
            .await
            .context(format!("Failed to list databases from {}", &uri))?;
        
        debug!("Received response from TerminusDB, parsing database list...");
        
        // The /db endpoint returns a direct array, not wrapped in ApiResponse
        let databases: Vec<Database> = res.json().await
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
}
