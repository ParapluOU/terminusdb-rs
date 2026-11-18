//! Shared harness: a real Reactor, in-process, in front of a real TerminusDB.
//!
//! Every test gets three things:
//!
//! * a client aimed **at Reactor**,
//! * a client aimed **directly at the node**,
//! * raw HTTP access through Reactor, for header assertions.
//!
//! Having both clients is the point. Almost every claim in this design is
//! comparative — "the same bytes as the node would have returned", "the new
//! value after a commit" — and a test that only talks to the cache cannot
//! distinguish a correct answer from a confidently wrong one.

#![allow(dead_code)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::TerminusDBHttpClient;
use terminusdb_reactor::capture::{CaptureHint, Supervisor};
use terminusdb_reactor::config::{Config, Driver, NamespaceConfig, NamespacePolicy};
use terminusdb_reactor::engine::Engine;
use terminusdb_reactor::frontend::{self, AppState};
use terminusdb_reactor::ids::Namespace;
use terminusdb_reactor::store::TieredStore;
use terminusdb_reactor::CommitId;

/// Install a tracing subscriber once, so a capture failure inside the harness
/// shows up in test output instead of manifesting as a mute timeout.
pub fn init_tracing() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("terminusdb_reactor=warn"));
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_test_writer()
            .try_init();
    });
}

/// The TerminusDB node these tests run against.
///
/// `Embedded` is the preferred path and what CLAUDE.md prescribes: a per-process
/// server, unique database per test, parallel-safe, no `#[ignore]`.
///
/// `External` exists because `TerminusDBServer::test_instance()` extracts its
/// server binary into a **shared** temp-dir cache keyed only on file size, and
/// rewrites it whenever the size differs. When another build in the same
/// workspace has produced a differently-sized binary and is *executing* the
/// cached one, every extraction fails with `ETXTBSY` and no embedded server can
/// start at all. Rather than skip the whole integration suite in that
/// situation, the harness falls back to a node that is already running. The
/// isolation story is unchanged — a uuid-named database per test, deleted
/// afterwards — and the chosen upstream is printed so the test output records
/// which one was used.
pub enum Upstream {
    /// A per-process embedded server.
    Embedded(&'static TerminusDBServer),
    /// An already-running node.
    External {
        /// Its port.
        port: u16,
    },
}

impl Upstream {
    /// Resolve an upstream: an explicit port, else an embedded server, else a
    /// node already listening on the conventional port.
    pub async fn resolve() -> Result<Upstream> {
        if let Ok(port) = std::env::var("REACTOR_TEST_UPSTREAM_PORT") {
            if let Ok(port) = port.parse::<u16>() {
                eprintln!(
                    "[harness] using the upstream named by REACTOR_TEST_UPSTREAM_PORT: {port}"
                );
                return Ok(Upstream::External { port });
            }
        }

        let mut embedded_error = None;
        for attempt in 1..=6 {
            match TerminusDBServer::test_instance().await {
                Ok(server) => {
                    eprintln!(
                        "[harness] using an embedded server on port {}",
                        server.port()
                    );
                    return Ok(Upstream::Embedded(server));
                }
                Err(err) => {
                    eprintln!("[harness] embedded server attempt {attempt} failed: {err:#}");
                    embedded_error = Some(err);
                    tokio::time::sleep(Duration::from_millis(400 * attempt)).await;
                }
            }
        }

        if probe_port(6363).await {
            eprintln!(
                "[harness] FALLING BACK to the node already running on :6363 \
                 (embedded server unavailable: {})",
                embedded_error
                    .as_ref()
                    .map(|e| e.to_string())
                    .unwrap_or_default()
            );
            return Ok(Upstream::External { port: 6363 });
        }

        Err(embedded_error
            .unwrap_or_else(|| anyhow::anyhow!("no embedded and no external TerminusDB available")))
    }

    /// The port the node listens on.
    pub fn port(&self) -> u16 {
        match self {
            Upstream::Embedded(server) => server.port(),
            Upstream::External { port } => *port,
        }
    }

    /// A client aimed directly at the node.
    pub async fn client(&self) -> Result<TerminusDBHttpClient> {
        match self {
            Upstream::Embedded(server) => server.client().await,
            Upstream::External { port } => TerminusDBHttpClient::new_with_timeout(
                url::Url::parse(&format!("http://localhost:{port}"))?,
                "admin",
                "root",
                "admin",
                Some(Duration::from_secs(120)),
            )
            .await
            .context("connecting to the external test node"),
        }
    }

    /// Run a test against a uniquely named, automatically deleted database.
    pub async fn with_tmp_db<F, Fut, T>(&self, prefix: &str, f: F) -> Result<T>
    where
        F: FnOnce(TerminusDBHttpClient, terminusdb_client::BranchSpec) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        match self {
            Upstream::Embedded(server) => server.with_tmp_db(prefix, f).await,
            Upstream::External { .. } => {
                let db = format!("{prefix}_{}", uuid::Uuid::new_v4().simple());
                let client = self.client().await?;
                client.ensure_database(&db).await?;
                let spec = terminusdb_client::BranchSpec::with_branch(&db, "main");
                let result = f(client.clone(), spec).await;
                let _ = client.delete_database(&db).await;
                result
            }
        }
    }

    /// As [`Upstream::with_tmp_db`], with the schemas for `S` pre-inserted.
    pub async fn with_schema<S, F, Fut, T>(&self, prefix: &str, f: F) -> Result<T>
    where
        S: terminusdb_schema::ToTDBSchemas,
        F: FnOnce(TerminusDBHttpClient, terminusdb_client::BranchSpec) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        self.with_tmp_db(prefix, |client, spec| async move {
            let args = terminusdb_client::DocumentInsertArgs::from(spec.clone());
            client.insert_schemas::<S>(args).await?;
            f(client, spec).await
        })
        .await
    }
}

impl Upstream {
    /// The node's reported version, e.g. `12.1.0-rc3` or `12.0.6`.
    pub async fn version(&self) -> Result<String> {
        let info: serde_json::Value = serde_json::from_slice(
            &reqwest::Client::new()
                .get(format!("http://localhost:{}/api/info", self.port()))
                .basic_auth("admin", Some("root"))
                .send()
                .await?
                .bytes()
                .await?,
        )?;
        Ok(info["api:info"]["terminusdb"]["version"]
            .as_str()
            .unwrap_or("unknown")
            .to_owned())
    }

    /// Does this node have `GET /api/changes`?
    ///
    /// It is a TerminusDB **12.1** endpoint. On 12.0.x it 404s — and because
    /// the client's `ChangesResponse` defaults every field, that 404 body
    /// deserializes into three empty lists rather than an error. The resolver
    /// now treats an empty list as *unknown* (firing `Everything`), so Reactor
    /// stays sound on an older node; but **per-document invalidation precision
    /// cannot be demonstrated there**, so tests that assert precision check this
    /// first and say so.
    pub async fn supports_changes(&self) -> bool {
        let db = format!("rx_probe_{}", uuid::Uuid::new_v4().simple());
        let client = match self.client().await {
            Ok(client) => client,
            Err(_) => return false,
        };
        if client.ensure_database(&db).await.is_err() {
            return false;
        }
        let status = reqwest::Client::new()
            .get(format!(
                "http://localhost:{}/api/changes/admin/{}",
                self.port(),
                db
            ))
            .basic_auth("admin", Some("root"))
            .send()
            .await
            .map(|r| r.status().as_u16())
            .unwrap_or(0);
        let _ = client.delete_database(&db).await;
        status == 200
    }
}

/// Is something listening there?
async fn probe_port(port: u16) -> bool {
    reqwest::Client::new()
        .get(format!("http://localhost:{port}/api/info"))
        .basic_auth("admin", Some("root"))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Resolve the upstream for a test.
pub async fn upstream() -> Result<Upstream> {
    init_tracing();
    Upstream::resolve().await
}

/// A running Reactor plus everything needed to talk to it.
pub struct Reactor {
    /// `http://127.0.0.1:<port>`.
    pub base: url::Url,
    /// The namespace under test.
    pub ns: Namespace,
    /// The engine, for asserting on internal state.
    pub engine: Arc<Engine>,
    /// The effective configuration.
    pub config: Arc<Config>,
    /// A plain HTTP client, for header-level assertions.
    pub http: reqwest::Client,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Reactor {
    /// Start Reactor in front of `server`, tracking `db`'s `main` branch.
    ///
    /// The listener is bound before anything else so the test knows the port;
    /// `0` would otherwise be unknowable from outside.
    pub async fn start(server: &Upstream, db: &str) -> Result<Reactor> {
        Reactor::start_with(server, db, |_| {}).await
    }

    /// Start with a configuration tweak — a different driver, a shorter fence.
    pub async fn start_with(
        server: &Upstream,
        db: &str,
        tweak: impl FnOnce(&mut Config),
    ) -> Result<Reactor> {
        init_tracing();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .context("binding an ephemeral port")?;
        let addr = listener.local_addr()?;

        let mut config = Config::default();
        config.node.id = "test-reactor".to_owned();
        config.node.listen = addr.to_string();
        config.upstream.url = format!("http://localhost:{}", server.port());
        config.upstream.user = "admin".to_owned();
        config.upstream.pass = "root".to_owned();
        config.upstream.org = "admin".to_owned();
        // Tests want to see capture happen, not wait for it.
        config.capture.poll_interval_ms = 100;
        // Default to the poll driver in tests.
        //
        // FINDING: with `driver = "both"` the `_ChangeSet` GraphQL subscription
        // takes the embedded 12.1.0-rc3 server down — "Plugin handler failed"
        // followed by `unwind(halt(1))`, i.e. the server process exits, after
        // which every later test in the binary gets connection-refused. That
        // matches this workspace's note that the changeset-SSE plugin is
        // unstable and is disabled in docker. The subscription driver is
        // exercised on purpose by `tests/capture.rs`, which is the one place
        // that should risk it; everything else polls.
        config.capture.driver = Driver::Poll;
        config.consistency.write_fence_ms = 10_000;
        config.consistency.min_version_wait_ms = 2_000;
        config.namespaces = vec![NamespaceConfig {
            org: "admin".to_owned(),
            db: db.to_owned(),
            branch: "main".to_owned(),
            driver: None,
            policy: NamespacePolicy::Lazy,
        }];
        tweak(&mut config);
        config.validate()?;
        let config = Arc::new(config);
        let ns = config.namespaces()[0].clone();

        let store = Arc::new(TieredStore::memory_only(
            config.cache.memory_budget_mb * 1024 * 1024,
        ));
        let engine = Arc::new(Engine::new(config.engine_config(), store));

        let mut tasks = Vec::new();
        for ns in config.namespaces() {
            let writer = engine
                .take_writer(&ns)
                .context("taking the namespace writer")?;
            tasks.push(tokio::spawn(writer.run()));
        }

        let client = Arc::new(
            TerminusDBHttpClient::new_with_timeout(
                url::Url::parse(&config.upstream.url)?,
                &config.upstream.user,
                &config.upstream.pass,
                &config.upstream.org,
                Some(config.upstream.request_timeout()),
            )
            .await
            .context("building the upstream client")?,
        );

        let (hints, hint_rx) = tokio::sync::mpsc::unbounded_channel::<CaptureHint>();
        let supervisor = Arc::new(Supervisor::new(
            Arc::clone(&engine),
            Arc::clone(&client),
            Arc::clone(&config),
        ));
        tasks.push(tokio::spawn(Arc::clone(&supervisor).run(hint_rx)));

        let proxy = Arc::new(frontend::build_proxy(&config)?);
        let gate = Arc::new(frontend::build_gate(&config)?);
        let state = AppState::new(Arc::clone(&config), Arc::clone(&engine), proxy, gate, hints)
            .into_shared();
        let app = frontend::router(state);
        tasks.push(tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        }));

        let reactor = Reactor {
            base: url::Url::parse(&format!("http://{addr}"))?,
            ns,
            engine,
            config,
            http: reqwest::Client::new(),
            tasks,
        };
        reactor.await_ready().await?;
        Ok(reactor)
    }

    /// Wait until the listener answers and capture has established a watermark.
    ///
    /// Until the watermark exists every read bypasses — correctly, since there
    /// is no position to pin a fill at — so a test that raced it would look
    /// like a cache that never caches.
    pub async fn await_ready(&self) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(90);
        while Instant::now() < deadline {
            if self
                .http
                .get(self.url("/healthz"))
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false)
                && self.engine.watermark(&self.ns).is_some()
            {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        anyhow::bail!("Reactor did not become ready (no watermark established)")
    }

    /// An absolute URL through Reactor.
    pub fn url(&self, path_and_query: &str) -> String {
        format!(
            "{}{}",
            self.base.as_str().trim_end_matches('/'),
            path_and_query
        )
    }

    /// A `terminusdb-client` aimed at Reactor rather than at the node.
    pub async fn client(&self) -> Result<TerminusDBHttpClient> {
        TerminusDBHttpClient::new_with_timeout(
            self.base.clone(),
            "admin",
            "root",
            "admin",
            Some(Duration::from_secs(120)),
        )
        .await
        .context("building a client aimed at Reactor")
    }

    /// A raw authenticated GET through Reactor.
    pub async fn get(&self, path_and_query: &str) -> Result<Probe> {
        self.get_as(path_and_query, Some(("admin", "root"))).await
    }

    /// A raw GET with specific (or no) credentials.
    pub async fn get_as(
        &self,
        path_and_query: &str,
        credentials: Option<(&str, &str)>,
    ) -> Result<Probe> {
        let mut request = self.http.get(self.url(path_and_query));
        if let Some((user, pass)) = credentials {
            request = request.basic_auth(user, Some(pass));
        }
        Probe::of(request.send().await?).await
    }

    /// A raw GET with extra headers.
    pub async fn get_with(&self, path_and_query: &str, headers: &[(&str, &str)]) -> Result<Probe> {
        let mut request = self
            .http
            .get(self.url(path_and_query))
            .basic_auth("admin", Some("root"));
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        Probe::of(request.send().await?).await
    }

    /// A raw POST through Reactor.
    pub async fn post(&self, path_and_query: &str, body: serde_json::Value) -> Result<Probe> {
        let request = self
            .http
            .post(self.url(path_and_query))
            .basic_auth("admin", Some("root"))
            .json(&body);
        Probe::of(request.send().await?).await
    }

    /// A raw POST with extra headers, for stating cache conditions.
    pub async fn post_with(
        &self,
        path_and_query: &str,
        body: serde_json::Value,
        headers: &[(&str, &str)],
    ) -> Result<Probe> {
        let mut request = self
            .http
            .post(self.url(path_and_query))
            .basic_auth("admin", Some("root"))
            .json(&body);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        Probe::of(request.send().await?).await
    }

    /// Wait until a commit has been applied here, so a test can assert on the
    /// state *after* invalidation rather than racing it.
    pub async fn await_commit(&self, commit: &CommitId) -> bool {
        self.await_commit_for(commit, Duration::from_secs(20)).await
    }

    /// As [`Reactor::await_commit`], with an explicit budget.
    pub async fn await_commit_for(&self, commit: &CommitId, budget: Duration) -> bool {
        use terminusdb_reactor::engine::CommitLookup;
        let deadline = Instant::now() + budget;
        while Instant::now() < deadline {
            if matches!(
                self.engine.position_of(&self.ns, commit),
                CommitLookup::Applied(_)
            ) {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        false
    }

    /// Wait until the watermark reports a different commit than `before`.
    pub async fn await_watermark_change(&self, before: Option<&CommitId>) -> Option<CommitId> {
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if let Some(watermark) = self.engine.watermark(&self.ns) {
                if Some(&watermark.commit) != before {
                    return Some(watermark.commit);
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        None
    }

    /// The current watermark commit, if any.
    pub fn head(&self) -> Option<CommitId> {
        self.engine.watermark(&self.ns).map(|w| w.commit)
    }
}

impl Drop for Reactor {
    fn drop(&mut self) {
        // Abort rather than signal: the test is over, and a lingering capture
        // task would keep polling a database the harness is about to delete.
        for task in &self.tasks {
            task.abort();
        }
    }
}

/// A response captured whole, so assertions can look at status, headers and
/// bytes without worrying about streaming.
pub struct Probe {
    /// HTTP status.
    pub status: u16,
    /// Response headers.
    pub headers: http::HeaderMap,
    /// The body, verbatim — never re-serialized, because byte fidelity is the
    /// thing several of these tests exist to check.
    pub body: bytes::Bytes,
}

impl Probe {
    async fn of(response: reqwest::Response) -> Result<Probe> {
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let body = response.bytes().await?;
        Ok(Probe {
            status,
            headers,
            body,
        })
    }

    /// A header as a string.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|v| v.to_str().ok())
    }

    /// The `Cache-Status` value.
    pub fn cache_status(&self) -> &str {
        self.header("cache-status").unwrap_or("<missing>")
    }

    /// True when Reactor reported a hit.
    pub fn is_hit(&self) -> bool {
        self.cache_status().contains("hit")
    }

    /// True when Reactor reported a miss.
    pub fn is_miss(&self) -> bool {
        self.cache_status().contains("miss")
    }

    /// True when Reactor reported a bypass.
    pub fn is_bypass(&self) -> bool {
        self.cache_status().contains("bypass")
    }

    /// The body as text.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }

    /// The body parsed as JSON, for assertions about content rather than bytes.
    pub fn json(&self) -> Result<serde_json::Value> {
        Ok(serde_json::from_slice(&self.body)?)
    }
}

/// Poll `check` until it is true, or give up.
pub async fn eventually<F>(budget: Duration, mut check: F) -> bool
where
    F: FnMut() -> bool,
{
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        if check() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    false
}

/// Track several branches of the same database.
///
/// The engine is partitioned by `Namespace { org, db, branch }`, so each branch
/// gets its own writer, watermark and term index. A test that wants to show
/// branches are independent has to have Reactor tracking both.
pub fn with_branches<'a>(branches: &'a [&'a str]) -> impl FnOnce(&mut Config) + 'a {
    move |config: &mut Config| {
        let template = config.namespaces[0].clone();
        config.namespaces = branches
            .iter()
            .map(|branch| NamespaceConfig {
                branch: (*branch).to_owned(),
                ..template.clone()
            })
            .collect();
    }
}

/// A driver override, for the per-driver capture tests.
pub fn with_driver(driver: Driver) -> impl FnOnce(&mut Config) {
    move |config: &mut Config| {
        config.capture.driver = driver;
        for ns in &mut config.namespaces {
            ns.driver = Some(driver);
        }
    }
}

/// Models the integration tests insert.
///
/// Deliberately small, but between them they cover the shapes the cache has to
/// reason about: a plain document, an arbitrary-precision decimal (the byte
/// fidelity case), a subdocument tree, and a link between documents.
pub mod models {
    use terminusdb_schema::*;
    use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

    /// A plain document with a value property and an id.
    #[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
    #[tdb(id_field = "id")]
    pub struct Person {
        /// Stable id, so tests can address the same document repeatedly.
        pub id: EntityIDFor<Self>,
        /// A value property, changed by the invalidation tests.
        pub name: String,
        /// A second value property, changed to prove per-property precision.
        pub age: i32,
    }

    /// Build a `Person`.
    pub fn person(id: &str, name: &str, age: i32) -> anyhow::Result<Person> {
        Ok(Person {
            id: EntityIDFor::new(id)?,
            name: name.to_owned(),
            age,
        })
    }

    /// A second top-level class, so a test can commit to something unrelated.
    #[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
    #[tdb(id_field = "id")]
    pub struct Company {
        /// Stable id.
        pub id: EntityIDFor<Self>,
        /// A value property.
        pub name: String,
    }

    /// Build a `Company`.
    pub fn company(id: &str, name: &str) -> anyhow::Result<Company> {
        Ok(Company {
            id: EntityIDFor::new(id)?,
            name: name.to_owned(),
        })
    }

    /// An arbitrary-precision decimal, for the byte-fidelity test.
    ///
    /// `xsd:decimal` is where a careless `serde_json::Value` round trip shows
    /// up: f64 has 53 bits of mantissa, and the value these tests use needs
    /// far more.
    #[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
    #[tdb(id_field = "id")]
    pub struct Ledger {
        /// Stable id.
        pub id: EntityIDFor<Self>,
        /// A high-precision amount.
        pub amount: BigDecimal,
    }

    /// Build a `Ledger`.
    pub fn ledger(id: &str, amount: &str) -> anyhow::Result<Ledger> {
        use std::str::FromStr;
        Ok(Ledger {
            id: EntityIDFor::new(id)?,
            amount: BigDecimal::from_str(amount)?,
        })
    }

    /// A subdocument: owned by its parent, no independent identity.
    #[derive(Debug, Clone, Default, PartialEq, TerminusDBModel, FromTDBInstance)]
    #[tdb(subdocument = true, key = "random")]
    pub struct Address {
        /// The only field, so a change to it is unambiguous.
        pub city: String,
    }

    /// A document that owns a subdocument.
    #[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
    #[tdb(id_field = "id")]
    pub struct Resident {
        /// Stable id.
        pub id: EntityIDFor<Self>,
        /// A value property.
        pub name: String,
        /// The owned subdocument.
        #[tdb(subdocument = true)]
        pub address: Address,
    }

    /// Build a `Resident`.
    pub fn resident(id: &str, name: &str, city: &str) -> anyhow::Result<Resident> {
        Ok(Resident {
            id: EntityIDFor::new(id)?,
            name: name.to_owned(),
            address: Address {
                city: city.to_owned(),
            },
        })
    }
}
