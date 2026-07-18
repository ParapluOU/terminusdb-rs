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

use std::collections::BTreeMap;
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
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

/// Server log verbosity (`TERMINUSDB_LOG_LEVEL`).
///
/// The embedded server accepts `ERROR`, `WARNING`, `NOTICE`, `INFO`, `DEBUG`
/// (default `INFO`). This enum guarantees only a valid value is emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warning,
    Notice,
    Info,
    Debug,
}

impl LogLevel {
    /// The exact atom the server expects for `TERMINUSDB_LOG_LEVEL`.
    fn as_env(self) -> &'static str {
        match self {
            LogLevel::Error => "ERROR",
            LogLevel::Warning => "WARNING",
            LogLevel::Notice => "NOTICE",
            LogLevel::Info => "INFO",
            LogLevel::Debug => "DEBUG",
        }
    }
}

/// Server log output format (`TERMINUSDB_LOG_FORMAT`).
///
/// The embedded server accepts `text` (default) or `json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Text,
    Json,
}

impl LogFormat {
    /// The exact atom the server expects for `TERMINUSDB_LOG_FORMAT`.
    fn as_env(self) -> &'static str {
        match self {
            LogFormat::Text => "text",
            LogFormat::Json => "json",
        }
    }
}

/// Typed configuration for the embedded TerminusDB server's runtime settings.
///
/// Every field maps to a `TERMINUSDB_*` environment variable that the embedded
/// server binary reads at startup. Fields are applied **per spawned process**
/// (never via the global process environment), so a consumer can configure the
/// server without calling `std::env::set_var`. Any field left `None`/empty
/// falls through to the server's own default.
///
/// Only the variables the embedded binary actually reads at runtime are modeled
/// as typed fields. For anything not covered here (or added upstream later),
/// use the [`env`](Self::env) escape hatch, and [`extra_serve_args`](Self::extra_serve_args)
/// for extra `serve` CLI flags.
///
/// Note: the server's port, worker count, database path and admin password are
/// controlled by the dedicated [`ServerOptions`] fields (`port`, `workers`,
/// `db_path`, `password`) — do not set `TERMINUSDB_SERVER_PORT` /
/// `_SERVER_WORKERS` / `_SERVER_DB_PATH` via [`env`](Self::env), as the harness
/// sets those last and would override you.
#[derive(Debug, Clone, Default)]
pub struct ServerConfig {
    // --- Networking / identity -------------------------------------------
    /// Bind name/host (`TERMINUSDB_SERVER_NAME`). Default: a random string.
    pub server_name: Option<String>,
    /// Max retries for conflicting transactions
    /// (`TERMINUSDB_SERVER_MAX_TRANSACTION_RETRIES`). Default: workers × 2.
    pub max_transaction_retries: Option<u32>,

    // --- Storage / paths --------------------------------------------------
    /// Temp directory (`TERMINUSDB_SERVER_TMP_PATH`). Default: `/tmp`.
    pub tmp_path: Option<std::path::PathBuf>,
    /// Pack directory (`TERMINUSDB_SERVER_PACK_DIR`).
    pub pack_dir: Option<std::path::PathBuf>,
    /// Plugin registry file (`TERMINUSDB_SERVER_REGISTRY_PATH`).
    pub registry_path: Option<std::path::PathBuf>,
    /// Plugins directory (`TERMINUSDB_PLUGINS_PATH`). Default: `<db>/../plugins`.
    pub plugins_path: Option<std::path::PathBuf>,
    /// File-upload storage directory (`TERMINUSDB_FILE_STORAGE_PATH`).
    pub file_storage_path: Option<std::path::PathBuf>,
    /// System CA bundle for outbound TLS (`TERMINUSDB_SYSTEM_SSL_CERTS`).
    pub system_ssl_certs: Option<std::path::PathBuf>,

    // --- Dashboard --------------------------------------------------------
    /// Serve the web dashboard UI (`TERMINUSDB_ENABLE_DASHBOARD`). Default: true.
    pub enable_dashboard: Option<bool>,

    // --- Auth / JWT / headers --------------------------------------------
    /// Enable JWT authentication (`TERMINUSDB_JWT_ENABLED`). Default: false.
    pub jwt_enabled: Option<bool>,
    /// JWKS endpoint URL for JWT verification (`TERMINUSDB_SERVER_JWKS_ENDPOINT`).
    pub jwks_endpoint: Option<String>,
    /// JWT claim used as the agent name
    /// (`TERMINUSDB_JWT_AGENT_NAME_PROPERTY`). Default: `preferred_username`.
    pub jwt_agent_name_property: Option<String>,
    /// Trust an upstream-set user header (`TERMINUSDB_INSECURE_USER_HEADER_ENABLED`).
    /// Default: false. Insecure — only behind a trusted proxy.
    pub insecure_user_header_enabled: Option<bool>,
    /// Name of the trusted user header (`TERMINUSDB_INSECURE_USER_HEADER`).
    pub insecure_user_header: Option<String>,

    // --- Logging ----------------------------------------------------------
    /// Log verbosity (`TERMINUSDB_LOG_LEVEL`). Default: `Info`.
    pub log_level: Option<LogLevel>,
    /// Log format (`TERMINUSDB_LOG_FORMAT`). Default: `Text`.
    pub log_format: Option<LogFormat>,
    /// Include stack traces in HTTP error responses
    /// (`TERMINUSDB_EXPOSE_STACK_TRACES`). Default: false. Dev only.
    pub expose_stack_traces: Option<bool>,

    // --- Performance / caching -------------------------------------------
    /// Layer LRU cache size (`TERMINUSDB_LRU_CACHE_SIZE`). Default: 512.
    pub lru_cache_size: Option<u64>,
    /// Per-operation cache-eviction probability, 0.0–1.0
    /// (`TERMINUSDB_CACHE_EVICTION_PROBABILITY`). Default: 0.0 (disabled).
    pub cache_eviction_probability: Option<f64>,
    /// Document work limit (`TERMINUSDB_DOC_WORK_LIMIT`). Default: 500_000.
    pub doc_work_limit: Option<u64>,
    /// Enable query parallelization (`TERMINUSDB_PARALLELIZE_ENABLED`). Default: true.
    pub parallelize_enabled: Option<bool>,
    /// Worker elaboration preference, 0.0–1.0
    /// (`TERMINUSDB_WORKER_ELABORATION_PREFERENCE`). Default: 0.5.
    pub worker_elaboration_preference: Option<f64>,

    // --- Schema / migration ----------------------------------------------
    /// Skip loading the ref/repo schema (`TERMINUSDB_IGNORE_REF_AND_REPO_SCHEMA`).
    pub ignore_ref_and_repo_schema: Option<bool>,
    /// Auto-apply pending schema migrations (`TERMINUSDB_TRUST_MIGRATIONS`).
    pub trust_migrations: Option<bool>,

    // --- Pinning ----------------------------------------------------------
    /// Databases to keep resident (`TERMINUSDB_PINNED_DATABASES`, comma-joined).
    pub pinned_databases: Vec<String>,
    /// Organizations to keep resident (`TERMINUSDB_PINNED_ORGANIZATIONS`, comma-joined).
    pub pinned_organizations: Vec<String>,

    // --- Integrations -----------------------------------------------------
    /// gRPC label-service endpoint (`TERMINUSDB_GRPC_LABEL_ENDPOINT`).
    pub grpc_label_endpoint: Option<String>,
    /// Semantic indexer endpoint (`TERMINUSDB_SEMANTIC_INDEXER_ENDPOINT`).
    pub semantic_indexer_endpoint: Option<String>,

    // --- Escape hatches ---------------------------------------------------
    /// Arbitrary extra environment variables set on the server process.
    /// Applied after the typed fields above (so an entry here overrides them),
    /// but before the harness-controlled port/workers/db_path. Use for any
    /// `TERMINUSDB_*` var not modeled as a typed field.
    pub env: BTreeMap<String, String>,
    /// Extra positional arguments appended to the `serve` command line.
    pub extra_serve_args: Vec<String>,
}

impl ServerConfig {
    /// Apply every configured setting as an environment variable on `cmd`.
    ///
    /// Only `Some`/non-empty fields are set, so unset fields fall through to the
    /// server's own defaults. The generic [`env`](Self::env) map is applied last
    /// and therefore overrides any typed field it collides with.
    fn apply_env(&self, cmd: &mut Command) {
        // Booleans are emitted as the atoms the server compares against.
        fn b(v: bool) -> &'static str {
            if v {
                "true"
            } else {
                "false"
            }
        }

        if let Some(ref v) = self.server_name {
            cmd.env("TERMINUSDB_SERVER_NAME", v);
        }
        if let Some(v) = self.max_transaction_retries {
            cmd.env("TERMINUSDB_SERVER_MAX_TRANSACTION_RETRIES", v.to_string());
        }

        if let Some(ref v) = self.tmp_path {
            cmd.env("TERMINUSDB_SERVER_TMP_PATH", v);
        }
        if let Some(ref v) = self.pack_dir {
            cmd.env("TERMINUSDB_SERVER_PACK_DIR", v);
        }
        if let Some(ref v) = self.registry_path {
            cmd.env("TERMINUSDB_SERVER_REGISTRY_PATH", v);
        }
        if let Some(ref v) = self.plugins_path {
            cmd.env("TERMINUSDB_PLUGINS_PATH", v);
        }
        if let Some(ref v) = self.file_storage_path {
            cmd.env("TERMINUSDB_FILE_STORAGE_PATH", v);
        }
        if let Some(ref v) = self.system_ssl_certs {
            cmd.env("TERMINUSDB_SYSTEM_SSL_CERTS", v);
        }

        if let Some(v) = self.enable_dashboard {
            cmd.env("TERMINUSDB_ENABLE_DASHBOARD", b(v));
        }

        if let Some(v) = self.jwt_enabled {
            cmd.env("TERMINUSDB_JWT_ENABLED", b(v));
        }
        if let Some(ref v) = self.jwks_endpoint {
            cmd.env("TERMINUSDB_SERVER_JWKS_ENDPOINT", v);
        }
        if let Some(ref v) = self.jwt_agent_name_property {
            cmd.env("TERMINUSDB_JWT_AGENT_NAME_PROPERTY", v);
        }
        if let Some(v) = self.insecure_user_header_enabled {
            cmd.env("TERMINUSDB_INSECURE_USER_HEADER_ENABLED", b(v));
        }
        if let Some(ref v) = self.insecure_user_header {
            cmd.env("TERMINUSDB_INSECURE_USER_HEADER", v);
        }

        if let Some(v) = self.log_level {
            cmd.env("TERMINUSDB_LOG_LEVEL", v.as_env());
        }
        if let Some(v) = self.log_format {
            cmd.env("TERMINUSDB_LOG_FORMAT", v.as_env());
        }
        if let Some(v) = self.expose_stack_traces {
            cmd.env("TERMINUSDB_EXPOSE_STACK_TRACES", b(v));
        }

        if let Some(v) = self.lru_cache_size {
            cmd.env("TERMINUSDB_LRU_CACHE_SIZE", v.to_string());
        }
        if let Some(v) = self.cache_eviction_probability {
            cmd.env("TERMINUSDB_CACHE_EVICTION_PROBABILITY", v.to_string());
        }
        if let Some(v) = self.doc_work_limit {
            cmd.env("TERMINUSDB_DOC_WORK_LIMIT", v.to_string());
        }
        if let Some(v) = self.parallelize_enabled {
            cmd.env("TERMINUSDB_PARALLELIZE_ENABLED", b(v));
        }
        if let Some(v) = self.worker_elaboration_preference {
            cmd.env("TERMINUSDB_WORKER_ELABORATION_PREFERENCE", v.to_string());
        }

        if let Some(v) = self.ignore_ref_and_repo_schema {
            cmd.env("TERMINUSDB_IGNORE_REF_AND_REPO_SCHEMA", b(v));
        }
        if let Some(v) = self.trust_migrations {
            cmd.env("TERMINUSDB_TRUST_MIGRATIONS", b(v));
        }

        if !self.pinned_databases.is_empty() {
            cmd.env("TERMINUSDB_PINNED_DATABASES", self.pinned_databases.join(","));
        }
        if !self.pinned_organizations.is_empty() {
            cmd.env(
                "TERMINUSDB_PINNED_ORGANIZATIONS",
                self.pinned_organizations.join(","),
            );
        }

        if let Some(ref v) = self.grpc_label_endpoint {
            cmd.env("TERMINUSDB_GRPC_LABEL_ENDPOINT", v);
        }
        if let Some(ref v) = self.semantic_indexer_endpoint {
            cmd.env("TERMINUSDB_SEMANTIC_INDEXER_ENDPOINT", v);
        }

        // Generic passthrough last so callers can override any typed field.
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
    }
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
    /// Enable test mode optimizations (longer timeouts, fewer workers).
    /// When true, automatically sets sensible defaults for parallel test execution.
    pub test_mode: bool,
    /// Number of Prolog workers for the server (default: 8).
    /// Set lower in test mode to reduce resource contention when running
    /// multiple server instances in parallel.
    pub workers: Option<u8>,
    /// Request timeout for clients created via `client()`.
    /// If None in test_mode, defaults to 15 minutes.
    /// If None otherwise, uses TERMINUSDB_DEFAULT_REQUEST_TIMEOUT or 60 seconds.
    pub request_timeout: Option<Duration>,
    /// Typed configuration for the embedded server's runtime `TERMINUSDB_*`
    /// settings (logging, auth, caching, paths, …). Applied per-process, so the
    /// server can be fully configured without setting real environment vars.
    pub config: ServerConfig,
}

/// Default worker count for test mode (reduced to minimize resource contention).
const TEST_MODE_WORKERS: u8 = 2;

/// Default request timeout for test mode (15 minutes).
const TEST_MODE_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// A running TerminusDB server instance.
///
/// The server is automatically stopped when this handle is dropped.
pub struct TerminusDBServer {
    child: Option<Child>,
    port: u16,
    /// Request timeout for clients created via `client()`.
    request_timeout: Option<Duration>,
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
    /// Equivalent to `start_server(ServerOptions { memory: true, quiet: true, test_mode: true, .. })`.
    ///
    /// Test mode enables:
    /// - Reduced worker count (2) to minimize resource contention
    /// - Long client timeouts (15 minutes) for parallel test execution
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
            test_mode: true,
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
    ///
    /// If the server was started with `test_mode: true`, the client will have
    /// a 15-minute request timeout to handle resource contention during parallel tests.
    pub async fn client(&self) -> anyhow::Result<TerminusDBHttpClient> {
        let client = create_test_client(self.port, self.request_timeout).await?;
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

    /// Run a test/example with a temporary database seeded with `seed_entities`.
    ///
    /// Builds on [`with_db_schema`](Self::with_db_schema): it auto-inserts the
    /// schema tree for `M` (including any subdocuments it references), then
    /// inserts every entity, then hands the client and `BranchSpec` to the
    /// closure. The database is deleted when the closure returns (even on
    /// failure). This removes the need for a hand-written `seed()` step for the
    /// common case of a single top-level model.
    ///
    /// For several *unrelated* top-level types, use
    /// [`with_db_schema`](Self::with_db_schema) and insert entities yourself.
    ///
    /// # Example
    ///
    /// ```ignore
    /// server
    ///     .with_db_seed("test", vec![alice, bob], |client, spec| async move {
    ///         // The Person schema tree and both entities are already inserted.
    ///         Ok(())
    ///     })
    ///     .await
    /// ```
    pub async fn with_db_seed<M, F, Fut, R>(
        &self,
        prefix: &str,
        seed_entities: impl IntoIterator<Item = M>,
        f: F,
    ) -> anyhow::Result<R>
    where
        M: terminusdb_schema::TerminusDBModel + terminusdb_schema::ToTDBSchema,
        F: FnOnce(TerminusDBHttpClient, terminusdb_client::BranchSpec) -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<R>>,
    {
        // Collect up front so we don't hold the iterator across await points.
        let entities: Vec<M> = seed_entities.into_iter().collect();
        // Delegate schema insertion to with_db_schema (single source of truth),
        // then insert the entities before running the closure.
        self.with_db_schema::<(M,), _, _, _>(prefix, |client, spec| async move {
            let args = terminusdb_client::DocumentInsertArgs::from(spec.clone());
            for entity in &entities {
                client.insert_instance(entity, args.clone()).await?;
            }
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

/// Maximum number of retry attempts when port allocation fails due to race conditions.
const MAX_PORT_RETRIES: usize = 5;

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
    // Retry with different ports in case of race conditions
    // (port can be taken between find_available_port and server bind)
    let mut last_error = None;
    for attempt in 0..MAX_PORT_RETRIES {
        match start_server_attempt(&opts).await {
            Ok(server) => return Ok(server),
            Err(e) => {
                let err_str = e.to_string();
                // Check if this is a port-in-use error
                if err_str.contains("already in use")
                    || err_str.contains("Address already in use")
                    || err_str.contains("exit status: 98")
                {
                    eprintln!(
                        "[terminusdb-bin] Port conflict on attempt {}, retrying...",
                        attempt + 1
                    );
                    last_error = Some(e);
                    // Small delay before retry to let other processes finish binding
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    continue;
                }
                // Not a port conflict, fail immediately
                return Err(e);
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Failed to start server after retries")))
}

/// Internal: Single attempt to start the server.
async fn start_server_attempt(opts: &ServerOptions) -> anyhow::Result<TerminusDBServer> {
    let binary_path = crate::extract_binary()?;
    eprintln!("[terminusdb-bin] Binary path: {:?}", binary_path);

    // Determine port: auto-allocate for memory mode, use 6363 for persistent mode
    let port = match opts.port {
        Some(p) => p,
        None if opts.memory => find_available_port()?,
        None => 6363,
    };
    eprintln!("[terminusdb-bin] Using port: {}", port);

    // Determine worker count: explicit > test_mode default > system default (8)
    let workers = opts.workers.or(if opts.test_mode {
        Some(TEST_MODE_WORKERS)
    } else {
        None
    });
    if let Some(w) = workers {
        eprintln!("[terminusdb-bin] Using {} workers", w);
    }

    // Determine request timeout for clients
    let request_timeout = opts.request_timeout.or(if opts.test_mode {
        Some(TEST_MODE_TIMEOUT)
    } else {
        None
    });
    if let Some(t) = request_timeout {
        eprintln!(
            "[terminusdb-bin] Client request timeout: {} seconds",
            t.as_secs()
        );
    }

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
        let mut init_cmd = Command::new(&binary_path);
        init_cmd.args(["store", "init"]).current_dir(&path);
        // Apply typed/generic config first; the db path is set last so it stays
        // authoritative even if a caller put a value in `config.env`.
        opts.config.apply_env(&mut init_cmd);
        init_cmd.env("TERMINUSDB_SERVER_DB_PATH", &path);
        crate::apply_runtime_env(&mut init_cmd)?;
        let init_output = init_cmd.output()?;
        // The terminusdb binary always exits non-zero due to a SWI-Prolog
        // `unwind(halt(N))` quirk, even when `store init` succeeds. Trust
        // the operation if either:
        //   - stdout shows "Successfully initialised", or
        //   - stderr says the store already exists.
        let stdout = String::from_utf8_lossy(&init_output.stdout);
        let stderr = String::from_utf8_lossy(&init_output.stderr);
        let succeeded = stdout.contains("Successfully initialised")
            || stderr.contains("StorageAlreadyInitializedError")
            || stderr.contains("already initialized");
        if !succeeded {
            anyhow::bail!(
                "Failed to initialize store: status={:?}, stdout={:?}, stderr={:?}",
                init_output.status,
                stdout,
                stderr
            );
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
    // Any extra positional `serve` flags requested by the caller.
    args.extend(opts.config.extra_serve_args.iter().cloned());

    let stdout = if opts.quiet {
        Stdio::null()
    } else {
        Stdio::inherit()
    };

    eprintln!("[terminusdb-bin] Spawning server with args: {:?}", args);

    // Build command
    let mut cmd = Command::new(&binary_path);
    cmd.args(&args).stdout(stdout).stderr(Stdio::piped());

    // Apply the caller's typed/generic server config first...
    opts.config.apply_env(&mut cmd);

    // ...then the harness-controlled settings last, so port/workers/db_path
    // always win over anything set via `config` (e.g. a stray `config.env`).
    cmd.env("TERMINUSDB_SERVER_PORT", port.to_string());
    if let Some(w) = workers {
        cmd.env("TERMINUSDB_SERVER_WORKERS", w.to_string());
    }
    if let Some(ref path) = db_path {
        cmd.current_dir(path);
        cmd.env("TERMINUSDB_SERVER_DB_PATH", path);
    }

    // Point the binary at its bundled SWI-Prolog runtime (Linux self-contained embed).
    crate::apply_runtime_env(&mut cmd)?;

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
        request_timeout,
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

    // Test mode: use reduced workers and long timeout
    let workers = TEST_MODE_WORKERS;
    let request_timeout = TEST_MODE_TIMEOUT;
    eprintln!(
        "[terminusdb-bin] test_instance: Using {} workers, {} second client timeout",
        workers,
        request_timeout.as_secs()
    );

    // --memory mode self-initializes, no store init needed
    let args = vec!["serve", "--memory", "root"];

    eprintln!(
        "[terminusdb-bin] test_instance: Spawning with args: {:?}",
        args
    );

    // Pipe stderr so we can capture early failure messages
    let mut cmd = std::process::Command::new(&binary_path);
    cmd.args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .env("TERMINUSDB_SERVER_PORT", port.to_string())
        .env("TERMINUSDB_SERVER_WORKERS", workers.to_string());
    // The v12 binary links libswipl dynamically; point it at the bundled
    // SWI-Prolog home and shared libs like every other spawn path does.
    crate::apply_runtime_env(&mut cmd)?;
    let mut child = cmd.spawn()?;

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
        request_timeout: Some(request_timeout),
    })
}

/// Create a client for the local test server with hardcoded "root" password.
/// This is used instead of `local_node()` which reads from environment variables,
/// since memory mode servers always use "root" password.
///
/// # Arguments
/// * `port` - The port the server is listening on
/// * `request_timeout` - Optional request timeout. If None, uses the default (60 seconds).
async fn create_test_client(
    port: u16,
    request_timeout: Option<Duration>,
) -> anyhow::Result<TerminusDBHttpClient> {
    let url = format!("http://localhost:{}", port);
    TerminusDBHttpClient::new_with_timeout(
        url::Url::parse(&url).unwrap(),
        "admin",
        "root",
        "admin",
        request_timeout,
    )
    .await
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
        // Use default timeout for health checks - we want them to be fast
        let client = match create_test_client(port, None).await {
            Ok(c) => c,
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        // Use try_info() to avoid logging expected errors during startup
        match client.try_info().await {
            Ok(_) => {
                // Info endpoint responded, but server might still be synchronizing.
                // Verify with an actual database operation (list_databases is fast).
                match client.list_databases_simple().await {
                    Ok(_) => {
                        eprintln!("[terminusdb-bin] Server is ready!");
                        // Put stderr back
                        child.stderr = stderr_handle;
                        return Ok(());
                    }
                    Err(e) => {
                        // Server is still synchronizing, keep waiting
                        if start.elapsed().as_secs().is_multiple_of(5)
                            && start.elapsed().as_secs() > 0
                        {
                            eprintln!(
                                "[terminusdb-bin] Still waiting... ({}s): server responding but not ready: {:?}",
                                start.elapsed().as_secs(),
                                e
                            );
                        }
                    }
                }
            }
            Err(e) => {
                if start.elapsed().as_secs().is_multiple_of(5) && start.elapsed().as_secs() > 0 {
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

    /// `ServerConfig::apply_env` should set exactly the configured vars (typed +
    /// generic passthrough), leave unset fields untouched, and let the generic
    /// `env` map override a typed field. No live server required.
    #[test]
    fn server_config_apply_env_maps_fields() {
        use std::collections::HashMap;

        let cfg = ServerConfig {
            jwt_enabled: Some(true),
            enable_dashboard: Some(false),
            log_level: Some(LogLevel::Debug),
            log_format: Some(LogFormat::Json),
            lru_cache_size: Some(1024),
            worker_elaboration_preference: Some(0.25),
            pinned_databases: vec!["admin/foo".into(), "admin/bar".into()],
            // Collides with a typed field on purpose: the map wins.
            env: BTreeMap::from([
                ("TERMINUSDB_LOG_LEVEL".to_string(), "WARNING".to_string()),
                ("TERMINUSDB_CUSTOM".to_string(), "x".to_string()),
            ]),
            ..Default::default()
        };

        let mut cmd = Command::new("true");
        cfg.apply_env(&mut cmd);

        let envs: HashMap<String, String> = cmd
            .get_envs()
            .filter_map(|(k, v)| Some((k.to_str()?.to_string(), v?.to_str()?.to_string())))
            .collect();

        assert_eq!(envs.get("TERMINUSDB_JWT_ENABLED").map(String::as_str), Some("true"));
        assert_eq!(
            envs.get("TERMINUSDB_ENABLE_DASHBOARD").map(String::as_str),
            Some("false")
        );
        assert_eq!(envs.get("TERMINUSDB_LOG_FORMAT").map(String::as_str), Some("json"));
        assert_eq!(envs.get("TERMINUSDB_LRU_CACHE_SIZE").map(String::as_str), Some("1024"));
        assert_eq!(
            envs.get("TERMINUSDB_WORKER_ELABORATION_PREFERENCE").map(String::as_str),
            Some("0.25")
        );
        assert_eq!(
            envs.get("TERMINUSDB_PINNED_DATABASES").map(String::as_str),
            Some("admin/foo,admin/bar")
        );
        assert_eq!(envs.get("TERMINUSDB_CUSTOM").map(String::as_str), Some("x"));
        // Generic map applied last, so it overrides the typed LogLevel::Debug.
        assert_eq!(envs.get("TERMINUSDB_LOG_LEVEL").map(String::as_str), Some("WARNING"));
        // Unset fields must not be emitted (fall through to server defaults).
        assert!(!envs.contains_key("TERMINUSDB_TRUST_MIGRATIONS"));
        assert!(!envs.contains_key("TERMINUSDB_SERVER_TMP_PATH"));
    }

    /// End-to-end: a memory server started with several `config` options set
    /// must still boot and respond. This exercises the full spawn path with the
    /// new `TERMINUSDB_*` env vars applied (not just the pure mapping unit test).
    #[tokio::test]
    async fn test_server_starts_with_typed_config() -> anyhow::Result<()> {
        let server = start_server(ServerOptions {
            memory: true,
            quiet: true,
            test_mode: true,
            config: ServerConfig {
                log_format: Some(LogFormat::Json),
                log_level: Some(LogLevel::Warning),
                enable_dashboard: Some(false),
                lru_cache_size: Some(1024),
                parallelize_enabled: Some(true),
                ..Default::default()
            },
            ..Default::default()
        })
        .await?;

        // Server accepted the config and is serving requests.
        let client = server.client().await?;
        client.info().await?;
        Ok(())
    }

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

    /// Test running multiple TerminusDBServer instances in parallel.
    ///
    /// This stress tests:
    /// - Unique port allocation across concurrent server starts
    /// - Test mode settings (reduced workers, long timeouts)
    /// - Schema insertion and querying under resource contention
    ///
    /// Each server runs in-memory with test mode enabled (2 workers, 15 min timeout).
    #[tokio::test]
    async fn test_parallel_server_instances() -> anyhow::Result<()> {
        use futures::future::join_all;
        use terminusdb_client::DocumentInsertArgs;
        use terminusdb_schema::{EntityIDFor, ToTDBInstance};
        use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

        // Simple test model
        #[derive(Debug, Clone, PartialEq, Default, TerminusDBModel, FromTDBInstance)]
        #[tdb(id_field = "id")]
        struct ParallelTestModel {
            id: EntityIDFor<Self>,
            name: String,
            server_index: i32,
        }

        // Number of parallel servers to spawn
        // Using more than typical CPU core count to stress test resource contention
        const NUM_SERVERS: usize = 16;

        eprintln!(
            "[test] Starting {} parallel TerminusDBServer instances...",
            NUM_SERVERS
        );

        // Spawn all servers in parallel
        let handles: Vec<_> = (0..NUM_SERVERS)
            .map(|i| {
                tokio::spawn(async move {
                    let start = std::time::Instant::now();
                    eprintln!("[test] Server {} starting...", i);

                    // Create a new test server (each gets unique port, test mode enabled)
                    let server = TerminusDBServer::test().await?;
                    let port = server.port();
                    eprintln!(
                        "[test] Server {} started on port {} in {:?}",
                        i,
                        port,
                        start.elapsed()
                    );

                    // Get client and create a unique database
                    let client = server.client().await?;
                    let db_name = format!("parallel_test_{}", i);
                    client.ensure_database(&db_name).await?;
                    eprintln!("[test] Server {} created database '{}'", i, db_name);

                    // Insert schema
                    let spec = terminusdb_client::BranchSpec::with_branch(&db_name, "main");
                    let args = DocumentInsertArgs::from(spec.clone());
                    client.insert_schemas::<(ParallelTestModel,)>(args).await?;
                    eprintln!("[test] Server {} inserted schema", i);

                    // Insert a test instance
                    let instance = ParallelTestModel {
                        id: EntityIDFor::new(&format!("test_item_{}", i)).unwrap(),
                        name: format!("Test Item from Server {}", i),
                        server_index: i as i32,
                    };
                    let args = DocumentInsertArgs::from(spec.clone());
                    client.insert_instance(&instance, args).await?;
                    eprintln!("[test] Server {} inserted instance", i);

                    // Query to verify using list_instances
                    let results: Vec<ParallelTestModel> =
                        client.list_instances(&spec, None, None).await?;
                    eprintln!(
                        "[test] Server {} queried {} instances",
                        i,
                        results.len()
                    );

                    // Verify we got our instance back
                    assert_eq!(results.len(), 1, "Server {} should have 1 instance", i);
                    assert_eq!(
                        results[0].server_index, i as i32,
                        "Server {} instance should have correct server_index",
                        i
                    );

                    // Cleanup
                    client.delete_database(&db_name).await?;
                    eprintln!(
                        "[test] Server {} completed successfully in {:?}",
                        i,
                        start.elapsed()
                    );

                    Ok::<(usize, u16), anyhow::Error>((i, port))
                })
            })
            .collect();

        // Wait for all servers to complete
        let results = join_all(handles).await;

        // Collect results and check for errors
        let mut ports = Vec::new();
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(Ok((server_idx, port))) => {
                    eprintln!("[test] Server {} (port {}) succeeded", server_idx, port);
                    ports.push(port);
                }
                Ok(Err(e)) => {
                    panic!("Server {} failed with error: {:?}", i, e);
                }
                Err(e) => {
                    panic!("Server {} task panicked: {:?}", i, e);
                }
            }
        }

        // Verify all ports were unique
        let unique_ports: std::collections::HashSet<_> = ports.iter().collect();
        assert_eq!(
            unique_ports.len(),
            ports.len(),
            "All servers should have unique ports. Ports: {:?}",
            ports
        );

        eprintln!(
            "[test] All {} parallel servers completed successfully with unique ports: {:?}",
            NUM_SERVERS, ports
        );

        Ok(())
    }
}
