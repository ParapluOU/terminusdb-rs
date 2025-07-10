//! Database administration operations

use {
    crate::TerminusDBAdapterError,
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
}
