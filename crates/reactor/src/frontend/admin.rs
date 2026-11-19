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

use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::analyze::{self, ReqView};
use crate::engine::WriterMsg;
use crate::ids::{ContentHash, Namespace};

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

/// `GET /_reactor/head`
///
/// The honest answer to "how far behind is this node", which is what a smart
/// client or load balancer needs in order to route by replication state instead
/// of guessing.
pub async fn head(State(state): State<Arc<AppState>>) -> Json<Vec<HeadReport>> {
    let mut out = Vec::new();
    for ns in state.config.namespaces() {
        let watermark = state.engine.watermark(&ns);
        out.push(HeadReport {
            namespace: ns.path(),
            commit: watermark.as_ref().map(|w| w.commit.as_str().to_owned()),
            position: watermark.as_ref().map(|w| w.pos.to_string()),
            classes: state.engine.schema(&ns).classes().count(),
            schema_refresh_pending: state.engine.schema_refresh_requested(&ns),
        });
    }
    Json(out)
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
            .map(|ns| state.config.tracks(ns))
            .unwrap_or(false),
        head: ns
            .as_ref()
            .and_then(|ns| state.engine.watermark(ns))
            .map(|w| w.commit.as_str().to_owned()),
        plan,
    })
    .into_response()
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
            // "Empty the cache" should mean it, learned framings included.
            state.framings.clear();
            state.auth.clear();
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
