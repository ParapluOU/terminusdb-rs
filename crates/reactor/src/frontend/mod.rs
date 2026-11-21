//! The HTTP front end: an axum reverse proxy with a cache in front of it.
//!
//! The router is deliberately almost empty. Reactor owns exactly two prefixes —
//! `/_reactor/` and `/healthz` — and *everything else* falls through to the
//! proxy handler. That shape is the routing expression of invariant 6: a path we
//! have never heard of is forwarded, never cached, and never 404'd by us.

pub mod admin;
pub mod auth;
pub mod fill;
pub mod proxy;
pub mod scope;
pub mod serve;
pub mod value;

use std::sync::Arc;
use std::time::Duration;

use axum::routing::{get, post};
use axum::Router;

use crate::analyze::AnalyzeConfig;
use crate::capture::CaptureHint;
use crate::config::Config;
use crate::engine::Engine;

pub use auth::{AlwaysAllow, AuthDecision, AuthGate, Gate};
pub use fill::{FillCoordinator, FillOutcome};
pub use proxy::{Proxy, UpstreamRequest, UpstreamResponse};
pub use scope::{Recorder, ScopeRecord};
pub use serve::{cache_status, fence_action, CacheState, FenceAction};

/// Everything a request handler needs.
///
/// Held in an `Arc` and cloned per request, so every field is either `Arc` or
/// cheap. Nothing in here is mutable: all mutation goes through the engine's
/// writer mailbox.
pub struct AppState {
    /// The effective configuration.
    pub config: Arc<Config>,
    /// Analyzer tuning, derived once from the configuration.
    pub analyze: AnalyzeConfig,
    /// The cache engine.
    pub engine: Arc<Engine>,
    /// The upstream HTTP client.
    pub proxy: Arc<Proxy>,
    /// Single-flight fill coordination.
    pub fills: Arc<FillCoordinator>,
    /// The authorization gate.
    pub auth: Arc<Gate>,
    /// Framings learned from the origin, so a single-document read whose child
    /// fragment is already cached needs no upstream request to know how the
    /// origin wraps it. See [`crate::compose::FramingCache`].
    pub framings: Arc<crate::compose::FramingCache>,
    /// Which namespaces this node tracks: seeded from the configuration and
    /// added to at run time, because an application that creates databases
    /// cannot list them in a file. See [`crate::registry`].
    pub registry: crate::registry::SharedRegistry,
    /// Open recording scopes: the dependency sets of application-level
    /// computations, observed as their reads go past. See [`scope`].
    pub recorder: Arc<scope::Recorder>,
    /// Nudges to capture: a write's commit, or a resync after a topology call.
    pub hints: tokio::sync::mpsc::UnboundedSender<CaptureHint>,
    /// A self-reference, so a detached fill can keep the state alive.
    ///
    /// A fill outlives the request that started it (that is the point — see
    /// [`fill`]), so it cannot borrow. This is initialized by
    /// [`AppState::into_shared`].
    this: std::sync::OnceLock<Arc<AppState>>,
}

impl AppState {
    /// Assemble the state.
    pub fn new(
        config: Arc<Config>,
        engine: Arc<Engine>,
        proxy: Arc<Proxy>,
        auth: Arc<Gate>,
        hints: tokio::sync::mpsc::UnboundedSender<CaptureHint>,
        registry: crate::registry::SharedRegistry,
    ) -> AppState {
        let analyze = config.analyze_config();
        let fills = Arc::new(FillCoordinator::new(config.upstream.max_inflight_fills));
        AppState {
            config,
            analyze,
            engine,
            proxy,
            fills,
            auth,
            framings: Arc::new(crate::compose::FramingCache::default()),
            registry,
            recorder: Arc::new(scope::Recorder::default()),
            hints,
            this: std::sync::OnceLock::new(),
        }
    }

    /// Wrap in an `Arc` and let the state reach itself.
    pub fn into_shared(self) -> Arc<AppState> {
        let shared = Arc::new(self);
        // `set` fails only if called twice, which cannot happen here.
        let _ = shared.this.set(Arc::clone(&shared));
        shared
    }

    /// An owned handle for a detached task.
    pub fn clone_handle(&self) -> Arc<AppState> {
        self.this
            .get()
            .cloned()
            .expect("AppState must be built with into_shared")
    }

    /// Send a capture hint, tolerating a shut-down capture pipeline.
    pub fn hint(&self, hint: CaptureHint) {
        if self.hints.send(hint).is_err() {
            tracing::debug!("capture pipeline is gone; hint dropped");
        }
    }
}

/// Build the router.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(admin::healthz))
        .route("/_reactor/head", get(admin::head))
        .route("/_reactor/metrics", get(admin::metrics))
        .route("/_reactor/config", get(admin::config))
        .route("/_reactor/fragment/{hash}", get(admin::fragment))
        .route("/_reactor/explain", post(admin::explain))
        .route("/_reactor/scope/{id}", get(admin::scope))
        // Namespaces can be registered while the process runs: an application
        // with a database per tenant cannot list them in a configuration file.
        .route(
            "/_reactor/namespace",
            get(admin::namespaces)
                .post(admin::track)
                .delete(admin::untrack),
        )
        .route("/_reactor/invalidate", post(admin::invalidate))
        // Application-computed values: the one entry point that puts something
        // into the graph that Reactor did not derive itself.
        .route(
            "/_reactor/value/{org}/{db}/{branch}",
            get(value::get).post(value::put),
        )
        // Everything else belongs to TerminusDB.
        .fallback(serve::handle)
        .with_state(state)
}

/// Build the upstream proxy from configuration.
pub fn build_proxy(config: &Config) -> anyhow::Result<Proxy> {
    let upstream = url::Url::parse(&config.upstream.url)?;
    Proxy::new(upstream, config.upstream.request_timeout())
}

/// Build the authorization gate from configuration.
pub fn build_gate(config: &Config) -> anyhow::Result<Gate> {
    let upstream = url::Url::parse(&config.upstream.url)?;
    let ttl = Duration::from_secs(config.consistency.auth_ttl_secs);
    // A zero TTL means "probe every time", which is still a probe — not an
    // open gate. Only an explicit choice could make it open, and there is
    // deliberately no configuration key for that.
    Ok(Gate::Probe(Arc::new(AuthGate::new(
        reqwest::Client::new(),
        upstream,
        ttl,
    ))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::EngineConfig;
    use crate::store::TieredStore;

    fn state() -> Arc<AppState> {
        let config = Arc::new(Config::default());
        let engine = Arc::new(Engine::new(
            EngineConfig::default(),
            Arc::new(TieredStore::memory_only(1 << 20)),
        ));
        let proxy = Arc::new(build_proxy(&config).unwrap());
        let gate = Arc::new(Gate::Open(AlwaysAllow));
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let registry = Arc::new(crate::registry::Registry::new(config.namespaces(), 16));
        AppState::new(config, engine, proxy, gate, tx, registry).into_shared()
    }

    #[test]
    fn the_state_can_reach_itself_for_detached_fills() {
        let state = state();
        let handle = state.clone_handle();
        assert!(Arc::ptr_eq(&state, &handle));
    }

    #[test]
    fn a_hint_with_no_capture_pipeline_is_dropped_quietly() {
        // The receiver is dropped immediately in `state()`, so this exercises
        // the shut-down path. A panic here would take down a request.
        let state = state();
        state.hint(CaptureHint::Resync {
            ns: crate::ids::Namespace::new("admin", "mydb", "main"),
        });
    }

    #[test]
    fn the_router_builds() {
        let _router = router(state());
    }

    #[test]
    fn the_gate_is_a_probe_by_default() {
        let config = Config::default();
        assert!(matches!(build_gate(&config).unwrap(), Gate::Probe(_)));
    }
}
