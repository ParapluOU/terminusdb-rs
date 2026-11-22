//! `/_reactor/*` and `/healthz`.
//!
//! Everything under `/_reactor/` and `/healthz` is Reactor's own; every other
//! path belongs to TerminusDB and is proxied. The prefix is deliberately one
//! TerminusDB does not use, so the namespaces cannot collide.
//!
//! These endpoints exist because the previous prototype had none, and "why is
//! this stale" was consequently unanswerable without attaching a debugger. A
//! cache you cannot interrogate online is a cache nobody should trust.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::analyze::{self, ReqView};
use crate::capture::CaptureHint;
use crate::engine::WriterMsg;
use crate::ids::{ContentHash, Namespace};
use terminusdb_client::CommitId;

use super::AppState;

/// `GET /healthz`
///
/// Liveness only: it says this process is up and able to answer, not that the
/// node behind it is healthy. A readiness probe that depended on upstream would
/// take Reactor out of the load balancer exactly when it is most useful (it can
/// still serve cached reads while the node is busy).
pub async fn healthz() -> &'static str {
    "ok\n"
}

/// One namespace's published state.
#[derive(Debug, Serialize)]
pub struct HeadReport {
    /// `org/db/local/branch/<branch>`.
    pub namespace: String,
    /// The commit the watermark is at, if one is established.
    pub commit: Option<String>,
    /// The Reactor-local position, as `epoch.seq`.
    pub position: Option<String>,
    /// Number of classes in the published schema index.
    pub classes: usize,
    /// Whether a schema refresh is pending.
    pub schema_refresh_pending: bool,
}

/// `?namespace=…&min_version=…` on `/_reactor/head`.
#[derive(Debug, Default, Deserialize)]
pub struct HeadParams {
    /// Report only this namespace, as `org/db/local/branch/<branch>`.
    ///
    /// Without it the answer lists every tracked namespace, which is the
    /// operator's view. A client that latches a commit before a computation
    /// wants exactly one, and with a database per room the full list is both
    /// large and almost entirely irrelevant to the caller.
    pub namespace: Option<String>,
    /// Do not answer with a watermark older than this commit.
    ///
    /// The latch-time half of `X-Reactor-Min-Version`: a read fence is no use
    /// to a computation that has already pinned itself at a stale commit, so
    /// the demand has to be satisfiable *before* the pin is chosen. Requires
    /// `namespace`, since the demand is about one history.
    pub min_version: Option<String>,
}

/// `GET /_reactor/head`
///
/// The honest answer to "how far behind is this node", which is what a smart
/// client or load balancer needs in order to route by replication state instead
/// of guessing.
///
/// Reports what is **tracked**, not what is configured: a namespace adopted at
/// run time under `[capture].auto_register` is as real as one in the file, and
/// omitting it told a client that its database could not be cached when it
/// could.
pub async fn head(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HeadParams>,
) -> Response {
    let Some(wanted) = params.namespace else {
        if params.min_version.is_some() {
            return (
                StatusCode::BAD_REQUEST,
                "min_version is a demand about one history: name the namespace too\n",
            )
                .into_response();
        }
        let mut namespaces = state.registry.namespaces();
        namespaces.sort_by_key(Namespace::path);
        let out: Vec<HeadReport> = namespaces.iter().map(|ns| report(&state, ns)).collect();
        return Json(out).into_response();
    };

    let Some(ns) = Namespace::from_path(&wanted) else {
        return (
            StatusCode::BAD_REQUEST,
            format!("`{wanted}` is not a namespace path (org/db/local/branch/<branch>)\n"),
        )
            .into_response();
    };
    if !state.registry.tracks(&ns) {
        return (
            StatusCode::NOT_FOUND,
            format!(
                "{ns} is not tracked, so nothing derived from it can be cached. Register it \
                 with POST /_reactor/namespace, add a [[namespace]] entry, or turn on \
                 [capture].auto_register and read from it once.\n"
            ),
        )
            .into_response();
    }

    if let Some(demanded) = params.min_version {
        let demanded = CommitId::new(demanded);
        let policy = state.config.consistency.min_version_policy;
        let budget = Duration::from_millis(state.config.consistency.min_version_wait_ms);
        let deadline = Instant::now() + budget;
        loop {
            let latched = state.engine.watermark(&ns).map(|w| w.pos);
            let action = match latched {
                Some(pos) => super::serve::fence_action(
                    policy,
                    state.engine.position_of(&ns, &demanded),
                    pos,
                ),
                // No watermark yet: there is no position to compare against,
                // and no pin to hand out either.
                None => super::serve::FenceAction::Wait,
            };
            match action {
                super::serve::FenceAction::Proceed => break,
                super::serve::FenceAction::Fail => {
                    return (
                        StatusCode::PRECONDITION_FAILED,
                        format!("{ns} has not reached {demanded}\n"),
                    )
                        .into_response()
                }
                // `ServeStale` answers with what we have, which at latch time
                // means handing out an older pin on purpose.
                super::serve::FenceAction::ServeStale => break,
                super::serve::FenceAction::Wait => {
                    if Instant::now() >= deadline {
                        // Out of budget. Say so rather than quietly handing back
                        // a stale pin: unlike a read, a latch cannot fall
                        // through to the origin.
                        return (
                            StatusCode::PRECONDITION_FAILED,
                            format!(
                                "{ns} did not reach {demanded} within \
                                 {}ms\n",
                                budget.as_millis()
                            ),
                        )
                            .into_response();
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            }
        }
    }

    Json(vec![report(&state, &ns)]).into_response()
}

/// One namespace's line in the head report.
fn report(state: &AppState, ns: &Namespace) -> HeadReport {
    let watermark = state.engine.watermark(ns);
    HeadReport {
        namespace: ns.path(),
        commit: watermark.as_ref().map(|w| w.commit.as_str().to_owned()),
        position: watermark.as_ref().map(|w| w.pos.to_string()),
        classes: state.engine.schema(ns).classes().count(),
        schema_refresh_pending: state.engine.schema_refresh_requested(ns),
    }
}

/// `GET /_reactor/metrics`
///
/// Prometheus text format, hand-rolled in [`crate::metrics`] rather than
/// pulling in an exporter for a dozen counters.
pub async fn metrics(State(state): State<Arc<AppState>>) -> Response {
    let body = state.engine.metrics().encode_prometheus();
    (
        StatusCode::OK,
        [(
            http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
        .into_response()
}

/// `GET /_reactor/config`
///
/// The effective configuration, so an operator can confirm what was actually
/// loaded. Credentials are redacted: this endpoint is reachable by anyone who
/// can reach the proxy.
pub async fn config(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut config = state.config.as_ref().clone();
    config.upstream.pass = "<redacted>".to_owned();
    Json(serde_json::to_value(&config).unwrap_or(serde_json::Value::Null))
}

/// A fragment, as the admin endpoint reports it.
#[derive(Debug, Serialize)]
pub struct FragmentReport {
    /// The content hash.
    pub hash: String,
    /// Body length in bytes.
    pub bytes: usize,
    /// The body, when it is valid UTF-8 — which for TerminusDB it always is.
    /// Reported as text rather than re-serialized JSON so that what you see is
    /// byte-for-byte what a client would get.
    pub body: Option<String>,
}

/// `GET /_reactor/fragment/{hash}`
///
/// Looks a body up by content hash. Keyed by hash rather than by cache key
/// because a hash is what appears in an `ETag`, in a log line and in a
/// divergence report — so it is what you have in your hand when you need this.
pub async fn fragment(State(state): State<Arc<AppState>>, Path(hash): Path<String>) -> Response {
    let Some(hash) = ContentHash::from_hex(hash.trim_matches('"')) else {
        return (StatusCode::BAD_REQUEST, "not a 64-character hex digest").into_response();
    };
    match state.engine.store().get(&hash) {
        Some(body) => Json(FragmentReport {
            hash: hash.to_hex(),
            bytes: body.len(),
            body: String::from_utf8(body.to_vec()).ok(),
        })
        .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            "no such fragment body (it may have been evicted, or held inline)",
        )
            .into_response(),
    }
}

/// The request envelope for `/_reactor/explain`.
#[derive(Debug, Deserialize)]
pub struct ExplainRequest {
    /// HTTP method. Defaults to `GET`.
    #[serde(default = "default_method")]
    pub method: String,
    /// Request path, e.g. `/api/document/admin/people`.
    pub path: String,
    /// Query string without the leading `?`.
    #[serde(default)]
    pub query: String,
    /// Request body, as text.
    #[serde(default)]
    pub body: String,
}

fn default_method() -> String {
    "GET".to_owned()
}

/// What `/_reactor/explain` answers.
#[derive(Debug, Serialize)]
pub struct ExplainReport {
    /// How the path was classified.
    pub target: analyze::Target,
    /// Whether this namespace is configured at all.
    pub tracked: bool,
    /// The published watermark, if any.
    pub head: Option<String>,
    /// The plan, including its dependency terms.
    pub plan: analyze::Plan,
}

/// `POST /_reactor/explain`
///
/// Analyzes a request and returns the plan **without executing it**. This is the
/// "why did this miss, and what is it supposed to depend on" tool: the plan is
/// the same value the request path would act on, so there is no second
/// implementation to drift.
pub async fn explain(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ExplainRequest>,
) -> Response {
    let Ok(method) = http::Method::from_bytes(request.method.as_bytes()) else {
        return (StatusCode::BAD_REQUEST, "not an HTTP method").into_response();
    };
    let body = bytes::Bytes::from(request.body.clone());
    let headers = http::HeaderMap::new();
    let view = ReqView::new(&method, &request.path, &request.query, &headers, &body);
    let target = view.target.clone();
    let ns = view.namespace().cloned();
    let schema = match &ns {
        Some(ns) => state.engine.schema(ns),
        None => Arc::new(crate::schema_index::SchemaIndex::default()),
    };
    let plan = analyze::plan(&view, &schema, &state.analyze);

    Json(ExplainReport {
        target,
        tracked: ns
            .as_ref()
            .map(|ns| state.registry.tracks(ns))
            .unwrap_or(false),
        head: ns
            .as_ref()
            .and_then(|ns| state.engine.watermark(ns))
            .map(|w| w.commit.as_str().to_owned()),
        plan,
    })
    .into_response()
}

/// What `/_reactor/scope/{id}` answers.
#[derive(Debug, Serialize)]
pub struct ScopeReport {
    /// The namespace the recorded reads were against — the first one, for a
    /// scope that read several. Kept for compatibility; `namespaces` is the
    /// whole answer.
    pub namespace: Option<String>,
    /// Every namespace read, with its terms and its pin.
    pub namespaces: Vec<ScopeNamespaceReport>,
    /// How many reads have been recorded.
    pub reads: usize,
    /// The union of their dependency terms, namespace-qualified where the read
    /// was not of the first namespace.
    pub terms: Vec<String>,
    /// The commits the reads were pinned at, across all namespaces.
    pub commits: Vec<String>,
    /// Why a value may not be stored against this scope, if that is so. Stated
    /// against the scope's own single commit when it has one, which is the
    /// question a developer is actually asking.
    pub refusal: Option<String>,
}

/// One namespace's line in a [`ScopeReport`].
#[derive(Debug, Serialize)]
pub struct ScopeNamespaceReport {
    /// `org/db/local/branch/<branch>`.
    pub namespace: String,
    /// The terms read there, in Reactor's wire encoding.
    pub terms: Vec<String>,
    /// The commits those reads were pinned at. More than one is a refusal.
    pub commits: Vec<String>,
}

/// `GET /_reactor/scope/{id}`
///
/// What a recording scope has observed so far, without consuming it. The
/// "why will my derived value not cache" endpoint: an application developer
/// gets a 409 from the store with one reason, and this is how they see the
/// reads that led to it.
pub async fn scope(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Response {
    let Some(record) = state.recorder.peek(&id) else {
        return (
            StatusCode::NOT_FOUND,
            "no such scope: it may have expired, or no labelled read has reached this node yet\n",
        )
            .into_response();
    };
    // Which namespace is "home" is decided by the store's URL, which this
    // endpoint does not have. The first one read is the useful guess: it is the
    // one a single-database computation read, and for a composition the refusal
    // we want to surface is almost always about the pins rather than about
    // which database is home.
    let home = record.namespaces().next().cloned();
    let refusal = home.as_ref().and_then(|home| {
        let at = record
            .commit_for(home)
            .map(terminusdb_client::CommitId::new)
            .unwrap_or_else(|| terminusdb_client::CommitId::new(""));
        record.refusal(home, &at)
    });
    Json(ScopeReport {
        namespace: home.as_ref().map(Namespace::path),
        namespaces: record
            .by_ns
            .iter()
            .map(|(ns, reads)| ScopeNamespaceReport {
                namespace: ns.path(),
                terms: reads.terms.iter().map(|term| term.to_string()).collect(),
                commits: reads.commits.iter().cloned().collect(),
            })
            .collect(),
        reads: record.reads,
        terms: home
            .as_ref()
            .map(|home| {
                record
                    .deps(home)
                    .iter()
                    .map(|dep| dep.to_string())
                    .collect()
            })
            .unwrap_or_default(),
        commits: record
            .by_ns
            .values()
            .flat_map(|reads| reads.commits.iter().cloned())
            .collect(),
        refusal,
    })
    .into_response()
}

/// The request body for `POST`/`DELETE` `/_reactor/namespace`.
#[derive(Debug, Deserialize)]
pub struct NamespaceRequest {
    /// Organization.
    pub org: String,
    /// Database.
    pub db: String,
    /// Branch. Defaults to `main`.
    #[serde(default = "default_main")]
    pub branch: String,
}

/// What a registration reports back.
#[derive(Debug, Serialize)]
pub struct TrackReport {
    /// `org/db/local/branch/<branch>`.
    pub namespace: String,
    /// Whether this call started tracking it, as opposed to finding it already
    /// tracked. Registration is idempotent, so `false` is a success.
    pub added: bool,
    /// How many namespaces are tracked now.
    pub tracked: usize,
    /// The bound.
    pub cap: usize,
}

/// `GET /_reactor/namespace`
///
/// Every tracked namespace, where it came from, and how long since a read
/// touched it — which is also what decides how often its head is polled.
pub async fn namespaces(State(state): State<Arc<AppState>>) -> Json<Vec<crate::registry::Tracked>> {
    Json(state.registry.report())
}

/// `POST /_reactor/namespace`
///
/// Start tracking a namespace. Idempotent, so an application that creates a
/// database per tenant can call this on the path that creates the database, or
/// on every request, without bookkeeping of its own.
///
/// Tracking means two things happen: reads of that namespace become cacheable,
/// and a capture task starts watching its head. The first read will still be a
/// miss — there is no position to pin a fill at until the watermark lands.
pub async fn track(
    State(state): State<Arc<AppState>>,
    Json(request): Json<NamespaceRequest>,
) -> Response {
    let ns = Namespace::new(request.org, request.db, request.branch);
    match state
        .registry
        .track(ns.clone(), crate::registry::Source::Registered)
    {
        Ok(added) => {
            if added {
                // Capture is what makes tracking mean anything: without a
                // watermark nothing in the namespace is served from cache.
                state.hint(CaptureHint::Track { ns: ns.clone() });
                tracing::info!(%ns, "namespace registered");
            }
            Json(TrackReport {
                namespace: ns.path(),
                added,
                tracked: state.registry.len(),
                cap: state.registry.cap(),
            })
            .into_response()
        }
        Err(full) => (
            StatusCode::INSUFFICIENT_STORAGE,
            format!(
                "{full}. Reads of {ns} are proxied uncached rather than evicting a namespace \
                 something may be using. Raise [capture].max_tracked_namespaces if this node \
                 has the capacity for another capture task.\n"
            ),
        )
            .into_response(),
    }
}

/// `DELETE /_reactor/namespace`
///
/// Stop tracking a namespace, and drop what was cached for it.
///
/// The purge is the point: an untracked namespace has no capture task, so
/// entries left behind would be invalidated by nothing. A namespace named in
/// the configuration file cannot be untracked — the file is the operator's
/// statement of intent, and the process would track it again on restart anyway.
pub async fn untrack(
    State(state): State<Arc<AppState>>,
    Json(request): Json<NamespaceRequest>,
) -> Response {
    let ns = Namespace::new(request.org, request.db, request.branch);
    match state.registry.untrack(&ns) {
        Ok(false) => (StatusCode::OK, format!("{ns} was not tracked\n")).into_response(),
        Ok(true) => {
            // Forget the watermark too: a namespace we have stopped watching
            // must not look current if it is tracked again later.
            if let Err(err) = state.engine.send(
                &ns,
                WriterMsg::Purge {
                    forget_watermark: true,
                },
            ) {
                tracing::warn!(%ns, error = %err, "untracked, but the purge did not reach the writer");
            }
            state.recorder.clear();
            tracing::info!(%ns, "namespace untracked and purged");
            (StatusCode::OK, format!("untracked and purged {ns}\n")).into_response()
        }
        Err(configured) => (StatusCode::CONFLICT, format!("{configured}\n")).into_response(),
    }
}

/// The request body for `/_reactor/invalidate`.
#[derive(Debug, Deserialize)]
pub struct InvalidateRequest {
    /// Organization.
    pub org: String,
    /// Database.
    pub db: String,
    /// Branch. Defaults to `main`.
    #[serde(default = "default_main")]
    pub branch: String,
}

fn default_main() -> String {
    "main".to_owned()
}

/// `POST /_reactor/invalidate`
///
/// Drops a namespace's cached state. The epoch is bumped too, so fills already
/// in flight cannot land afterwards — an operator who invalidates expects the
/// cache to be empty when the call returns, not a moment later.
///
/// Memoized authorization grants are dropped at the same time, so this is also
/// the way to make a revocation take effect without waiting out the TTL.
pub async fn invalidate(
    State(state): State<Arc<AppState>>,
    Json(request): Json<InvalidateRequest>,
) -> Response {
    let ns = Namespace::new(request.org, request.db, request.branch);
    // The history is intact, so the watermark is still true: keeping it lets
    // the next read refill immediately rather than bypassing.
    match state.engine.send(
        &ns,
        WriterMsg::Purge {
            forget_watermark: false,
        },
    ) {
        Ok(()) => {
            // "Empty the cache" should mean it, learned framings included. Open
            // recording scopes go too: their reads were answered from state
            // that no longer exists, so a value stored against one would claim
            // dependencies derived from bytes nobody can account for.
            state.framings.clear();
            state.auth.clear();
            state.recorder.clear();
            tracing::info!(%ns, "namespace invalidated by operator request");
            (StatusCode::ACCEPTED, format!("purged {ns}\n")).into_response()
        }
        Err(err) => (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("could not reach the writer for {ns}: {err}\n"),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn healthz_is_liveness_only() {
        assert_eq!(healthz().await, "ok\n");
    }

    #[test]
    fn an_explain_request_defaults_to_a_get_with_no_body() {
        let request: ExplainRequest =
            serde_json::from_value(serde_json::json!({"path": "/api/document/admin/db"})).unwrap();
        assert_eq!(request.method, "GET");
        assert!(request.query.is_empty());
        assert!(request.body.is_empty());
    }

    #[test]
    fn an_invalidate_request_defaults_to_main() {
        let request: InvalidateRequest =
            serde_json::from_value(serde_json::json!({"org": "admin", "db": "people"})).unwrap();
        assert_eq!(request.branch, "main");
    }

    #[test]
    fn a_fragment_hash_must_be_hex() {
        assert!(ContentHash::from_hex("nope").is_none());
        let hash = ContentHash::of(b"body");
        assert_eq!(ContentHash::from_hex(&hash.to_hex()), Some(hash));
        // Quoted, as it appears in an ETag.
        assert_eq!(
            ContentHash::from_hex(format!("\"{}\"", hash.to_hex()).trim_matches('"')),
            Some(hash)
        );
    }
}
