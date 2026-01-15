//! Database opening with automatic schema management and seeding
//!
//! This module provides `open_database`, a method for standardized database initialization
//! that handles schema insertion, state tracking, and optional seeding.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use terminusdb_schema::json::InstanceFromJson;
use terminusdb_schema::{FromTDBInstance, Instance, ToTDBInstance, ToTDBInstances, ToTDBSchemas};
use terminusdb_schema_derive::TerminusDBModel;
use thiserror::Error;
use tracing::{debug, instrument};

use crate::{document::DocumentInsertArgs, spec::BranchSpec, DefaultTDBDeserializer};

/// Large timeout for bulk seeding operations (5 minutes)
const SEED_TIMEOUT: Duration = Duration::from_secs(300);

/// Tracks schema state for migration detection.
///
/// This document is stored as a singleton in each database to record the schema hash
/// at initialization time. When reopening a database, the current schema hash is compared
/// against the recorded hash to detect schema changes that require migration.
#[derive(Debug, Clone, TerminusDBModel)]
#[tdb(key = "lexical", key_fields = "db_name")]
pub struct SchemaState {
    /// Database name this state belongs to
    pub db_name: String,
    /// Hash of the schema tree at initialization time
    pub schema_hash: String,
    /// ISO 8601 timestamp when the schema was initialized
    pub initialized_at: String,
}

/// Result of opening a database via [`TerminusDBHttpClient::open_database`].
#[derive(Debug, Clone)]
pub struct OpenedDatabase {
    /// The client configured for this database
    pub client: super::client::TerminusDBHttpClient,
    /// Branch specification for the opened database
    pub spec: BranchSpec,
    /// Whether the database was newly created during this call
    pub was_created: bool,
    /// Whether seed data was inserted during this call
    pub was_seeded: bool,
}

/// Errors specific to database opening operations.
#[derive(Debug, Error)]
pub enum OpenDatabaseError {
    /// Schema hash mismatch - the database was initialized with a different schema
    #[error("Schema migration required: expected hash {expected}, got {current}")]
    SchemaMigrationRequired {
        /// The schema hash recorded when the database was initialized
        expected: String,
        /// The schema hash computed from the current code
        current: String,
    },

    /// General initialization failure
    #[error("Database initialization failed: {0}")]
    InitializationFailed(#[from] anyhow::Error),
}

/// Compute a stable hash for a schema set.
///
/// The schemas are sorted by class name and deduplicated before hashing
/// to ensure consistent results across runs.
pub fn compute_schema_hash<S: ToTDBSchemas>() -> String {
    let mut schemas = S::to_schemas();
    schemas.sort(); // Schema implements Ord by class_name
    schemas.dedup();

    let mut hasher = DefaultHasher::new();
    for schema in &schemas {
        schema.hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

impl super::client::TerminusDBHttpClient {
    /// Opens a database with automatic schema management and optional seeding.
    ///
    /// This method standardizes database initialization by:
    /// 1. Creating the database if it doesn't exist
    /// 2. Inserting schemas if not initialized
    /// 3. Recording schema hash for migration detection
    /// 4. Running an optional seeder to insert initial data
    ///
    /// # Behavior
    ///
    /// - **Database doesn't exist**: Creates DB, inserts schemas, records hash, runs seeder
    /// - **Database exists but not initialized**: Inserts schemas, records hash, runs seeder
    /// - **Database initialized with same schema**: Returns immediately (no-op)
    /// - **Database initialized with different schema**: Returns `SchemaMigrationRequired` error
    ///
    /// # Type Parameters
    ///
    /// * `S` - Schema types tuple (e.g., `(User, Project, Task)`)
    /// * `F` - Seeder closure type
    ///
    /// # Arguments
    ///
    /// * `db_name` - Name of the database to open
    /// * `seeder` - Optional closure that returns instances to seed
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use terminusdb_client::{TerminusDBHttpClient, ToTDBInstances};
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    ///
    /// // Open with seeder
    /// let opened = client.open_database::<(User, Project), _>(
    ///     "mydb",
    ///     Some(|| vec![
    ///         Box::new(User { name: "Admin".into() }) as Box<dyn ToTDBInstances>,
    ///     ])
    /// ).await?;
    ///
    /// if opened.was_created {
    ///     println!("Database was newly created");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(
        name = "terminus.database.open",
        skip(self, seeder),
        fields(db = %db_name),
        err
    )]
    pub async fn open_database<S, F>(
        &self,
        db_name: &str,
        seeder: Option<F>,
    ) -> Result<OpenedDatabase, OpenDatabaseError>
    where
        S: ToTDBSchemas,
        F: FnOnce() -> Vec<Box<dyn ToTDBInstances>>,
    {
        let spec = BranchSpec::with_branch(db_name, "main");
        let current_hash = compute_schema_hash::<S>();

        debug!(hash = %current_hash, "Computed schema hash");

        // 1. Ensure database exists
        let db_existed = self.database_exists(db_name).await?;
        if !db_existed {
            debug!("Database does not exist, creating");
            self.ensure_database(db_name).await?;
        }

        // 2. Check for existing SchemaState
        let mut deserializer = DefaultTDBDeserializer;
        let existing_state: Option<SchemaState> = self
            .get_instance_if_exists::<SchemaState>(db_name, &spec, &mut deserializer)
            .await
            .ok()
            .flatten();

        match existing_state {
            None => {
                debug!("No SchemaState found, initializing database");
                // Initialize: insert schemas, record state, seed
                let args = DocumentInsertArgs::from(spec.clone());
                // Insert both user schemas and SchemaState schema
                self.insert_schemas::<S>(args.clone().as_schema()).await?;
                self.insert_entity_schema::<SchemaState>(args.clone().as_schema())
                    .await?;
                debug!("Schemas inserted");

                let state = SchemaState {
                    db_name: db_name.to_string(),
                    schema_hash: current_hash,
                    initialized_at: chrono::Utc::now().to_rfc3339(),
                };
                self.insert_instance(&state, args.clone()).await?;
                debug!("SchemaState recorded");

                // Run seeder if provided - client handles bulk insertion
                let was_seeded = if let Some(seed_fn) = seeder {
                    let instance_boxes = seed_fn();
                    if !instance_boxes.is_empty() {
                        debug!(count = instance_boxes.len(), "Seeding instances");
                        let seed_args = args
                            .clone()
                            .with_timeout(SEED_TIMEOUT)
                            .with_skip_existence_check(true);

                        // Collect all instances from the boxed trait objects
                        let mut all_instances = Vec::new();
                        for instance_box in instance_boxes {
                            let instances = instance_box.to_instance_tree_flatten(true);
                            all_instances.extend(instances);
                        }

                        // Set random key prefix and prepare for insertion
                        for instance in &mut all_instances {
                            instance.set_random_key_prefix();
                            instance.capture = true;
                        }
                        all_instances.retain(|i| !i.is_reference());

                        if !all_instances.is_empty() {
                            let models: Vec<_> = all_instances.iter().collect();
                            self.insert_documents(models, seed_args).await?;
                        }
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };

                Ok(OpenedDatabase {
                    client: self.clone(),
                    spec,
                    was_created: !db_existed,
                    was_seeded,
                })
            }
            Some(state) if state.schema_hash == current_hash => {
                debug!("SchemaState matches, database already initialized");
                // Schema matches, just return
                Ok(OpenedDatabase {
                    client: self.clone(),
                    spec,
                    was_created: false,
                    was_seeded: false,
                })
            }
            Some(state) => {
                // Schema mismatch
                debug!(
                    expected = %state.schema_hash,
                    current = %current_hash,
                    "Schema hash mismatch"
                );
                Err(OpenDatabaseError::SchemaMigrationRequired {
                    expected: state.schema_hash,
                    current: current_hash,
                })
            }
        }
    }

    /// Opens a database without a seeder.
    ///
    /// This is a convenience method for `open_database` when no initial data needs to be seeded.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use terminusdb_client::TerminusDBHttpClient;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = TerminusDBHttpClient::local_node().await;
    /// let opened = client.open_database_no_seed::<(User, Project)>("mydb").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn open_database_no_seed<S: ToTDBSchemas>(
        &self,
        db_name: &str,
    ) -> Result<OpenedDatabase, OpenDatabaseError> {
        self.open_database::<S, fn() -> Vec<Box<dyn ToTDBInstances>>>(db_name, None)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_schema_hash_is_deterministic() {
        // Same schema should produce same hash
        let hash1 = compute_schema_hash::<(SchemaState,)>();
        let hash2 = compute_schema_hash::<(SchemaState,)>();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_compute_schema_hash_format() {
        let hash = compute_schema_hash::<(SchemaState,)>();
        // Should be 16 hex characters
        assert_eq!(hash.len(), 16);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
