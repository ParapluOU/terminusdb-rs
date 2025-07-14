//! Database administration operations

use {
    crate::{Database, TerminusDBAdapterError},
    ::tracing::{debug, error, instrument},
    anyhow::Context,
    serde_json::json,
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
