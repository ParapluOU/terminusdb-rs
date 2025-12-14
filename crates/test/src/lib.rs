//! Testing utilities for TerminusDB.
//!
//! This crate provides ergonomic test helpers for writing integration tests
//! against TerminusDB using an embedded in-memory server.
//!
//! # Quick Start
//!
//! ```ignore
//! use terminusdb_test::test;
//!
//! #[test(db = "my_feature")]
//! async fn test_my_feature(client: _, spec: _) -> anyhow::Result<()> {
//!     // client is a TerminusDBHttpClient connected to the test server
//!     // spec is a BranchSpec for a fresh database named "my_feature_0"
//!
//!     client.ensure_database(&spec.db).await?;
//!     // ... your test code ...
//!     Ok(())
//! }
//! ```
//!
//! # How It Works
//!
//! The `#[test]` macro:
//! 1. Spawns (or reuses) an in-memory TerminusDB server
//! 2. Creates a uniquely-named database with your prefix
//! 3. Provides a client and branch spec to your test
//! 4. Automatically cleans up the database after the test
//!
//! # Manual Usage
//!
//! If you prefer not to use the macro, you can use `with_test_db` directly:
//!
//! ```ignore
//! use terminusdb_test::with_test_db;
//!
//! #[tokio::test]
//! async fn test_manual() -> anyhow::Result<()> {
//!     with_test_db("my_test", |client, spec| async move {
//!         // test code
//!         Ok(())
//!     }).await
//! }
//! ```

pub use terminusdb_client::{BranchSpec, TerminusDBHttpClient};

/// Re-export the test macro.
///
/// Use as `#[terminusdb_test::test(db = "prefix")]`
pub use terminusdb_test_macros::test;

/// Run a test with a temporary database that is automatically cleaned up.
///
/// This function:
/// 1. Gets or creates a shared test server instance
/// 2. Creates a uniquely-named database with the given prefix
/// 3. Runs your test closure with the client and spec
/// 4. Cleans up the database when done (even on failure)
///
/// # Example
///
/// ```ignore
/// use terminusdb_test::with_test_db;
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
pub async fn with_test_db<F, Fut, T>(prefix: &str, f: F) -> anyhow::Result<T>
where
    F: FnOnce(TerminusDBHttpClient, BranchSpec) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let server = terminusdb_bin::TerminusDBServer::test_instance().await?;
    server.with_tmp_db(prefix, f).await
}
