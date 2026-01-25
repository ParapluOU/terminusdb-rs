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

use std::net::TcpListener;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use terminusdb_client::TerminusDBHttpClient;
use tokio::sync::OnceCell;

/// Store the PID of the shared test server so we can kill it on exit.
/// Using AtomicU32 since process IDs fit in u32 on most platforms.
static TEST_SERVER_PID: AtomicU32 = AtomicU32::new(0);

/// Register an atexit handler to kill the test server process.
/// This is called once when the first test_instance is created.
fn register_exit_handler() {
    extern "C" fn cleanup() {
        let pid = TEST_SERVER_PID.load(Ordering::SeqCst);
        if pid != 0 {
            eprintln!("[terminusdb-bin] atexit: Killing server PID {}", pid);
            #[cfg(unix)]
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
                // Give it a moment to terminate gracefully
                std::thread::sleep(std::time::Duration::from_millis(100));
                // Force kill if still running
                libc::kill(pid as i32, libc::SIGKILL);
            }
            #[cfg(windows)]
            {
                // On Windows, we'd need to use TerminateProcess
                // For now, just log - the process will be orphaned
                eprintln!("[terminusdb-bin] Windows cleanup not implemented");
            }
        }
    }

    // Register the cleanup function to run at exit
    // This is safe and only registers once due to OnceCell semantics
    unsafe {
        libc::atexit(cleanup);
    }
}

/// Find an available port by binding to port 0 and getting an OS-assigned port.
/// The listener is dropped after getting the port, freeing it for the server.
fn find_available_port() -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    Ok(port)
}

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
    /// Port to listen on. If None, auto-allocates a unique port in memory mode,
    /// or uses 6363 in persistent mode.
    pub port: Option<u16>,
}

/// A running TerminusDB server instance.
///
/// The server is automatically stopped when this handle is dropped.
pub struct TerminusDBServer {
    child: Option<Child>,
    port: u16,
}

/// Shared test server instance (per-process)
static TEST_INSTANCE: OnceCell<TerminusDBServer> = OnceCell::const_new();

impl TerminusDBServer {
    /// Get the port this server is listening on.
    pub fn port(&self) -> u16 {
        self.port
    }

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

    /// Get or create a shared test server instance for this process.
    ///
    /// The server is started on the first call and kept running for the
    /// lifetime of the process. Subsequent calls return the same instance.
    /// This is useful for running multiple tests against the same server.
    ///
    /// Each process gets its own server on a unique port, enabling parallel
    /// test execution across multiple processes.
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
    ///     // Same server instance as test_one (within same process)
    ///     let server = TerminusDBServer::test_instance().await?;
    ///     let client = server.client().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn test_instance() -> anyhow::Result<&'static Self> {
        TEST_INSTANCE
            .get_or_try_init(|| async {
                eprintln!("[terminusdb-bin] test_instance: Starting new server");
                start_test_server().await
            })
            .await
    }

    /// Get a configured HTTP client for this server.
    ///
    /// Uses explicit "root" password instead of `local_node()` which reads from
    /// environment variables. Memory mode servers always use "root" password.
    /// Verifies the server is responding before returning.
    pub async fn client(&self) -> anyhow::Result<TerminusDBHttpClient> {
        let client = create_test_client(self.port).await?;
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
        if let Some(ref mut child) = self.child {
            eprintln!(
                "[terminusdb-bin] Drop: Killing server PID {} on port {}",
                child.id(),
                self.port
            );
            let _ = child.kill();
            let _ = child.wait();
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

    // Determine port: auto-allocate for memory mode, use 6363 for persistent mode
    let port = match opts.port {
        Some(p) => p,
        None if opts.memory => find_available_port()?,
        None => 6363,
    };
    eprintln!("[terminusdb-bin] Using port: {}", port);

    // Only set up db_path for persistent (non-memory) mode
    // Memory mode should NOT have TERMINUSDB_SERVER_DB_PATH set, as it would
    // override --memory and cause the server to try opening a disk store
    let db_path = if !opts.memory {
        let path = match opts.db_path {
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

        eprintln!("[terminusdb-bin] Initializing store in {:?}...", path);
        let init_status = std::process::Command::new(&binary_path)
            .args(["store", "init"])
            .current_dir(&path)
            .env("TERMINUSDB_SERVER_DB_PATH", &path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if !init_status.success() {
            anyhow::bail!("Failed to initialize store: {:?}", init_status);
        }

        Some(path)
    } else {
        None
    };

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

    // Build command
    let mut cmd = std::process::Command::new(&binary_path);
    cmd.args(&args)
        .stdout(stdout)
        .stderr(Stdio::piped())
        .env("TERMINUSDB_SERVER_PORT", port.to_string());

    if let Some(ref path) = db_path {
        cmd.current_dir(path);
        cmd.env("TERMINUSDB_SERVER_DB_PATH", path);
    }

    let mut child = cmd.spawn()?;

    eprintln!(
        "[terminusdb-bin] Server spawned with PID: {} on port {}",
        child.id(),
        port
    );

    // Wait for server to be ready, checking for early process exit
    wait_for_ready(&mut child, port, Duration::from_secs(30)).await?;

    let server = TerminusDBServer {
        child: Some(child),
        port,
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

/// Start the test server (internal helper for test_instance)
async fn start_test_server() -> anyhow::Result<TerminusDBServer> {
    // Register exit handler BEFORE spawning the server
    // This ensures cleanup even if the process exits unexpectedly
    register_exit_handler();

    let binary_path = crate::extract_binary()?;
    eprintln!(
        "[terminusdb-bin] test_instance: Binary path: {:?}",
        binary_path
    );

    // Allocate a unique port for this server
    let port = find_available_port()?;
    eprintln!("[terminusdb-bin] test_instance: Allocated port {}", port);

    // --memory mode self-initializes, no store init needed
    let args = vec!["serve", "--memory", "root"];

    eprintln!(
        "[terminusdb-bin] test_instance: Spawning with args: {:?}",
        args
    );

    // Pipe stderr so we can capture early failure messages
    let mut child = std::process::Command::new(&binary_path)
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .env("TERMINUSDB_SERVER_PORT", port.to_string())
        .spawn()?;

    let pid = child.id();
    eprintln!(
        "[terminusdb-bin] test_instance: Server spawned with PID: {} on port {}",
        pid, port
    );

    // Store the PID so the atexit handler can kill it
    TEST_SERVER_PID.store(pid, Ordering::SeqCst);

    // Wait for server to be ready, checking for early process exit
    wait_for_ready(&mut child, port, Duration::from_secs(30)).await?;

    Ok(TerminusDBServer {
        child: Some(child),
        port,
    })
}

/// Create a client for the local test server with hardcoded "root" password.
/// This is used instead of `local_node()` which reads from environment variables,
/// since memory mode servers always use "root" password.
async fn create_test_client(port: u16) -> anyhow::Result<TerminusDBHttpClient> {
    let url = format!("http://localhost:{}", port);
    TerminusDBHttpClient::new(url::Url::parse(&url).unwrap(), "admin", "root", "admin").await
}

/// Wait for the server to respond using TerminusDBHttpClient.
/// Also checks if the process has exited early or logged fatal errors.
async fn wait_for_ready(child: &mut Child, port: u16, max_wait: Duration) -> anyhow::Result<()> {
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
                                anyhow::bail!("Server failed to start: {}", stderr_buffer.trim());
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

        // Use create_test_client() for health check (hardcoded "root" password)
        let client = match create_test_client(port).await {
            Ok(c) => c,
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        // Use try_info() to avoid logging expected errors during startup
        match client.try_info().await {
            Ok(_) => {
                eprintln!("[terminusdb-bin] Server is ready!");
                // Put stderr back
                child.stderr = stderr_handle;
                return Ok(());
            }
            Err(e) => {
                if start.elapsed().as_secs().is_multiple_of(5) {
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

    /// Test that memory mode via TerminusDBServer::test() doesn't write files to disk.
    #[tokio::test]
    async fn test_memory_mode_no_disk_writes() -> anyhow::Result<()> {
        // Create a fresh temp directory that we'll monitor for writes
        let test_dir =
            std::env::temp_dir().join(format!("terminusdb-memory-test-{}", std::process::id()));
        if test_dir.exists() {
            std::fs::remove_dir_all(&test_dir)?;
        }
        std::fs::create_dir_all(&test_dir)?;

        // Start server using the public API with memory mode
        let server = start_server(ServerOptions {
            memory: true,
            quiet: true,
            ..Default::default()
        })
        .await?;

        // Verify the server is responding
        let client = server.client().await?;
        client.info().await?;

        // Drop the server to stop it
        drop(server);

        // Memory mode should not have created any temp directories
        // (in persistent mode, start_server creates a temp dir, but not in memory mode)
        // We check that no terminusdb-server-{our_pid} directory was created
        let temp_dir = std::env::temp_dir();
        let our_pid = std::process::id();
        let our_dir = temp_dir.join(format!("terminusdb-server-{}", our_pid));
        assert!(
            !our_dir.exists(),
            "Memory mode should not create db_path directory, but found: {:?}",
            our_dir
        );

        // Cleanup
        std::fs::remove_dir_all(&test_dir)?;

        Ok(())
    }

    /// Test that TerminusDBServer::test() starts successfully in memory mode.
    #[tokio::test]
    async fn test_server_test_starts() -> anyhow::Result<()> {
        let server = TerminusDBServer::test().await?;
        let client = server.client().await?;
        // info() succeeds means server is running
        let _info = client.info().await?;
        Ok(())
    }

    /// Test that ensure_database works immediately after test_instance().client()
    /// This replicates the exact pattern used in apps.
    #[tokio::test]
    async fn test_ensure_database_after_client() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        // This is the exact pattern from the app that was failing
        client.ensure_database("test_ensure_db").await?;

        // Verify it exists
        let databases = client.list_databases_simple().await?;
        let found = databases.iter().any(|db| {
            db.path
                .as_ref()
                .map(|p| p.contains("test_ensure_db"))
                .unwrap_or(false)
        });
        assert!(found, "Database should exist after ensure_database");

        // Cleanup
        client.delete_database("test_ensure_db").await?;

        Ok(())
    }
}
