//! Test helpers for ORM integration tests.
//!
//! Provides utilities for setting up and tearing down test databases.
//!
//! # Example
//! ```ignore
//! use terminusdb_orm::testing::with_test_db;
//!
//! #[tokio::test]
//! async fn my_test() -> anyhow::Result<()> {
//!     with_test_db("my_test", |client, spec| async move {
//!         // Your test code here...
//!         // Database is automatically cleaned up when done
//!         Ok(())
//!     }).await
//! }
//! ```

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use terminusdb_client::{BranchSpec, TerminusDBHttpClient};

/// Counter for generating unique test database names
static TEST_DB_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Extension trait for creating test databases from a client.
#[async_trait::async_trait]
pub trait TestDbExt {
    /// Create a test database with a unique name based on the given prefix.
    ///
    /// # Example
    /// ```ignore
    /// use terminusdb_orm::testing::TestDbExt;
    ///
    /// let server = TerminusDBServer::test_instance().await?;
    /// let client = server.client().await?;
    /// let test_db = client.with_test_db("my_test").await?;
    /// ```
    async fn with_test_db(self, prefix: &str) -> anyhow::Result<TestDb>;
}

#[async_trait::async_trait]
impl TestDbExt for TerminusDBHttpClient {
    async fn with_test_db(self, prefix: &str) -> anyhow::Result<TestDb> {
        let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::SeqCst);
        let db_name = format!("{}_{}", prefix, counter);
        TestDb::with_client(self, &db_name).await
    }
}

/// Run a test with a temporary database that is automatically cleaned up.
///
/// This is the most ergonomic way to write integration tests. It:
/// 1. Gets or creates a shared test server instance
/// 2. Creates a uniquely-named database
/// 3. Runs your test closure with the client and spec
/// 4. Cleans up the database when done (even on failure)
///
/// # Example
/// ```ignore
/// use terminusdb_orm::testing::with_test_db;
///
/// #[tokio::test]
/// async fn my_test() -> anyhow::Result<()> {
///     with_test_db("my_test", |client, spec| async move {
///         // Insert schema, data, run queries...
///         client.ensure_database(&spec.db).await?;
///         Ok(())
///     }).await
/// }
/// ```
#[cfg(feature = "testing")]
pub async fn with_test_db<F, Fut, T>(prefix: &str, f: F) -> anyhow::Result<T>
where
    F: FnOnce(TerminusDBHttpClient, BranchSpec) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let server = terminusdb_bin::TerminusDBServer::test_instance().await?;
    server.with_tmp_db(prefix, f).await
}

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
    /// **Note**: This connects to `local_node()` which requires an external server.
    /// For embedded test servers, use `from_server()` instead.
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

    /// Create a new test database from a TerminusDBServer test instance.
    ///
    /// This is the recommended way to create test databases - it uses the
    /// embedded in-memory server, so tests don't require an external server.
    ///
    /// # Example
    /// ```ignore
    /// use terminusdb_bin::TerminusDBServer;
    ///
    /// let server = TerminusDBServer::test_instance().await?;
    /// let test_db = TestDb::from_server(&server, "my_test").await?;
    /// ```
    #[cfg(feature = "testing")]
    pub async fn from_server(
        server: &terminusdb_bin::TerminusDBServer,
        prefix: &str,
    ) -> anyhow::Result<Self> {
        let counter = TEST_DB_COUNTER.fetch_add(1, Ordering::SeqCst);
        let db_name = format!("{}_{}", prefix, counter);
        let client = server.client().await?;
        Self::with_client(client, &db_name).await
    }

    /// Create a test database with a specific name.
    pub async fn with_name(db_name: &str) -> anyhow::Result<Self> {
        let client = TerminusDBHttpClient::local_node().await;
        Self::with_client(client, db_name).await
    }

    /// Create a test database using a provided client.
    ///
    /// This is useful when using an embedded test server:
    /// ```ignore
    /// let server = TerminusDBServer::test_instance().await?;
    /// let client = server.client().await?;
    /// let test_db = TestDb::with_client(client, "my_test").await?;
    /// ```
    pub async fn with_client(client: TerminusDBHttpClient, db_name: &str) -> anyhow::Result<Self> {
        // Use "admin" organization (standard for local/test servers)
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
                    let _ = rt.block_on(async { client.delete_database(&db_name).await });
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

#[cfg(all(test, feature = "testing"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_test_db_creation() {
        let server = terminusdb_bin::TerminusDBServer::test_instance()
            .await
            .unwrap();
        let client = server.client().await.unwrap();
        let test_db = TestDb::with_client(client, "test_helper_0").await.unwrap();
        assert!(test_db.db_name().starts_with("test_helper_"));
        assert_eq!(test_db.org(), "admin");
    }

    #[tokio::test]
    async fn test_spec_creation() {
        let server = terminusdb_bin::TerminusDBServer::test_instance()
            .await
            .unwrap();
        let client = server.client().await.unwrap();
        let test_db = TestDb::with_client(client, "spec_test_0").await.unwrap();
        let spec = test_db.spec();
        assert!(spec.db.starts_with("spec_test_"));
        assert_eq!(spec.branch, Some("main".to_string()));
    }

    #[tokio::test]
    async fn test_with_test_db_extension() {
        let server = terminusdb_bin::TerminusDBServer::test_instance()
            .await
            .unwrap();
        let test_db = server
            .client()
            .await
            .unwrap()
            .with_test_db("ext_test")
            .await
            .unwrap();
        assert!(test_db.db_name().starts_with("ext_test_"));
        assert_eq!(test_db.org(), "admin");
    }

    #[tokio::test]
    async fn test_with_test_db_closure() {
        let result = super::with_test_db("closure_test", |client, spec| async move {
            // Verify we got a working client and spec
            assert!(spec.db.starts_with("closure_test_"));
            assert_eq!(spec.branch, Some("main".to_string()));

            // Verify the database exists
            let dbs = client.list_databases_simple().await?;
            let found = dbs.iter().any(|db| {
                db.path
                    .as_ref()
                    .map(|p| p.contains(&spec.db))
                    .unwrap_or(false)
            });
            assert!(found, "Database should exist");

            Ok(42) // Return a value to prove it works
        })
        .await
        .unwrap();

        assert_eq!(result, 42);
    }
}
