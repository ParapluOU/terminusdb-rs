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
    /// Custom database path. If not set, a temp directory is created.
    pub db_path: Option<std::path::PathBuf>,
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

                // --memory mode self-initializes, no store init needed
                let args = vec!["serve", "--memory", "root"];

                eprintln!("[terminusdb-bin] test_instance: Spawning with args: {:?}", args);

                // Pipe stderr so we can capture early failure messages
                let mut child = std::process::Command::new(&binary_path)
                    .args(&args)
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped())
                    .spawn()?;

                eprintln!(
                    "[terminusdb-bin] test_instance: Server spawned with PID: {}",
                    child.id()
                );

                // Wait for server to be ready, checking for early process exit
                wait_for_ready(&mut child, Duration::from_secs(30)).await?;

                let server = TerminusDBServer {
                    child: Some(child),
                    shared: true,
                };

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

    /// Run a test with a temporary database with pre-inserted schemas.
    ///
    /// Creates a uniquely-named database, inserts schemas for all types in T,
    /// passes the client and BranchSpec to the closure, and deletes the database
    /// when done (even if the test fails/panics).
    ///
    /// # Type Parameters
    ///
    /// * `T` - A tuple of types implementing `ToTDBSchema` (e.g., `(Model1, Model2)`)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use terminusdb_bin::TerminusDBServer;
    /// use terminusdb_client::DocumentInsertArgs;
    ///
    /// #[tokio::test]
    /// async fn test_with_schema() -> anyhow::Result<()> {
    ///     let server = TerminusDBServer::test_instance().await?;
    ///
    ///     server.with_db_schema::<(Person, Company)>("test", |client, spec| async move {
    ///         // Schemas for Person and Company already inserted!
    ///         let args = DocumentInsertArgs::from(spec.clone());
    ///         client.insert_instance(&person, args).await?;
    ///         Ok(())
    ///     }).await
    /// }
    /// ```
    pub async fn with_db_schema<T, F, Fut, R>(&self, prefix: &str, f: F) -> anyhow::Result<R>
    where
        T: terminusdb_schema::ToTDBSchemas,
        F: FnOnce(TerminusDBHttpClient, terminusdb_client::BranchSpec) -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<R>>,
    {
        self.with_tmp_db(prefix, |client, spec| async move {
            let args = terminusdb_client::DocumentInsertArgs::from(spec.clone());
            client.insert_schemas::<T>(args).await?;
            f(client, spec).await
        })
        .await
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
///         ..Default::default()
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

    // Use custom db_path or create a temp directory
    let db_path = match opts.db_path {
        Some(ref path) => {
            std::fs::create_dir_all(path)?;
            path.clone()
        }
        None => {
            let path =
                std::env::temp_dir().join(format!("terminusdb-server-{}", std::process::id()));
            std::fs::create_dir_all(&path)?;
            path
        }
    };

    // Only initialize store for persistent mode; --memory self-initializes
    if !opts.memory {
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

    eprintln!("[terminusdb-bin] Spawning server with args: {:?}", args);

    // Always pipe stderr so we can capture early failure messages
    let mut child = std::process::Command::new(&binary_path)
        .args(&args)
        .current_dir(&db_path)
        .env("TERMINUSDB_SERVER_DB_PATH", &db_path)
        .stdout(stdout)
        .stderr(Stdio::piped())
        .spawn()?;

    eprintln!("[terminusdb-bin] Server spawned with PID: {}", child.id());

    // Wait for server to be ready, checking for early process exit
    wait_for_ready(&mut child, Duration::from_secs(30)).await?;

    let server = TerminusDBServer {
        child: Some(child),
        shared: false,
    };

    Ok(server)
}

/// Known error patterns that indicate the server failed to start properly.
/// These errors may appear in stderr even if the process doesn't exit.
const FATAL_ERROR_PATTERNS: &[&str] = &[
    "Unable to open system database",
    "error while loading shared libraries",
    "FATAL ERROR",
    "store has not been initialized",
];

/// Wait for the server to respond using TerminusDBHttpClient.
/// Also checks if the process has exited early or logged fatal errors.
async fn wait_for_ready(child: &mut Child, max_wait: Duration) -> anyhow::Result<()> {
    use std::io::Read;

    let start = std::time::Instant::now();

    eprintln!("[terminusdb-bin] Waiting for server to become ready...");

    // Take stderr for non-blocking reads
    let mut stderr_handle = child.stderr.take();
    let mut stderr_buffer = String::new();

    while start.elapsed() < max_wait {
        // Check if process has exited
        if let Some(status) = child.try_wait()? {
            // Process exited - read remaining stderr
            if let Some(ref mut stderr) = stderr_handle {
                let _ = stderr.read_to_string(&mut stderr_buffer);
            }
            let stderr_msg = stderr_buffer.trim();
            if stderr_msg.is_empty() {
                anyhow::bail!("Server process exited with status: {}", status);
            } else {
                anyhow::bail!(
                    "Server process exited with status {}: {}",
                    status,
                    stderr_msg
                );
            }
        }

        // Try to read any available stderr (non-blocking via set_nonblocking or small reads)
        if let Some(ref mut stderr) = stderr_handle {
            let mut buf = [0u8; 4096];
            // Set non-blocking mode for the read
            #[cfg(unix)]
            {
                use std::os::unix::io::AsRawFd;
                let fd = stderr.as_raw_fd();
                unsafe {
                    let flags = libc::fcntl(fd, libc::F_GETFL);
                    libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
                }
            }
            match stderr.read(&mut buf) {
                Ok(0) => {} // EOF
                Ok(n) => {
                    if let Ok(s) = std::str::from_utf8(&buf[..n]) {
                        stderr_buffer.push_str(s);
                        // Check for fatal error patterns
                        for pattern in FATAL_ERROR_PATTERNS {
                            if stderr_buffer.contains(pattern) {
                                // Kill the process since it's in a bad state
                                let _ = child.kill();
                                let _ = child.wait();
                                anyhow::bail!(
                                    "Server failed to start: {}",
                                    stderr_buffer.trim()
                                );
                            }
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data available, that's fine
                }
                Err(_) => {} // Ignore other read errors
            }
        }

        // Use the client's local_node() + info() for health check
        let client = TerminusDBHttpClient::local_node().await;
        match client.info().await {
            Ok(_) => {
                eprintln!("[terminusdb-bin] Server is ready!");
                // Put stderr back
                child.stderr = stderr_handle;
                return Ok(());
            }
            Err(e) => {
                if start.elapsed().as_secs() % 5 == 0 {
                    eprintln!(
                        "[terminusdb-bin] Still waiting... ({}s): {:?}",
                        start.elapsed().as_secs(),
                        e
                    );
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Timeout - include any stderr we collected
    let stderr_msg = stderr_buffer.trim();
    if stderr_msg.is_empty() {
        anyhow::bail!("Server did not become ready within {:?}", max_wait)
    } else {
        anyhow::bail!(
            "Server did not become ready within {:?}. Stderr: {}",
            max_wait,
            stderr_msg
        )
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that memory mode doesn't write any files to disk.
    #[tokio::test]
    async fn test_memory_mode_no_disk_writes() -> anyhow::Result<()> {
        // Create a fresh temp directory
        let test_dir = std::env::temp_dir().join(format!(
            "terminusdb-memory-test-{}",
            std::process::id()
        ));
        if test_dir.exists() {
            std::fs::remove_dir_all(&test_dir)?;
        }
        std::fs::create_dir_all(&test_dir)?;

        // Start server in memory mode using the public API
        let server = start_server(ServerOptions {
            memory: true,
            quiet: true,
            db_path: Some(test_dir.clone()),
            ..Default::default()
        })
        .await?;

        // Verify the server is responding
        let client = server.client().await?;
        client.info().await?;

        // Drop the server to stop it
        drop(server);

        // Check that no files were written
        let entries: Vec<_> = std::fs::read_dir(&test_dir)?.collect();
        assert!(
            entries.is_empty(),
            "Memory mode should not write files to disk, but found: {:?}",
            entries
                .iter()
                .filter_map(|e| e.as_ref().ok().map(|e| e.path()))
                .collect::<Vec<_>>()
        );

        // Cleanup
        std::fs::remove_dir_all(&test_dir)?;

        Ok(())
    }
}
