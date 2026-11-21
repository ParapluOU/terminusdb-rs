//! TOML configuration and CLI overrides.
//!
//! `reactor.example.toml` is the operator-facing contract: the section names and
//! key names here mirror it exactly, and every default is chosen so that a
//! Reactor started with **no configuration file at all** is still safe — it
//! simply caches nothing, because no namespace is listed and an unlisted
//! namespace is proxied and never cached.
//!
//! Validation happens once, at load, and fails loudly. Startup is the one place
//! in this crate where panicking is acceptable: a misconfigured cache that
//! starts and then misbehaves is far worse than one that refuses to start.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

use crate::analyze::AnalyzeConfig;
use crate::engine::{Decomposition, EngineConfig, EvalLimits, Policy, PolicySet, Refresh};
use crate::ids::Namespace;

/// The whole configuration.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    /// This Reactor's identity and listen address.
    pub node: NodeConfig,
    /// The TerminusDB node we front.
    pub upstream: UpstreamConfig,
    /// Fragment store sizing and tiers.
    pub cache: CacheConfig,
    /// Consistency knobs: fences and staleness.
    pub consistency: ConsistencyConfig,
    /// How commits are noticed.
    pub capture: CaptureConfig,
    /// Namespaces to track. Anything not listed is proxied, never cached.
    #[serde(rename = "namespace")]
    pub namespaces: Vec<NamespaceConfig>,
    /// Analyzer behaviour.
    pub analyze: AnalyzeSection,
    /// Filter-predicate indexing bounds.
    pub predicates: PredicateSection,
    /// Logging and online auditing.
    pub observability: ObservabilityConfig,
}

/// `[node]`
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct NodeConfig {
    /// Identity reported in `X-Reactor-Node` and in metrics. Defaults to the
    /// hostname, because the first question about a stale answer is always
    /// "which node served it".
    pub id: String,
    /// Where this Reactor listens. The load balancer points here, not at
    /// TerminusDB.
    pub listen: String,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            id: default_node_id(),
            listen: "0.0.0.0:6364".to_owned(),
        }
    }
}

/// Best-effort hostname, with a stable fallback.
fn default_node_id() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|h| !h.trim().is_empty())
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|h| h.trim().to_owned())
                .filter(|h| !h.is_empty())
        })
        .unwrap_or_else(|| "reactor".to_owned())
}

/// `[upstream]`
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct UpstreamConfig {
    /// The local TerminusDB node.
    pub url: String,
    /// Credentials for Reactor's **own** requests — capture, fills, schema
    /// reads. Client requests are forwarded with the client's own
    /// `Authorization`; Reactor never substitutes these for a client's.
    pub user: String,
    /// Password for the above.
    pub pass: String,
    /// Organization segment of upstream paths.
    pub org: String,
    /// Cap on concurrent fills, so a commit that dirties thousands of fragments
    /// cannot stampede the node it is supposed to protect.
    pub max_inflight_fills: usize,
    /// Timeout for Reactor's own upstream requests.
    pub request_timeout_ms: u64,
}

impl Default for UpstreamConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:6363".to_owned(),
            user: "admin".to_owned(),
            pass: "root".to_owned(),
            org: "admin".to_owned(),
            max_inflight_fills: 32,
            request_timeout_ms: 30_000,
        }
    }
}

impl UpstreamConfig {
    /// The request timeout as a `Duration`.
    pub fn request_timeout(&self) -> Duration {
        Duration::from_millis(self.request_timeout_ms)
    }
}

/// `[cache]`
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct CacheConfig {
    /// Hot in-memory tier budget, in MiB.
    pub memory_budget_mb: u64,
    /// Warm tier directory. Fragments are immutable and content-addressed, so
    /// pointing several nodes at one NAS directory makes this the shared tier
    /// of design doc §5.7 mode 2 — with no coherence protocol, because nothing
    /// is ever updated in place. `None` is a private in-memory cache (mode 1).
    pub dir: Option<PathBuf>,
    /// Versions retained per fragment key.
    pub max_versions_per_key: usize,
    /// Fragment keys tracked per namespace, as a count.
    ///
    /// The bound nothing else provides: `memory_budget_mb` bounds cached
    /// *bodies* and `max_versions_per_key` bounds versions *within* a key, but
    /// neither bounds how many distinct keys exist — and since a document list
    /// is cached as one fragment per document, that grows with the number of
    /// documents read, not the number of requests. Over the cap the
    /// least-recently-used keys are dropped (slot and term index together); the
    /// bodies stay in the content-addressed store, so an eviction costs a
    /// refill and nothing else.
    ///
    /// A count rather than a byte budget because it is the number an operator
    /// can reason about: it is the size of the working set you are promising to
    /// keep warm. Reckon 1-2 KiB of metadata per key plus any inlined body.
    pub max_tracked_keys: usize,
    /// Responses larger than this are streamed through and never cached.
    ///
    /// Not in the example file: a buffer limit is a safety property rather than
    /// a tuning knob, but it has to be settable because "how big is a big
    /// response" is a property of the data, not of Reactor.
    pub max_cacheable_body_mb: u64,
    /// Cache document lists as a spine over per-document fragments, so a field
    /// change to one member does not throw the whole list away.
    ///
    /// On by default, because the list read is the hot path and whole-response
    /// caching makes it nearly useless. Off is the **escape hatch**: it falls
    /// back to caching each response as one blob, which is unconditionally
    /// byte-faithful. Turn it off if composition ever returns bytes that differ
    /// from the origin's, and say so, because that is a bug and not a tuning
    /// problem.
    pub decompose: bool,
    /// Discard a cached version this many seconds after it was filled, whatever
    /// its dependencies say. `0` means never, which is the default and the
    /// intended operating point.
    ///
    /// Term invalidation is primary and a TTL cannot make a wrong dependency
    /// right: between a write and the commit landing in capture, a stale entry
    /// is stale for the whole TTL just as it would be forever. What a TTL
    /// *does* bound is the damage from a dependency we got wrong — a WOQL
    /// `X-Reactor-Invalidate-On` condition that names too few classes, or a
    /// term that cannot fire — which is the one hole a caller can punch in this
    /// design. So it is a seatbelt for the opt-in surface, not a substitute for
    /// invalidation on the surfaces Reactor proves for itself.
    pub ttl_secs: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            memory_budget_mb: 512,
            dir: None,
            max_versions_per_key: 4,
            max_tracked_keys: 50_000,
            max_cacheable_body_mb: 8,
            decompose: true,
            ttl_secs: 0,
        }
    }
}

/// `[predicates]`
///
/// A GraphQL filter that is more than a single `eq` is indexed as a
/// *predicate*: the writer decides, per commit, whether each change moved a
/// document into or out of the set, from the whole before and after document.
/// That is exact, and it is the only way a filtered read survives an unrelated
/// write — but it is CPU proportional to the live predicates on the classes a
/// commit touches, and the commit path **waits** for it, because the watermark
/// must not advance past invalidations we have not applied.
///
/// These are therefore latency bounds dressed as precision bounds. Over either
/// one, the commit invalidates every predicate on the classes it touched
/// instead of deciding each one — coarse, bounded, and counted as
/// `reactor_predicate_evaluations_skipped`. Watch that counter together with
/// `reactor_watermark_lag_ms`: backpressure here must degrade precision, never
/// latency, because a stalled watermark stalls every read.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct PredicateSection {
    /// `(change, predicate)` pairs one commit may evaluate.
    pub max_evaluations_per_commit: usize,
    /// Distinct predicates one namespace may index.
    pub max_predicates: usize,
    /// Threads in the shared evaluation pool. `0` uses rayon's default, which
    /// is the number of cores.
    pub workers: usize,
}

impl Default for PredicateSection {
    fn default() -> Self {
        let defaults = EvalLimits::default();
        Self {
            max_evaluations_per_commit: defaults.max_evaluations,
            max_predicates: defaults.max_predicates,
            workers: defaults.workers,
        }
    }
}

/// What to do when a client demands a version this node has not reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum MinVersionPolicy {
    /// Block up to `min_version_wait_ms`, then fall through to the origin.
    #[default]
    Wait,
    /// Answer `412 Precondition Failed` so the load balancer retries elsewhere.
    /// The right setting for read-after-write paths.
    Fail,
    /// Answer anyway, labelled via `Cache-Status` and `Age`.
    ServeStale,
}

/// `[consistency]`
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ConsistencyConfig {
    /// Policy for an unsatisfied `X-Reactor-Min-Version`.
    pub min_version_policy: MinVersionPolicy,
    /// How long `wait` waits.
    pub min_version_wait_ms: u64,
    /// Hold a write's response until our watermark reaches the commit it
    /// produced, so a later read on **this** node is read-your-writes. `0`
    /// disables it.
    pub write_fence_ms: u64,
    /// How long a successful authorization probe is reused.
    ///
    /// Not in the example file. Only *successes* are memoized, so this bounds
    /// how long a revoked capability can still read the cache; a freshly
    /// granted one is never delayed. 30s is the documented tradeoff.
    pub auth_ttl_secs: u64,
}

impl Default for ConsistencyConfig {
    fn default() -> Self {
        Self {
            min_version_policy: MinVersionPolicy::Wait,
            min_version_wait_ms: 500,
            write_fence_ms: 2000,
            auth_ttl_secs: 30,
        }
    }
}

/// How commits are noticed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Driver {
    /// Head-poll `/api/log`. The only driver that works on a shared-disk reader
    /// node, where nothing commits locally and the subscription is silent.
    Poll,
    /// `_ChangeSet` over SSE. Low latency; needs a local commit path.
    Subscription,
    /// Subscription for latency, poll as the liveness backstop. The default,
    /// and the right answer when unsure.
    #[default]
    Both,
}

impl Driver {
    /// True when this driver polls the head.
    pub fn polls(self) -> bool {
        matches!(self, Driver::Poll | Driver::Both)
    }

    /// True when this driver subscribes.
    pub fn subscribes(self) -> bool {
        matches!(self, Driver::Subscription | Driver::Both)
    }
}

/// `[capture]`
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct CaptureConfig {
    /// Default driver for every namespace.
    pub driver: Driver,
    /// Driver for namespaces registered at run time, rather than configured.
    ///
    /// Separate from [`Self::driver`], and defaulting to `poll`, because of how
    /// the server's subscriptions are keyed: a `_ChangeSet` cohort is
    /// `cohort(Descriptor, Class, Operation, Selection, Mode)`, so a
    /// subscription covers **one database** and there is no org-wide stream to
    /// attach to. An application with a database per tenant would therefore
    /// want one SSE connection per tenant, against a server that caps them at
    /// **20 per user** by default (`SubscriptionLimiter::max_per_user` in
    /// upstream's `terminusdb-webserver`, raisable with
    /// `TERMINUSDB_SSE_MAX_SUBSCRIPTIONS_PER_USER`). Reactor authenticates as
    /// one upstream user, so every namespace it subscribes to draws on the same
    /// bucket — and a slot is **never returned** once its client goes away, so
    /// the cap is a lifetime budget and every resubscribe spends one of it
    /// (reproduction: terminusdb/terminusdb#2673). Raising the cap buys a
    /// bigger one-off allowance, not headroom, so the many-namespace case
    /// polls and only the handful named in a file subscribe.
    pub dynamic_driver: Driver,
    /// Register a namespace the first time it is read, rather than requiring
    /// `POST /_reactor/namespace`.
    ///
    /// Off by default, and deliberately: invariant 6 says an unknown route is
    /// forwarded rather than cached, and "track every database anybody
    /// mentions" inverts that default. Turn it on for an application that
    /// creates databases (a room or publication per tenant) and would otherwise
    /// have to register each one; leave it off if the set of databases is known,
    /// because then an unexpected namespace is a bug and should not quietly
    /// acquire a capture task.
    ///
    /// The first read of a namespace is never a hit either way: registration
    /// establishes a watermark, and until that lands there is no position to
    /// pin a fill at.
    pub auto_register: bool,
    /// Bound on how many namespaces may be tracked at once.
    ///
    /// Each one costs a capture task, a writer and a schema index, so this is
    /// the knob that keeps a per-tenant layout from turning into a thousand
    /// poll loops. Over the cap, registration is **refused** rather than
    /// evicting a namespace someone is using: a refused registration means that
    /// database is proxied uncached, which is a performance loss, where
    /// evicting one that is in use would be a correctness risk.
    pub max_tracked_namespaces: usize,
    /// Head-poll interval. On a reader node this *is* the staleness bound.
    pub poll_interval_ms: u64,
    /// Head-poll interval for a namespace nothing has read lately.
    ///
    /// With a database per tenant, most of them are idle most of the time, and
    /// polling a thousand heads every 150ms to notice a commit nobody is
    /// waiting for is the whole cost of the layout. Staleness in a namespace
    /// nobody reads costs nothing — invariant 4 means a lagging watermark
    /// serves an older snapshot rather than a wrong one — and the first read
    /// after a quiet spell is a miss that goes to the origin anyway.
    pub idle_poll_interval_ms: u64,
    /// How long without a read before a namespace counts as idle.
    pub idle_after_ms: u64,
    /// Above this many changed documents in one commit, stop paying for
    /// per-document reads and fire coarse terms instead. Over-invalidation is a
    /// performance bug; under-invalidation is a correctness bug.
    pub diff_threshold: usize,
    /// Walk at most this many commits to recover a gap before treating it as a
    /// non-fast-forward.
    pub max_gap_walk: usize,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            driver: Driver::Both,
            dynamic_driver: Driver::Poll,
            auto_register: false,
            max_tracked_namespaces: 512,
            poll_interval_ms: 150,
            idle_poll_interval_ms: 5_000,
            idle_after_ms: 30_000,
            diff_threshold: 2000,
            max_gap_walk: 500,
        }
    }
}

/// Per-namespace recomputation policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum NamespacePolicy {
    /// Fill on demand.
    #[default]
    Lazy,
    /// Recompute dirtied fragments as soon as a commit lands.
    ///
    /// **Not implemented**; [`Config::validate`] rejects it. The variant and its
    /// mapping to [`Refresh::Eager`] are kept because the engine-side policy
    /// plumbing is already in place and the only missing piece is the refill
    /// queue — but accepting the value today would promise refills nobody does.
    Eager,
}

/// `[[namespace]]`
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamespaceConfig {
    /// Organization.
    pub org: String,
    /// Database.
    pub db: String,
    /// Branch.
    #[serde(default = "default_branch")]
    pub branch: String,
    /// Per-namespace override of `[capture].driver`, e.g. `poll` on a reader
    /// node.
    #[serde(default)]
    pub driver: Option<Driver>,
    /// Lazy or eager recomputation.
    #[serde(default)]
    pub policy: NamespacePolicy,
}

fn default_branch() -> String {
    "main".to_owned()
}

impl NamespaceConfig {
    /// The namespace this entry names.
    pub fn namespace(&self) -> Namespace {
        Namespace::new(self.org.clone(), self.db.clone(), self.branch.clone())
    }
}

/// `[analyze]`
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct AnalyzeSection {
    /// Bypass instead of falling back to a coarse dependency, trading hit rate
    /// for the smallest possible blast radius. Recommended while rolling out.
    pub strict: bool,
    /// Cache GraphQL query responses.
    pub graphql: bool,
    /// Cache read-only WOQL.
    pub woql: bool,
    /// Cache commit-pinned requests. The zero-risk case; there is no reason to
    /// turn it off.
    pub pinned: bool,
}

impl Default for AnalyzeSection {
    fn default() -> Self {
        Self {
            strict: false,
            graphql: true,
            woql: true,
            pinned: true,
        }
    }
}

/// `[observability]`
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ObservabilityConfig {
    /// Serve from cache *and* re-execute against the local node at the same
    /// pinned commit, comparing bytes. `0.0`–`1.0`.
    ///
    /// **Not implemented**; [`Config::validate`] rejects anything but `0.0`. The
    /// design is §7.4 of the design doc: the shadow read must be *pinned at the
    /// commit the entry claims validity at*, which is what turns a byte
    /// difference into proof of a bug rather than a race with the origin.
    pub shadow_sample_rate: f64,
    /// `tracing` filter directive.
    pub log_level: String,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            shadow_sample_rate: 0.0,
            log_level: "info".to_owned(),
        }
    }
}

impl Config {
    /// Parse TOML text.
    pub fn from_toml(text: &str) -> anyhow::Result<Config> {
        let config: Config = toml::from_str(text).context("parsing reactor configuration")?;
        config.validate()?;
        Ok(config)
    }

    /// Load from a path.
    ///
    /// A **missing** file is not an error: we warn and run on defaults, which
    /// track no namespace and therefore cache nothing. A file that exists but
    /// does not parse or does not validate *is* an error, because it means the
    /// operator intended something we cannot honour.
    pub fn load(path: &Path) -> anyhow::Result<Config> {
        match std::fs::read_to_string(path) {
            Ok(text) => Config::from_toml(&text),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!(
                    path = %path.display(),
                    "no configuration file; running on defaults, which track no \
                     namespace and therefore cache nothing"
                );
                Ok(Config::default())
            }
            Err(err) => {
                Err(err).with_context(|| format!("reading configuration {}", path.display()))
            }
        }
    }

    /// Check everything we can check before serving a request.
    pub fn validate(&self) -> anyhow::Result<()> {
        self.listen_addr()?;
        url::Url::parse(&self.upstream.url)
            .with_context(|| format!("[upstream].url is not a URL: {}", self.upstream.url))?;

        if self.upstream.max_inflight_fills == 0 {
            bail!("[upstream].max_inflight_fills must be at least 1");
        }
        if self.cache.max_versions_per_key == 0 {
            bail!("[cache].max_versions_per_key must be at least 1");
        }
        if self.predicates.max_evaluations_per_commit == 0 {
            bail!(
                "[predicates].max_evaluations_per_commit must be at least 1 — 0 would make \
                 every filtered read coarse, which is what this feature exists to avoid"
            );
        }
        if self.cache.max_tracked_keys == 0 {
            bail!("[cache].max_tracked_keys must be at least 1");
        }
        if self.capture.poll_interval_ms == 0 {
            bail!("[capture].poll_interval_ms must be at least 1");
        }
        if self.capture.max_gap_walk == 0 {
            bail!("[capture].max_gap_walk must be at least 1");
        }
        if self.capture.idle_poll_interval_ms < self.capture.poll_interval_ms {
            bail!(
                "[capture].idle_poll_interval_ms ({}) must be at least \
                 [capture].poll_interval_ms ({}) — an idle namespace is polled *less* often, \
                 not more",
                self.capture.idle_poll_interval_ms,
                self.capture.poll_interval_ms
            );
        }
        if self.capture.max_tracked_namespaces == 0 {
            bail!(
                "[capture].max_tracked_namespaces must be at least 1 — 0 would track nothing, \
                 which is what leaving [[namespace]] empty already does"
            );
        }
        if !(0.0..=1.0).contains(&self.observability.shadow_sample_rate) {
            bail!(
                "[observability].shadow_sample_rate must be between 0.0 and 1.0, got {}",
                self.observability.shadow_sample_rate
            );
        }
        // Both of the following are *designed* and not *built*. A knob that
        // parses and then does nothing is the exact failure mode this engine
        // exists to avoid — an operator who sets it believes they switched on an
        // audit, or eager refills, and nothing tells them otherwise. So the
        // config refuses the value instead of accepting it quietly.
        if self.observability.shadow_sample_rate != 0.0 {
            bail!(
                "[observability].shadow_sample_rate is not implemented yet: nothing in the serve \
                 path reads it, so any value above 0.0 would promise an audit that never runs. \
                 Leave it at 0.0 — see README 'Known gaps'."
            );
        }
        if let Some(ns) = self
            .namespaces
            .iter()
            .find(|n| n.policy == NamespacePolicy::Eager)
        {
            bail!(
                "[[namespace]] policy = \"eager\" is not implemented yet ({}/{}/{}): invalidation \
                 drops an entry and the next reader refills it. Use \"lazy\" — see README \
                 'Known gaps'.",
                ns.org,
                ns.db,
                ns.branch
            );
        }

        // A namespace listed twice would get two writers, and the single-writer
        // invariant is the whole basis of the engine's ordering guarantees.
        let mut seen = std::collections::HashSet::new();
        for entry in &self.namespaces {
            let ns = entry.namespace();
            if !seen.insert(ns.clone()) {
                bail!("namespace {ns} is listed more than once");
            }
            if entry.org.is_empty() || entry.db.is_empty() || entry.branch.is_empty() {
                bail!("namespace {ns} has an empty org, db or branch");
            }
        }

        if let Some(dir) = &self.cache.dir {
            check_writable(dir)
                .with_context(|| format!("[cache].dir is not usable: {}", dir.display()))?;
        }
        Ok(())
    }

    /// The parsed listen address.
    pub fn listen_addr(&self) -> anyhow::Result<SocketAddr> {
        self.node
            .listen
            .parse()
            .with_context(|| format!("[node].listen is not an address: {}", self.node.listen))
    }

    /// Every tracked namespace.
    pub fn namespaces(&self) -> Vec<Namespace> {
        self.namespaces.iter().map(|n| n.namespace()).collect()
    }

    /// The entry for a namespace, if it is tracked.
    pub fn namespace_entry(&self, ns: &Namespace) -> Option<&NamespaceConfig> {
        self.namespaces.iter().find(|n| &n.namespace() == ns)
    }

    /// Whether a namespace is tracked at all. An untracked namespace is proxied
    /// and never cached — the safe default for an unknown database.
    pub fn tracks(&self, ns: &Namespace) -> bool {
        self.namespace_entry(ns).is_some()
    }

    /// The driver for a namespace, honouring its override.
    ///
    /// A namespace with no `[[namespace]]` entry was registered at run time, and
    /// gets [`CaptureConfig::dynamic_driver`] — see there for why that is `poll`
    /// by default and not `[capture].driver`.
    pub fn driver_for(&self, ns: &Namespace) -> Driver {
        match self.namespace_entry(ns) {
            Some(entry) => entry.driver.unwrap_or(self.capture.driver),
            None => self.capture.dynamic_driver,
        }
    }

    /// Engine tuning derived from this configuration.
    pub fn engine_config(&self) -> EngineConfig {
        let refresh = if self
            .namespaces
            .iter()
            .any(|n| n.policy == NamespacePolicy::Eager)
        {
            Refresh::Eager
        } else {
            Refresh::Lazy
        };
        EngineConfig {
            policy: PolicySet::new(Policy {
                refresh,
                max_body_bytes: self.cache.max_cacheable_body_mb * 1024 * 1024,
                decompose: if self.cache.decompose {
                    Decomposition::NdjsonList
                } else {
                    Decomposition::Whole
                },
                // `Policy::fresh_enough` has always honoured this; until now
                // nothing could set it, so the mechanism was unreachable.
                ttl: (self.cache.ttl_secs > 0).then(|| Duration::from_secs(self.cache.ttl_secs)),
                ..Policy::default()
            }),
            max_versions_per_key: self.cache.max_versions_per_key,
            max_tracked_keys: self.cache.max_tracked_keys,
            predicates: EvalLimits {
                max_evaluations: self.predicates.max_evaluations_per_commit,
                max_predicates: self.predicates.max_predicates,
                workers: self.predicates.workers,
                // Fault injection only; never configurable from a file.
                delay_ms: 0,
            },
            ..EngineConfig::default()
        }
    }

    /// Analyzer tuning derived from this configuration.
    pub fn analyze_config(&self) -> AnalyzeConfig {
        AnalyzeConfig {
            strict: self.analyze.strict,
            max_body_bytes: (self.cache.max_cacheable_body_mb * 1024 * 1024) as usize,
        }
    }

    /// The largest response body we are willing to buffer in order to cache it.
    pub fn max_cacheable_body_bytes(&self) -> usize {
        (self.cache.max_cacheable_body_mb * 1024 * 1024) as usize
    }
}

/// Confirm a directory exists (creating it if need be) and that we can write to
/// it, by actually writing — permissions metadata lies often enough, especially
/// on a NAS mount, which is exactly where this tier is meant to live.
fn check_writable(dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir).context("creating the directory")?;
    let probe = dir.join(".reactor-write-probe");
    std::fs::write(&probe, b"ok").context("writing a probe file")?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

/// Command-line interface.
#[derive(Clone, Debug, clap::Parser)]
#[command(
    name = "tdb-reactor",
    about = "Reactive cache and reverse proxy for TerminusDB 12"
)]
pub struct Cli {
    /// Path to the TOML configuration file.
    #[arg(long, short, env = "REACTOR_CONFIG", default_value = "reactor.toml")]
    pub config: PathBuf,
    /// Override `[node].listen`.
    #[arg(long)]
    pub listen: Option<String>,
    /// Override `[upstream].url`.
    #[arg(long)]
    pub upstream: Option<String>,
    /// Override `[observability].log_level`.
    #[arg(long)]
    pub log_level: Option<String>,
}

impl Cli {
    /// Load the configuration and apply the overrides.
    pub fn resolve(&self) -> anyhow::Result<Config> {
        let mut config = Config::load(&self.config)?;
        if let Some(listen) = &self.listen {
            config.node.listen = listen.clone();
        }
        if let Some(upstream) = &self.upstream {
            config.upstream.url = upstream.clone();
        }
        if let Some(level) = &self.log_level {
            config.observability.log_level = level.clone();
        }
        // Re-validate: an override can be just as wrong as a file.
        config.validate()?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The example file is the operator-facing contract. If this fails, the
    /// documentation and the parser have diverged.
    #[test]
    fn the_example_configuration_parses_and_validates() {
        let text = include_str!("../reactor.example.toml");
        let config = Config::from_toml(text).expect("the example must stay valid");

        assert_eq!(config.node.id, "tdb-reader-01");
        assert_eq!(config.node.listen, "0.0.0.0:6364");
        assert_eq!(config.upstream.url, "http://localhost:6363");
        assert_eq!(config.upstream.max_inflight_fills, 32);
        assert_eq!(config.cache.memory_budget_mb, 512);
        assert_eq!(config.cache.dir, None, "the warm tier is commented out");
        assert_eq!(config.cache.max_versions_per_key, 4);
        assert_eq!(
            config.consistency.min_version_policy,
            MinVersionPolicy::Wait
        );
        assert_eq!(config.consistency.write_fence_ms, 2000);
        assert_eq!(config.capture.driver, Driver::Both);
        assert_eq!(config.capture.poll_interval_ms, 150);
        assert_eq!(config.capture.diff_threshold, 2000);
        assert_eq!(config.capture.max_gap_walk, 500);
        assert_eq!(config.namespaces.len(), 1);
        assert_eq!(
            config.namespaces[0].namespace(),
            Namespace::new("admin", "people", "main")
        );
        assert_eq!(config.namespaces[0].policy, NamespacePolicy::Lazy);
        assert!(!config.analyze.strict);
        assert!(config.analyze.graphql && config.analyze.woql && config.analyze.pinned);
        assert_eq!(config.observability.log_level, "info");
    }

    #[test]
    fn an_empty_configuration_is_valid_and_caches_nothing() {
        let config = Config::from_toml("").expect("defaults must validate");
        assert!(config.namespaces().is_empty());
        assert!(!config.tracks(&Namespace::new("admin", "people", "main")));
        assert_eq!(config.consistency.auth_ttl_secs, 30);
        assert_eq!(config.cache.max_cacheable_body_mb, 8);
    }

    #[test]
    fn a_missing_file_falls_back_to_defaults() {
        let config = Config::load(Path::new("/nonexistent/reactor.toml")).unwrap();
        assert!(config.namespaces().is_empty());
    }

    #[test]
    fn an_unreadable_but_present_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reactor.toml");
        std::fs::write(&path, "this is not toml = = =").unwrap();
        assert!(Config::load(&path).is_err());
    }

    #[test]
    fn config_round_trips_through_toml() {
        let text = include_str!("../reactor.example.toml");
        let config = Config::from_toml(text).unwrap();
        let rendered = toml::to_string(&config).expect("must serialize");
        let reparsed = Config::from_toml(&rendered).expect("must re-parse");
        assert_eq!(reparsed.node.id, config.node.id);
        assert_eq!(reparsed.namespaces(), config.namespaces());
        assert_eq!(
            reparsed.capture.poll_interval_ms,
            config.capture.poll_interval_ms
        );
    }

    #[test]
    fn an_unknown_key_is_rejected_rather_than_ignored() {
        // A typo in a safety-relevant key must not be silently dropped.
        assert!(Config::from_toml("[consistency]\nwrite_fence_mss = 1").is_err());
        assert!(Config::from_toml("[nonsense]\nx = 1").is_err());
    }

    #[test]
    fn the_three_min_version_policies_parse() {
        for (text, expected) in [
            ("wait", MinVersionPolicy::Wait),
            ("fail", MinVersionPolicy::Fail),
            ("serve-stale", MinVersionPolicy::ServeStale),
        ] {
            let config =
                Config::from_toml(&format!("[consistency]\nmin_version_policy = \"{text}\""))
                    .unwrap();
            assert_eq!(config.consistency.min_version_policy, expected);
        }
        assert!(Config::from_toml("[consistency]\nmin_version_policy = \"maybe\"").is_err());
    }

    #[test]
    fn the_three_drivers_parse_and_report_their_duties() {
        assert!(Driver::Poll.polls() && !Driver::Poll.subscribes());
        assert!(Driver::Subscription.subscribes() && !Driver::Subscription.polls());
        assert!(Driver::Both.polls() && Driver::Both.subscribes());
        assert!(Config::from_toml("[capture]\ndriver = \"sometimes\"").is_err());
    }

    #[test]
    fn a_namespace_can_override_the_driver() {
        let config = Config::from_toml(
            r#"
            [capture]
            driver = "both"
            [[namespace]]
            org = "admin"
            db = "people"
            driver = "poll"
            [[namespace]]
            org = "admin"
            db = "other"
            "#,
        )
        .unwrap();
        assert_eq!(
            config.driver_for(&Namespace::new("admin", "people", "main")),
            Driver::Poll
        );
        assert_eq!(
            config.driver_for(&Namespace::new("admin", "other", "main")),
            Driver::Both
        );
        assert_eq!(
            config.driver_for(&Namespace::new("admin", "registered", "main")),
            Driver::Poll,
            "a namespace with no [[namespace]] entry was registered at run time, and gets \
             [capture].dynamic_driver — poll, because a _ChangeSet cohort covers one database \
             and the per-user SSE cap does not survive a database per tenant"
        );
    }

    #[test]
    fn a_duplicate_namespace_is_refused() {
        // Two writers for one namespace would break the ordering guarantee the
        // whole engine rests on.
        let text = r#"
            [[namespace]]
            org = "admin"
            db = "people"
            branch = "main"
            [[namespace]]
            org = "admin"
            db = "people"
            "#;
        let err = Config::from_toml(text).unwrap_err().to_string();
        assert!(err.contains("more than once"), "got {err}");
    }

    #[test]
    fn nonsense_numbers_are_refused() {
        for text in [
            "[upstream]\nmax_inflight_fills = 0",
            "[cache]\nmax_versions_per_key = 0",
            "[capture]\npoll_interval_ms = 0",
            "[capture]\nmax_gap_walk = 0",
            "[observability]\nshadow_sample_rate = 1.5",
            "[node]\nlisten = \"not an address\"",
            "[upstream]\nurl = \"not a url\"",
        ] {
            assert!(Config::from_toml(text).is_err(), "{text} should be refused");
        }
    }

    #[test]
    fn a_writable_warm_tier_directory_is_accepted_and_created() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("cas/deeper");
        let config = Config::from_toml(&format!("[cache]\ndir = \"{}\"", nested.display()))
            .expect("a creatable directory is fine");
        assert_eq!(config.cache.dir.as_deref(), Some(nested.as_path()));
        assert!(nested.is_dir(), "it must be created at validation time");
        assert!(
            !nested.join(".reactor-write-probe").exists(),
            "the probe file must be cleaned up"
        );
    }

    #[test]
    fn an_unwritable_warm_tier_directory_fails_at_startup() {
        // A file where a directory should be: not creatable, so not usable.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("iamafile");
        std::fs::write(&file, b"x").unwrap();
        assert!(Config::from_toml(&format!("[cache]\ndir = \"{}\"", file.display())).is_err());
    }

    #[test]
    fn derived_engine_and_analyze_config_follow_the_file() {
        let config = Config::from_toml(
            r#"
            [cache]
            max_versions_per_key = 9
            max_cacheable_body_mb = 2
            [analyze]
            strict = true
            [[namespace]]
            org = "admin"
            db = "people"
            "#,
        )
        .unwrap();
        let engine = config.engine_config();
        assert_eq!(engine.max_versions_per_key, 9);
        assert_eq!(
            engine.policy.default_policy().max_body_bytes,
            2 * 1024 * 1024
        );
        assert_eq!(engine.policy.default_policy().refresh, Refresh::Lazy);
        assert!(config.analyze_config().strict);
        assert_eq!(config.max_cacheable_body_bytes(), 2 * 1024 * 1024);
    }

    #[test]
    fn a_ttl_is_off_by_default_and_reaches_the_policy_when_set() {
        let unbounded = Config::default();
        assert!(
            unbounded
                .engine_config()
                .policy
                .default_policy()
                .ttl
                .is_none(),
            "term invalidation is primary; time is not"
        );

        let bounded = Config::from_toml("[cache]\nttl_secs = 30").unwrap();
        assert_eq!(
            bounded.engine_config().policy.default_policy().ttl,
            Some(Duration::from_secs(30))
        );
    }

    /// A knob that parses and then does nothing is worse than a missing knob:
    /// the operator believes they turned something on. Both of these are
    /// designed and unbuilt, so both are refused at load with the reason.
    #[test]
    fn unimplemented_knobs_are_refused_rather_than_ignored() {
        let eager = Config::from_toml(
            r#"
            [[namespace]]
            org = "admin"
            db = "people"
            policy = "eager"
            "#,
        )
        .expect_err("eager recomputation does not exist yet");
        assert!(
            eager.to_string().contains("not implemented"),
            "the error must say why, got: {eager}"
        );
        assert!(
            eager.to_string().contains("admin/people/main"),
            "and which namespace, got: {eager}"
        );

        let shadow = Config::from_toml("[observability]\nshadow_sample_rate = 1.0")
            .expect_err("shadow verification does not exist yet");
        assert!(
            shadow.to_string().contains("not implemented"),
            "the error must say why, got: {shadow}"
        );

        // The mapping itself still works, so the day the refill queue lands the
        // only change is deleting the guard.
        let mut cfg = Config::default();
        cfg.namespaces.push(NamespaceConfig {
            org: "admin".to_owned(),
            db: "people".to_owned(),
            branch: "main".to_owned(),
            driver: None,
            policy: NamespacePolicy::Eager,
        });
        assert_eq!(
            cfg.engine_config().policy.default_policy().refresh,
            Refresh::Eager
        );
    }

    #[test]
    fn cli_overrides_win_and_are_revalidated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reactor.toml");
        std::fs::write(&path, "[node]\nlisten = \"127.0.0.1:1\"").unwrap();

        let cli = Cli {
            config: path.clone(),
            listen: Some("127.0.0.1:9999".to_owned()),
            upstream: Some("http://localhost:7777".to_owned()),
            log_level: Some("debug".to_owned()),
        };
        let config = cli.resolve().unwrap();
        assert_eq!(config.node.listen, "127.0.0.1:9999");
        assert_eq!(config.upstream.url, "http://localhost:7777");
        assert_eq!(config.observability.log_level, "debug");

        let bad = Cli {
            config: path,
            listen: Some("nonsense".to_owned()),
            upstream: None,
            log_level: None,
        };
        assert!(bad.resolve().is_err(), "an override is validated too");
    }

    #[test]
    fn the_node_id_defaults_to_something_stable() {
        assert!(!default_node_id().is_empty());
    }
}
