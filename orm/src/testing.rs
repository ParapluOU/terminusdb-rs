//! Test helpers for ORM integration tests.
//!
//! Provides utilities for setting up and tearing down test databases.
//!
//! # Example
//! ```ignore
//! use terminusdb_orm::testing::TestDb;
//!
//! #[tokio::test]
//! async fn my_test() {
//!     let test_db = TestDb::new("my_test_db").await.unwrap();
//!
//!     // Use the client
//!     let client = test_db.client();
//!     let spec = test_db.spec();
//!
//!     // ... run your tests ...
//!
//!     // Cleanup happens automatically when test_db is dropped
//! }
//! ```

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use terminusdb_client::{BranchSpec, TerminusDBHttpClient};

/// Counter for generating unique test database names
static TEST_DB_COUNTER: AtomicU32 = AtomicU32::new(0);

/// A test database context that handles setup and cleanup.
///
/// When dropped, the test database is automatically deleted.
pub struct TestDb {
    client: Arc<TerminusDBHttpClient>,
    db_name: String,
    org: String,
    cleanup_on_drop: bool,
}

impl TestDb {
    /// Create a new test database with a unique name.
    ///
    /// The database name will be `{prefix}_{counter}` where counter is auto-incremented.
    ///
    /// # Example
    /// ```ignore
    /// let test_db = TestDb::new("orm_test").await?;
    /// ```
    pub async fn new(prefix: &str) -> anyhow::Result<Self> {
        let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::SeqCst);
        let db_name = format!("{}_{}", prefix, counter);
        Self::with_name(&db_name).await
    }

    /// Create a test database with a specific name.
    pub async fn with_name(db_name: &str) -> anyhow::Result<Self> {
        let client = TerminusDBHttpClient::local_node().await;
        // local_node() always uses "admin" organization
        let org = "admin".to_string();

        // Ensure the database exists (create if not present)
        let _ = client.ensure_database(db_name).await;

        Ok(Self {
            client: Arc::new(client),
            db_name: db_name.to_string(),
            org,
            cleanup_on_drop: true,
        })
    }

    /// Create a test context using an existing database (no cleanup on drop).
    pub async fn existing(db_name: &str) -> anyhow::Result<Self> {
        let client = TerminusDBHttpClient::local_node().await;
        // local_node() always uses "admin" organization
        let org = "admin".to_string();

        Ok(Self {
            client: Arc::new(client),
            db_name: db_name.to_string(),
            org,
            cleanup_on_drop: false,
        })
    }

    /// Disable automatic cleanup on drop.
    pub fn keep_on_drop(mut self) -> Self {
        self.cleanup_on_drop = false;
        self
    }

    /// Get a reference to the HTTP client.
    pub fn client(&self) -> &TerminusDBHttpClient {
        &self.client
    }

    /// Get a clone of the Arc-wrapped client.
    pub fn client_arc(&self) -> Arc<TerminusDBHttpClient> {
        Arc::clone(&self.client)
    }

    /// Get the database name.
    pub fn db_name(&self) -> &str {
        &self.db_name
    }

    /// Get the organization name.
    pub fn org(&self) -> &str {
        &self.org
    }

    /// Get a BranchSpec for the main branch.
    pub fn spec(&self) -> BranchSpec {
        BranchSpec {
            db: self.db_name.clone(),
            branch: Some("main".to_string()),
            ref_commit: None,
        }
    }

    /// Get a BranchSpec for a specific branch.
    pub fn spec_branch(&self, branch: &str) -> BranchSpec {
        BranchSpec {
            db: self.db_name.clone(),
            branch: Some(branch.to_string()),
            ref_commit: None,
        }
    }

    /// Delete the test database manually.
    pub async fn cleanup(&self) -> anyhow::Result<()> {
        self.client.delete_database(&self.db_name).await.map(|_| ())
    }

    /// Reset the database (delete and recreate).
    pub async fn reset(&self) -> anyhow::Result<()> {
        let _ = self.client.delete_database(&self.db_name).await;
        self.client.ensure_database(&self.db_name).await?;
        Ok(())
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        if self.cleanup_on_drop {
            // Use a blocking runtime to clean up
            // This is acceptable in tests but would be problematic in production
            let client = self.client.clone();
            let db_name = self.db_name.clone();

            // Attempt async cleanup in a new runtime
            // If we're already in a runtime context, this might fail,
            // but that's OK for tests - the DB will be cleaned up next time
            std::thread::spawn(move || {
                if let Ok(rt) = tokio::runtime::Runtime::new() {
                    let _ = rt.block_on(async {
                        client.delete_database(&db_name).await
                    });
                }
            });
        }
    }
}

/// A guard that initializes the global ORM client for tests.
///
/// Ensures the client is only initialized once per test run.
pub struct TestOrmClientGuard {
    _private: (),
}

impl TestOrmClientGuard {
    /// Initialize the global ORM client for testing.
    ///
    /// Returns `Ok(guard)` if this is the first initialization,
    /// or if the client was already initialized.
    pub async fn init() -> anyhow::Result<Self> {
        use crate::OrmClient;

        if !OrmClient::is_initialized() {
            let client = TerminusDBHttpClient::local_node().await;
            // Ignore error if already initialized (race condition)
            let _ = OrmClient::init(client);
        }

        Ok(Self { _private: () })
    }
}

/// Convenience macro for running async tests with a test database.
///
/// # Example
/// ```ignore
/// use terminusdb_orm::testing::test_with_db;
///
/// test_with_db!(test_my_feature, "my_prefix", |test_db| async move {
///     let client = test_db.client();
///     // ... test code ...
///     Ok(())
/// });
/// ```
#[macro_export]
macro_rules! test_with_db {
    ($name:ident, $prefix:expr, $body:expr) => {
        #[tokio::test]
        async fn $name() {
            let test_db = $crate::testing::TestDb::new($prefix)
                .await
                .expect("Failed to create test database");

            let result: anyhow::Result<()> = $body(test_db).await;
            result.expect("Test failed");
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires running TerminusDB instance"]
    async fn test_test_db_creation() {
        let test_db = TestDb::new("test_helper").await.unwrap();
        assert!(test_db.db_name().starts_with("test_helper_"));
        assert_eq!(test_db.org(), "admin");
    }

    #[tokio::test]
    #[ignore = "Requires running TerminusDB instance"]
    async fn test_spec_creation() {
        let test_db = TestDb::new("spec_test").await.unwrap();
        let spec = test_db.spec();
        assert!(spec.db.starts_with("spec_test_"));
        assert_eq!(spec.branch, Some("main".to_string()));
    }
}
