//! High-level async server management for TerminusDB.
//!
//! Provides a clean API for starting/stopping TerminusDB servers without
//! exposing low-level process management details.
//!
//! # Example
//!
//! ```no_run
//! use terminusdb_bin::TerminusDBServer;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Quick test server
//!     let server = TerminusDBServer::test().await?;
//!     let client = server.client().await?;
//!
//!     // Or use a shared instance across tests
//!     let server = TerminusDBServer::test_instance().await?;
//!     let client = server.client().await?;
//!
//!     Ok(())
//! }
//! ```

use std::process::{Child, Stdio};
use std::time::Duration;
use terminusdb_client::TerminusDBHttpClient;
use tokio::sync::OnceCell;

/// Options for starting a TerminusDB server.
#[derive(Debug, Clone, Default)]
pub struct ServerOptions {
    /// Run server in-memory (no persistence). Good for testing.
    pub memory: bool,
    /// Admin password (defaults to "root" if not set).
    pub password: Option<String>,
    /// Suppress stdout/stderr output.
    pub quiet: bool,
}

/// A running TerminusDB server instance.
///
/// The server is automatically stopped when this handle is dropped,
/// unless it's a shared instance from `test_instance()`.
pub struct TerminusDBServer {
    child: Option<Child>,
    shared: bool,
}

/// Shared test server instance
static TEST_INSTANCE: OnceCell<TerminusDBServer> = OnceCell::const_new();

impl TerminusDBServer {
    /// Start a new test server with default test settings.
    ///
    /// Equivalent to `start_server(ServerOptions { memory: true, quiet: true, .. })`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use terminusdb_bin::TerminusDBServer;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let server = TerminusDBServer::test().await?;
    ///     let client = server.client().await?;
    ///     // Server stops when dropped
    ///     Ok(())
    /// }
    /// ```
    pub async fn test() -> anyhow::Result<Self> {
        start_server(ServerOptions {
            memory: true,
            quiet: true,
            ..Default::default()
        })
        .await
    }

    /// Get or create a shared test server instance.
    ///
    /// The server is started on the first call and kept running for the
    /// lifetime of the process. Subsequent calls return the same instance.
    /// This is useful for running multiple tests against the same server.
    ///
    /// Note: The shared server is NOT stopped when the reference is dropped.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use terminusdb_bin::TerminusDBServer;
    ///
    /// #[tokio::test]
    /// async fn test_one() -> anyhow::Result<()> {
    ///     let server = TerminusDBServer::test_instance().await?;
    ///     let client = server.client().await?;
    ///     // Use client...
    ///     Ok(())
    /// }
    ///
    /// #[tokio::test]
    /// async fn test_two() -> anyhow::Result<()> {
    ///     // Same server instance as test_one
    ///     let server = TerminusDBServer::test_instance().await?;
    ///     let client = server.client().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn test_instance() -> anyhow::Result<&'static Self> {
        TEST_INSTANCE
            .get_or_try_init(|| async {
                let binary_path = crate::extract_binary()?;
                eprintln!("[terminusdb-bin] test_instance: Binary path: {:?}", binary_path);

                // Create a clean temp directory for the DB path
                let db_path = std::env::temp_dir()
                    .join(format!("terminusdb-test-{}", std::process::id()));
                std::fs::create_dir_all(&db_path)?;
                eprintln!("[terminusdb-bin] test_instance: DB path: {:?}", db_path);

                // Initialize the store first (required before starting server in new directory)
                eprintln!("[terminusdb-bin] test_instance: Initializing store...");
                let init_status = std::process::Command::new(&binary_path)
                    .args(["store", "init"])
                    .current_dir(&db_path)
                    .env("TERMINUSDB_SERVER_DB_PATH", &db_path)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()?;
                if !init_status.success() {
                    anyhow::bail!("Failed to initialize store: {:?}", init_status);
                }

                let mut args = vec!["serve".to_string()];
                args.push("--memory".to_string());
                args.push("root".to_string());

                eprintln!("[terminusdb-bin] test_instance: Spawning with args: {:?}", args);

                let mut child = std::process::Command::new(&binary_path)
                    .args(&args)
                    .current_dir(&db_path)
                    .env("TERMINUSDB_SERVER_DB_PATH", &db_path)
                    .stdout(Stdio::null())  // Suppress output for test instance
                    .stderr(Stdio::null())
                    .spawn()?;

                eprintln!("[terminusdb-bin] test_instance: Server spawned with PID: {}", child.id());

                let server = TerminusDBServer {
                    child: Some(child),
                    shared: true,
                };

                // Wait for server to be ready
                wait_for_ready(Duration::from_secs(30)).await?;

                Ok(server)
            })
            .await
    }

    /// Get a configured HTTP client for this server.
    ///
    /// Uses `TerminusDBHttpClient::local_node()` which handles env vars automatically.
    /// Verifies the server is responding before returning.
    pub async fn client(&self) -> anyhow::Result<TerminusDBHttpClient> {
        let client = TerminusDBHttpClient::local_node().await;
        // Verify server is responding
        client.info().await?;
        Ok(client)
    }

    /// Run a test with a temporary database that is automatically cleaned up.
    ///
    /// Creates a uniquely-named database, passes the client and BranchSpec to the closure,
    /// and deletes the database when done (even if the test fails/panics).
    ///
    /// This enables safe parallel test execution since each test gets its own database.
    ///
    /// # Arguments
    ///
    /// * `prefix` - A prefix for the database name (e.g., "test_something")
    /// * `f` - An async closure that receives the client and branch spec
    ///
    /// # Example
    ///
    /// ```no_run
    /// use terminusdb_bin::TerminusDBServer;
    ///
    /// #[tokio::test]
    /// async fn test_something() -> anyhow::Result<()> {
    ///     let server = TerminusDBServer::test_instance().await?;
    ///
    ///     server.with_tmp_db("test_something", |client, spec| async move {
    ///         // Your test code here
    ///         // Database is automatically cleaned up when this closure returns!
    ///         Ok(())
    ///     }).await
    /// }
    /// ```
    pub async fn with_tmp_db<F, Fut, T>(&self, prefix: &str, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(TerminusDBHttpClient, terminusdb_client::BranchSpec) -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<T>>,
    {
        use uuid::Uuid;

        let db_name = format!("{}_{}", prefix, Uuid::new_v4().simple());
        let client = self.client().await?;
        client.ensure_database(&db_name).await?;

        let spec = terminusdb_client::BranchSpec::with_branch(&db_name, "main");

        // Run the test closure
        let result = f(client.clone(), spec).await;

        // Always cleanup, regardless of test success/failure
        let _ = client.delete_database(&db_name).await;

        result
    }
}

impl Drop for TerminusDBServer {
    fn drop(&mut self) {
        // Don't kill shared instances
        if !self.shared {
            if let Some(ref mut child) = self.child {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

/// Start a TerminusDB server with custom options.
///
/// For testing, prefer `TerminusDBServer::test()` or `TerminusDBServer::test_instance()`.
///
/// # Arguments
///
/// * `opts` - Server configuration options
///
/// # Returns
///
/// A `TerminusDBServer` handle that stops the server when dropped.
///
/// # Example
///
/// ```no_run
/// use terminusdb_bin::{start_server, ServerOptions};
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let server = start_server(ServerOptions {
///         memory: true,
///         password: Some("secret".into()),
///         quiet: false,
///     }).await?;
///
///     let client = server.client().await?;
///     drop(server); // Server stops
///     Ok(())
/// }
/// ```
pub async fn start_server(opts: ServerOptions) -> anyhow::Result<TerminusDBServer> {
    let binary_path = crate::extract_binary()?;
    eprintln!("[terminusdb-bin] Binary path: {:?}", binary_path);

    // Create a clean temp directory for the DB path
    let db_path = std::env::temp_dir()
        .join(format!("terminusdb-server-{}", std::process::id()));
    std::fs::create_dir_all(&db_path)?;

    // Initialize the store first (required before starting server in new directory)
    eprintln!("[terminusdb-bin] Initializing store in {:?}...", db_path);
    let init_status = std::process::Command::new(&binary_path)
        .args(["store", "init"])
        .current_dir(&db_path)
        .env("TERMINUSDB_SERVER_DB_PATH", &db_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !init_status.success() {
        anyhow::bail!("Failed to initialize store: {:?}", init_status);
    }

    let mut args = vec!["serve".to_string()];
    if opts.memory {
        let password = opts.password.as_deref().unwrap_or("root");
        args.push("--memory".to_string());
        args.push(password.to_string());
    }

    let stdout = if opts.quiet {
        Stdio::null()
    } else {
        Stdio::inherit()
    };
    let stderr = if opts.quiet {
        Stdio::null()
    } else {
        Stdio::inherit()
    };

    eprintln!("[terminusdb-bin] Spawning server with args: {:?}", args);

    let child = std::process::Command::new(&binary_path)
        .args(&args)
        .current_dir(&db_path)
        .env("TERMINUSDB_SERVER_DB_PATH", &db_path)
        .stdout(stdout)
        .stderr(stderr)
        .spawn()?;

    eprintln!("[terminusdb-bin] Server spawned with PID: {}", child.id());

    let server = TerminusDBServer {
        child: Some(child),
        shared: false,
    };

    // Wait for server to be ready using the client itself
    wait_for_ready(Duration::from_secs(30)).await?;

    Ok(server)
}

/// Wait for the server to respond using TerminusDBHttpClient.
async fn wait_for_ready(max_wait: Duration) -> anyhow::Result<()> {
    let start = std::time::Instant::now();

    eprintln!("[terminusdb-bin] Waiting for server to become ready...");

    while start.elapsed() < max_wait {
        // Use the client's local_node() + info() for health check
        let client = TerminusDBHttpClient::local_node().await;
        match client.info().await {
            Ok(_) => {
                eprintln!("[terminusdb-bin] Server is ready!");
                return Ok(());
            }
            Err(e) => {
                if start.elapsed().as_secs() % 5 == 0 {
                    eprintln!("[terminusdb-bin] Still waiting... ({}s): {:?}",
                        start.elapsed().as_secs(), e);
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    anyhow::bail!("Server did not become ready within {:?}", max_wait)
}

/// Execute a closure with a running TerminusDB server.
///
/// The server is automatically started before the closure runs and
/// stopped after the closure completes (regardless of success/failure).
///
/// # Example
///
/// ```no_run
/// use terminusdb_bin::{with_server, ServerOptions};
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     with_server(ServerOptions::default(), |client| async move {
///         let info = client.info().await?;
///         println!("Connected to TerminusDB");
///         Ok(())
///     }).await
/// }
/// ```
pub async fn with_server<F, Fut, T>(opts: ServerOptions, f: F) -> anyhow::Result<T>
where
    F: FnOnce(TerminusDBHttpClient) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let server = start_server(opts).await?;
    let client = server.client().await?;
    f(client).await
    // Server dropped here, stopping the process
}
