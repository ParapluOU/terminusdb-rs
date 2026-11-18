//! The request path: lookup, fill, and the client-visible protocol.
//!
//! The order of checks in [`handle`] is the order they are in for a reason:
//!
//! 1. **Streaming shapes bypass first.** An SSE subscription never ends; buffering
//!    one to hash it would hang the client forever.
//! 2. **Untracked namespaces bypass.** An unlisted database is proxied and never
//!    cached — the safe default for a database nobody configured.
//! 3. **Then the plan**, which decides read/write/resync/bypass.
//! 4. **Then the fence**, before anything is served, because a client that
//!    demanded a version must not be answered from an older one.
//! 5. **Then authorization**, before any cached byte is handed over. Cache keys
//!    contain no user, so this is the only thing standing between a hit and an
//!    access-control bypass.
//!
//! Every response carries `Cache-Status` (RFC 9211 shape), so what Reactor did
//! is always visible from the outside rather than inferred from timing.

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use http::header::{HeaderMap, HeaderValue};
use http::{Method, Request, StatusCode};

use crate::analyze::{self, Augment, Plan, ReqView, Rewrite, Target};
use crate::capture::CaptureHint;
use crate::config::MinVersionPolicy;
use crate::engine::{BodyRef, CommitLookup, FragmentKey, FragmentVersion, Lookup, ResponseMeta};
use crate::frontend::fill::{FillOutcome, SharedOutcome};
use crate::frontend::proxy::{self, UpstreamRequest};
use crate::frontend::AppState;
use crate::ids::{Namespace, Position, Watermark};
use crate::term::Term;
use terminusdb_client::CommitId;

/// What Reactor did with a request, for `Cache-Status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheState {
    /// Served from cache.
    Hit,
    /// Cacheable, but we had to fill it.
    Miss,
    /// Not cached, by analysis or by policy.
    Bypass,
}

impl CacheState {
    fn as_str(self) -> &'static str {
        match self {
            CacheState::Hit => "hit",
            CacheState::Miss => "miss",
            CacheState::Bypass => "bypass",
        }
    }
}

/// Render the `Cache-Status` value.
///
/// RFC 9211 shape: the cache's name first, then what it did, then optional
/// parameters. Keeping to the standard means an operator's existing tooling can
/// read it.
pub fn cache_status(state: CacheState, detail: Option<&str>) -> String {
    match detail {
        Some(detail) => format!("Reactor; {}; detail={}", state.as_str(), detail),
        None => format!("Reactor; {}", state.as_str()),
    }
}

/// What to do about an unsatisfied `X-Reactor-Min-Version`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FenceAction {
    /// The demanded version is reached; carry on.
    Proceed,
    /// Wait for it, then fall through to the origin.
    Wait,
    /// Answer `412` so a load balancer can retry on a node that has it.
    Fail,
    /// Answer from what we have, labelled.
    ServeStale,
}

/// Decide what a fence demands.
///
/// [`CommitLookup::Unknown`] is **never** satisfied. It means "not applied here
/// yet, or applied so long ago we have forgotten" and we cannot tell those
/// apart; treating it as satisfied in either case is how a fence silently stops
/// fencing.
///
/// `Applied(p)` is only satisfied when `p <= latched`, because the writer
/// publishes a commit's position just before its watermark: a reader can
/// momentarily see a position it has not latched.
pub fn fence_action(
    policy: MinVersionPolicy,
    lookup: CommitLookup,
    latched: Position,
) -> FenceAction {
    match lookup {
        CommitLookup::Applied(pos) if pos <= latched => FenceAction::Proceed,
        _ => match policy {
            MinVersionPolicy::Wait => FenceAction::Wait,
            MinVersionPolicy::Fail => FenceAction::Fail,
            MinVersionPolicy::ServeStale => FenceAction::ServeStale,
        },
    }
}

/// True when a request wants a streaming response we must never buffer.
pub fn wants_streaming(headers: &HeaderMap) -> bool {
    headers
        .get(http::header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .map(|accept| {
            let accept = accept.to_ascii_lowercase();
            accept.contains("text/event-stream") || accept.contains("application/x-ndjson")
        })
        .unwrap_or(false)
}

/// The upstream path for a fill, pinned at `commit`.
///
/// `None` means we cannot pin this request by URL, and therefore must not cache
/// its response: invariant 2 says a fragment is "the value at that commit", and
/// an unpinned read cannot promise that. The bare `POST /api/woql` form (no
/// `<org>/<db>` in the path) is exactly this case — it carries its resource in
/// the body, so there is no URL to pin. The `/api/woql/<path>` form is fine.
pub fn pinned_path(target: &Target, commit: &CommitId, query: &str) -> Option<String> {
    let path = match target {
        // Already pinned: the client asked for a fixed commit.
        Target::Pinned { endpoint, ns, .. } => {
            format!("/api/{}/{}", endpoint, ns.commit_path(commit))
        }
        Target::Branch { endpoint, ns } => {
            format!("/api/{}/{}", endpoint, ns.commit_path(commit))
        }
        Target::Other { .. } => return None,
    };
    Some(if query.is_empty() {
        path
    } else {
        format!("{path}?{query}")
    })
}

/// Build the response for a cache hit.
///
/// `Age` and `ETag` only appear on hits: `Age` is meaningless for a response we
/// just fetched, and an `ETag` we did not compute is not ours to claim.
fn hit_response(
    state: &AppState,
    version: &FragmentVersion,
    body: Bytes,
    watermark: &Watermark,
    request_headers: &HeaderMap,
) -> Response {
    let etag = format!("\"{}\"", version.hash.to_hex());
    let not_modified = request_headers
        .get(http::header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(',').any(|candidate| candidate.trim() == etag))
        .unwrap_or(false);

    let mut builder = Response::builder()
        .status(StatusCode::from_u16(version.meta.status).unwrap_or(StatusCode::OK));
    {
        let headers = builder.headers_mut().expect("a fresh builder has headers");
        if let Some(content_type) = &version.meta.content_type {
            if let Some(value) = proxy::header_value(content_type) {
                headers.insert(http::header::CONTENT_TYPE, value);
            }
        }
        if let Some(data_version) = &version.meta.data_version {
            if let Some(value) = proxy::header_value(data_version) {
                headers.insert("terminusdb-data-version", value);
            }
        }
        if let Some(value) = proxy::header_value(&etag) {
            headers.insert(http::header::ETAG, value);
        }
        headers.insert(
            http::header::AGE,
            HeaderValue::from(version.age().as_secs()),
        );
        decorate(headers, state, Some(watermark), CacheState::Hit, None);
    }

    if not_modified {
        // The client already holds these bytes.
        return builder
            .status(StatusCode::NOT_MODIFIED)
            .body(Body::empty())
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
    }
    builder
        .body(Body::from(body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// Add the headers every response carries.
fn decorate(
    headers: &mut HeaderMap,
    state: &AppState,
    watermark: Option<&Watermark>,
    cache_state: CacheState,
    detail: Option<&str>,
) {
    if let Some(value) = proxy::header_value(&cache_status(cache_state, detail)) {
        headers.insert("cache-status", value);
    }
    if let Some(value) = proxy::header_value(&state.config.node.id) {
        headers.insert("x-reactor-node", value);
    }
    if let Some(watermark) = watermark {
        if let Some(value) = proxy::header_value(watermark.commit.as_str()) {
            headers.insert("x-reactor-head", value);
        }
    }
}

/// Turn an upstream response into a client response.
fn relay(
    state: &AppState,
    upstream: proxy::UpstreamResponse,
    watermark: Option<&Watermark>,
    cache_state: CacheState,
    detail: Option<&str>,
) -> Response {
    let mut builder = Response::builder().status(upstream.status);
    {
        let headers = builder.headers_mut().expect("a fresh builder has headers");
        proxy::copy_response_headers(&upstream.headers, headers);
        decorate(headers, state, watermark, cache_state, detail);
    }
    builder
        .body(Body::from(upstream.body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// A bad-gateway response for an upstream we could not reach.
fn bad_gateway(state: &AppState, reason: &str) -> Response {
    let mut response = (StatusCode::BAD_GATEWAY, reason.to_owned()).into_response();
    decorate(
        response.headers_mut(),
        state,
        None,
        CacheState::Bypass,
        Some("upstream-error"),
    );
    response
}

/// The main proxy handler: everything not under `/_reactor/` or `/healthz`.
pub async fn handle(State(state): State<Arc<AppState>>, request: Request<Body>) -> Response {
    let (parts, body) = request.into_parts();
    let method = parts.method.clone();
    let path = parts.uri.path().to_owned();
    let query = parts.uri.query().unwrap_or("").to_owned();
    let path_and_query = match parts.uri.query() {
        Some(query) => format!("{path}?{query}"),
        None => path.clone(),
    };
    let mut headers = parts.headers.clone();
    proxy::strip_hop_by_hop(&mut headers);

    // Streaming requests must never be buffered, so decide before reading the
    // body — and an SSE request has no body anyway.
    if wants_streaming(&parts.headers) {
        return stream_through(&state, method, path_and_query, headers, Bytes::new()).await;
    }

    let limit = state.config.max_cacheable_body_bytes().max(1024 * 1024);
    let body = match axum::body::to_bytes(body, limit).await {
        Ok(body) => body,
        Err(_) => {
            // Too large to analyze, so it cannot be a cacheable read. We have
            // already consumed part of the body and cannot replay it, so this
            // is the one case we have to refuse rather than forward.
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                "request body exceeds the configured limit",
            )
                .into_response();
        }
    };

    let bypass_requested = headers.contains_key("x-reactor-bypass");
    let view = ReqView::new(&method, &path, &query, &headers, &body);
    let request = UpstreamRequest {
        method: method.clone(),
        path_and_query,
        headers: headers.clone(),
        body: body.clone(),
    };

    if bypass_requested {
        return forward(&state, &request, CacheState::Bypass, Some("client")).await;
    }

    let ns = match view.namespace() {
        Some(ns) => ns.clone(),
        None => {
            return forward(
                &state,
                &request,
                CacheState::Bypass,
                Some("not-a-namespace"),
            )
            .await
        }
    };
    if !state.config.tracks(&ns) {
        // The safe default for a database nobody configured.
        return forward(&state, &request, CacheState::Bypass, Some("untracked")).await;
    }

    let schema = state.engine.schema(&ns);
    let plan = analyze::plan(&view, &schema, &state.analyze);

    // Explain-only: report the plan without executing anything.
    if headers.contains_key("x-reactor-explain") {
        let mut response = axum::Json(&plan).into_response();
        decorate(
            response.headers_mut(),
            &state,
            state.engine.watermark(&ns).as_ref(),
            CacheState::Bypass,
            Some("explain"),
        );
        return response;
    }

    // Visibility into the opt-in WOQL rule: an operator needs to see how often
    // a query declines to be cached, and why.
    match &plan {
        Plan::Bypass { reason } => match *reason {
            "woql needs an explicit invalidation condition" => {
                state.engine.metrics().woql_without_conditions.inc()
            }
            "woql invalidation condition did not parse" => {
                state.engine.metrics().woql_bad_conditions.inc()
            }
            _ => {}
        },
        Plan::Read { terms, .. } if view.target.endpoint() == "woql" && !terms.is_empty() => {
            state.engine.metrics().woql_with_conditions.inc()
        }
        _ => {}
    }

    match plan {
        Plan::Bypass { reason } => {
            forward(&state, &request, CacheState::Bypass, Some(reason)).await
        }
        Plan::Write => serve_write(&state, &request, &ns).await,
        Plan::Resync {
            reason,
            ns: hinted,
            purge,
        } => {
            let response = forward(&state, &request, CacheState::Bypass, Some(reason)).await;
            if response.status().is_success() {
                let target = hinted.unwrap_or(ns);
                // Purge BEFORE the response goes back to the client, and wait
                // for it to actually take effect.
                //
                // Queuing it is not enough. `Engine::send` only hands the
                // message to the writer's mailbox; if we answered 200 while the
                // purge was still queued, a client that immediately re-read
                // would be served the pre-rewrite entry it just invalidated —
                // exactly the bug this path exists to prevent. So we wait for
                // the epoch to advance, which is the observable effect of a
                // purge having been applied.
                purge_now(&state, &target, purge).await;
                // And resynchronize: a purged namespace with a stale watermark
                // would pin later fills at a commit that may no longer be on
                // the branch.
                state.hint(CaptureHint::Resync { ns: target });
            }
            response
        }
        Plan::Read {
            key,
            terms,
            immutable,
            rewrite,
            augment,
        } => {
            serve_read(
                &state,
                &request,
                &ns,
                &view.target,
                &query,
                key,
                terms,
                immutable,
                rewrite,
                augment,
            )
            .await
        }
    }
}

/// How long to wait for a purge to be applied before answering anyway.
///
/// Short: this is on the response path of a topology call that has already
/// done its work upstream. Giving up leaves a brief window in which a stale
/// entry could still be served, which is why the wait exists at all — but
/// blocking a successful `reset` indefinitely because a writer task is wedged
/// would be worse.
const PURGE_WAIT: Duration = Duration::from_secs(2);

/// Drop cached state for a topology call that rewrote data.
///
/// Namespace-scoped on purpose: a `reset` on one branch must not empty another
/// branch's cache, and a database-wide purge must not reach another database.
async fn purge_now(state: &AppState, target: &Namespace, scope: analyze::PurgeScope) {
    let namespaces: Vec<Namespace> = match scope {
        analyze::PurgeScope::None => return,
        analyze::PurgeScope::Branch => vec![target.clone()],
        // Every branch of this database that we track.
        analyze::PurgeScope::Database => state
            .config
            .namespaces()
            .into_iter()
            .filter(|ns| ns.org == target.org && ns.db == target.db)
            .collect(),
    };

    for ns in namespaces {
        let before = state.engine.watermark(&ns).map(|w| w.pos.epoch);
        // These endpoints rewrite history, so the watermark must go too: it
        // names a commit that may no longer be on the branch, and a fill pinned
        // there would faithfully return pre-rewrite data. Found by
        // `reset_purges_the_branch`, which saw exactly that — the cache was
        // correctly emptied and then refilled with pre-reset bytes.
        if state
            .engine
            .send(
                &ns,
                crate::engine::WriterMsg::Purge {
                    forget_watermark: true,
                },
            )
            .is_err()
        {
            tracing::debug!(%ns, "no writer to purge");
            continue;
        }
        tracing::info!(%ns, ?scope, "purging after a topology call");

        // With no watermark nothing can have been cached (a read without one
        // bypasses, because there is no position to pin a fill at), so there is
        // nothing to wait for.
        let Some(before) = before else { continue };
        let deadline = Instant::now() + PURGE_WAIT;
        while Instant::now() < deadline {
            match state.engine.watermark(&ns).map(|w| w.pos.epoch) {
                // Either observable effect of the purge having been applied:
                // the watermark is gone, or the epoch advanced.
                None => break,
                Some(now) if now > before => break,
                _ => tokio::time::sleep(Duration::from_millis(2)).await,
            }
        }
    }
}

/// Forward a request and relay the buffered response.
async fn forward(
    state: &AppState,
    request: &UpstreamRequest,
    cache_state: CacheState,
    detail: Option<&str>,
) -> Response {
    match state.proxy.send(request).await {
        Ok(upstream) => relay(state, upstream, None, cache_state, detail),
        Err(err) => {
            tracing::warn!(error = %err, path = %request.path_and_query, "upstream request failed");
            bad_gateway(state, "upstream request failed")
        }
    }
}

/// Relay a streaming response without buffering it.
async fn stream_through(
    state: &AppState,
    method: Method,
    path_and_query: String,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request = UpstreamRequest {
        method,
        path_and_query,
        headers,
        body,
    };
    match state.proxy.start(&request).await {
        Ok(upstream) => {
            let status = upstream.status();
            let mut upstream_headers = upstream.headers().clone();
            proxy::strip_hop_by_hop(&mut upstream_headers);
            let mut builder = Response::builder().status(status);
            {
                let headers = builder.headers_mut().expect("a fresh builder has headers");
                proxy::copy_response_headers(&upstream_headers, headers);
                decorate(headers, state, None, CacheState::Bypass, Some("stream"));
            }
            builder
                .body(Body::from_stream(upstream.bytes_stream()))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        Err(err) => {
            tracing::warn!(error = %err, "upstream streaming request failed");
            bad_gateway(state, "upstream streaming request failed")
        }
    }
}

/// A mutation: forward it, then make read-your-writes work on this node.
async fn serve_write(state: &AppState, request: &UpstreamRequest, ns: &Namespace) -> Response {
    let upstream = match state.proxy.send(request).await {
        Ok(upstream) => upstream,
        Err(err) => {
            tracing::warn!(error = %err, "upstream write failed");
            return bad_gateway(state, "upstream write failed");
        }
    };
    let commit = upstream.commit_id();
    let succeeded = upstream.is_success();
    let response = relay(state, upstream, None, CacheState::Bypass, Some("write"));

    if !succeeded {
        return response;
    }
    let Some(commit) = commit else {
        // A successful write with no data-version: we do not know what it
        // produced, so ask capture to re-examine the head rather than guess.
        state.hint(CaptureHint::Resync { ns: ns.clone() });
        return response;
    };

    // Hand the commit to capture immediately: waiting for a driver to notice
    // would make every write pay the poll interval.
    state.hint(CaptureHint::Commit {
        ns: ns.clone(),
        commit: commit.clone(),
    });

    let fence = Duration::from_millis(state.config.consistency.write_fence_ms);
    if !fence.is_zero() {
        // Hold the response until our own watermark reaches the commit, so the
        // client's next read on this node cannot be older than its write.
        let deadline = Instant::now() + fence;
        while Instant::now() < deadline {
            if matches!(
                state.engine.position_of(ns, &commit),
                CommitLookup::Applied(_)
            ) {
                return response;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        tracing::debug!(
            %ns, %commit,
            "write fence timed out; the response is correct but a later read here may lag"
        );
    }
    response
}

/// A cacheable read.
#[allow(clippy::too_many_arguments)]
async fn serve_read(
    state: &AppState,
    request: &UpstreamRequest,
    ns: &Namespace,
    target: &Target,
    query: &str,
    key: FragmentKey,
    terms: Vec<Term>,
    immutable: bool,
    rewrite: Option<Rewrite>,
    augment: Augment,
) -> Response {
    // Without a watermark there is no position to pin a fill at, so there is
    // nothing we can honestly cache.
    let Some(watermark) = state.engine.watermark(ns) else {
        return forward(state, request, CacheState::Bypass, Some("no-watermark")).await;
    };

    // The fence comes before anything is served.
    if let Some(min_version) = request
        .headers
        .get("x-reactor-min-version")
        .and_then(|value| value.to_str().ok())
    {
        let demanded = CommitId::new(min_version.trim());
        match resolve_fence(state, ns, &demanded, &watermark).await {
            FenceResolution::Proceed => {}
            FenceResolution::Fail => {
                let mut response = (
                    StatusCode::PRECONDITION_FAILED,
                    format!("this node has not reached {demanded}"),
                )
                    .into_response();
                decorate(
                    response.headers_mut(),
                    state,
                    Some(&watermark),
                    CacheState::Bypass,
                    Some("min-version"),
                );
                return response;
            }
            FenceResolution::FallThrough => {
                return forward(state, request, CacheState::Bypass, Some("min-version")).await;
            }
            FenceResolution::ServeStale => {}
        }
    }

    // Authorization, before a single cached byte is handed over. Cache keys
    // contain no user, so this is what keeps a hit from becoming a bypass.
    let authorized = state
        .auth
        .allows(request.headers.get(http::header::AUTHORIZATION), ns)
        .await;
    if !authorized.is_allowed() {
        return forward(state, request, CacheState::Bypass, Some("unauthorized")).await;
    }

    let at = watermark.pos;
    if let Lookup::Hit(version) = state.engine.lookup(&key, at) {
        if let Some(body) = resolve_body(state, &version) {
            if state
                .config
                .engine_config()
                .policy
                .for_path(request.path_and_query.as_str())
                .fresh_enough(version.age())
            {
                state.engine.metrics().lookup_hits.inc();
                return hit_response(state, &version, body, &watermark, &request.headers);
            }
        }
        // The body was evicted from the store, or the entry is past its TTL:
        // both are plain misses.
    }

    // Miss: fill once, pinned, however many callers are waiting.
    let outcome = fill_now(
        state, request, ns, target, query, &key, &watermark, terms, immutable, rewrite, augment,
    )
    .await;

    match outcome.as_ref().as_ref() {
        Ok(fill) => {
            let mut builder = Response::builder()
                .status(StatusCode::from_u16(fill.status).unwrap_or(StatusCode::OK));
            {
                let headers = builder.headers_mut().expect("a fresh builder has headers");
                proxy::copy_response_headers(&fill.headers, headers);
                decorate(
                    headers,
                    state,
                    Some(&watermark),
                    CacheState::Miss,
                    fill.version.is_none().then_some("uncacheable"),
                );
            }
            builder
                .body(Body::from(fill.body.clone()))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        Err(err) => {
            tracing::warn!(error = %err, "fill failed");
            bad_gateway(state, "fill failed")
        }
    }
}

/// What a fence concluded, after any waiting.
enum FenceResolution {
    /// Satisfied.
    Proceed,
    /// Answer 412.
    Fail,
    /// Forward to the origin uncached.
    FallThrough,
    /// Serve what we have, labelled.
    ServeStale,
}

/// Apply the configured fence policy, waiting if that is the policy.
async fn resolve_fence(
    state: &AppState,
    ns: &Namespace,
    demanded: &CommitId,
    watermark: &Watermark,
) -> FenceResolution {
    let policy = state.config.consistency.min_version_policy;
    match fence_action(
        policy,
        state.engine.position_of(ns, demanded),
        watermark.pos,
    ) {
        FenceAction::Proceed => FenceResolution::Proceed,
        FenceAction::Fail => FenceResolution::Fail,
        FenceAction::ServeStale => FenceResolution::ServeStale,
        FenceAction::Wait => {
            let deadline = Instant::now()
                + Duration::from_millis(state.config.consistency.min_version_wait_ms);
            while Instant::now() < deadline {
                // Re-latch: the watermark moves while we wait, and the whole
                // point is to be answered from the newer one.
                if let Some(current) = state.engine.watermark(ns) {
                    if matches!(
                        fence_action(policy, state.engine.position_of(ns, demanded), current.pos),
                        FenceAction::Proceed
                    ) {
                        return FenceResolution::Proceed;
                    }
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            tracing::debug!(%ns, %demanded, "min-version fence timed out; falling through");
            FenceResolution::FallThrough
        }
    }
}

/// Fetch a version's bytes, wherever they live.
fn resolve_body(state: &AppState, version: &FragmentVersion) -> Option<Bytes> {
    match &version.body {
        BodyRef::Inline(bytes) => Some(bytes.clone()),
        // A miss here is eviction, not corruption: the fragment is simply
        // refilled.
        BodyRef::Cas(hash) => state.engine.store().get(hash),
    }
}

/// Run (or join) the pinned fill for this key.
#[allow(clippy::too_many_arguments)]
async fn fill_now(
    state: &AppState,
    request: &UpstreamRequest,
    ns: &Namespace,
    target: &Target,
    query: &str,
    key: &FragmentKey,
    watermark: &Watermark,
    terms: Vec<Term>,
    immutable: bool,
    rewrite: Option<Rewrite>,
    augment: Augment,
) -> SharedOutcome {
    let state_for_fill = state.clone_handle();
    let request = request.clone();
    let ns = ns.clone();
    let target = target.clone();
    let query = query.to_owned();
    let key_for_fill = key.clone();
    let watermark = watermark.clone();

    state
        .fills
        .fill(key.clone(), watermark.pos, move || async move {
            execute_fill(
                state_for_fill,
                request,
                ns,
                target,
                query,
                key_for_fill,
                watermark,
                terms,
                immutable,
                rewrite,
                augment,
            )
            .await
        })
        .await
}

/// Issue the pinned upstream request and store the result if we may.
#[allow(clippy::too_many_arguments)]
async fn execute_fill(
    state: Arc<AppState>,
    request: UpstreamRequest,
    ns: Namespace,
    target: Target,
    query: String,
    key: FragmentKey,
    watermark: Watermark,
    mut terms: Vec<Term>,
    immutable: bool,
    rewrite: Option<Rewrite>,
    augment: Augment,
) -> anyhow::Result<FillOutcome> {
    // Invariant 2: fills are pinned. A request we cannot pin is forwarded and
    // never cached.
    let Some(path) = pinned_path(&target, &watermark.commit, &query) else {
        let upstream = state.proxy.send(&request).await?;
        return Ok(uncacheable(upstream, "unpinnable"));
    };

    let mut pinned = request.clone();
    pinned.path_and_query = path;
    if let Some(rewrite) = &rewrite {
        // GraphQL with `_id`/`_type` injected: the body we execute is not the
        // body the client sent.
        pinned.body = Bytes::from(rewrite.body.clone());
        pinned.headers.remove(http::header::CONTENT_LENGTH);
    }

    let upstream = state.proxy.send(&pinned).await?;
    if !upstream.is_success() {
        // The pinned form was rejected (or the document is absent). Answer from
        // the original request so pinning cannot break a client, and cache
        // nothing. Whether every endpoint accepts `/local/commit/<id>` —
        // GraphQL in particular — is unverified; phase E should test it.
        tracing::debug!(
            %ns, status = %upstream.status, path = %pinned.path_and_query,
            "pinned fill was not successful; falling back to the unpinned request"
        );
        state.engine.metrics().fills_rejected_stale.inc();
        let fallback = state.proxy.send(&request).await?;
        return Ok(uncacheable(fallback, "pin-rejected"));
    }

    let body = upstream.body.clone();
    let max = state.config.max_cacheable_body_bytes();
    if body.len() > max {
        return Ok(uncacheable(upstream, "too-large"));
    }

    // Prefer the commit the response actually came from.
    let returned = upstream.commit_id();
    let (position, commit) = match &returned {
        Some(returned) if returned != &watermark.commit => {
            match state.engine.position_of(&ns, returned) {
                CommitLookup::Applied(pos) => (pos, returned.clone()),
                // We have no position for it, so we cannot say when it is valid.
                CommitLookup::Unknown => return Ok(uncacheable(upstream, "unknown-data-version")),
            }
        }
        Some(returned) => (watermark.pos, returned.clone()),
        None if immutable => (watermark.pos, watermark.commit.clone()),
        // No data-version and not an immutable pin: we cannot honestly claim
        // which commit these bytes are from.
        None => return Ok(uncacheable(upstream, "no-data-version")),
    };

    // Complete the read set from the response where the request alone could not
    // determine it.
    if augment == Augment::DocIds {
        crate::analyze::augment_terms_from_response(&body, &mut terms);
    }

    let meta = ResponseMeta {
        status: upstream.status.as_u16(),
        content_type: upstream.content_type(),
        data_version: upstream.data_version(),
    };

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    let fill = crate::engine::FillResult {
        key,
        at: position,
        commit,
        body: body.clone(),
        meta,
        deps: if immutable { Vec::new() } else { terms },
        children: Vec::new(),
        reply: Some(reply_tx),
    };
    state
        .engine
        .send(&ns, crate::engine::WriterMsg::Insert(Box::new(fill)))?;

    // The writer answers with the version it stored, or with an unstored one if
    // it had to reject the fill. Either way the bytes are ours to serve.
    let version = reply_rx.await.ok();
    let cached = version.and_then(|version| version.is_open().then_some(version));

    let mut headers = upstream.headers.clone();
    proxy::strip_hop_by_hop(&mut headers);
    Ok(FillOutcome {
        status: upstream.status.as_u16(),
        headers,
        body,
        version: cached,
    })
}

/// A response we will serve but not store.
fn uncacheable(upstream: proxy::UpstreamResponse, reason: &str) -> FillOutcome {
    tracing::debug!(reason, status = %upstream.status, "response not cached");
    FillOutcome {
        status: upstream.status.as_u16(),
        headers: upstream.headers,
        body: upstream.body,
        version: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{Epoch, Seq};

    fn pos(seq: u64) -> Position {
        Position {
            epoch: Epoch(0),
            seq: Seq(seq),
        }
    }

    fn ns() -> Namespace {
        Namespace::new("admin", "mydb", "main")
    }

    #[test]
    fn cache_status_follows_rfc_9211_shape() {
        assert_eq!(cache_status(CacheState::Hit, None), "Reactor; hit");
        assert_eq!(cache_status(CacheState::Miss, None), "Reactor; miss");
        assert_eq!(
            cache_status(CacheState::Bypass, Some("untracked")),
            "Reactor; bypass; detail=untracked"
        );
    }

    #[test]
    fn a_reached_commit_satisfies_every_policy() {
        for policy in [
            MinVersionPolicy::Wait,
            MinVersionPolicy::Fail,
            MinVersionPolicy::ServeStale,
        ] {
            assert_eq!(
                fence_action(policy, CommitLookup::Applied(pos(3)), pos(5)),
                FenceAction::Proceed,
                "{policy:?}"
            );
            assert_eq!(
                fence_action(policy, CommitLookup::Applied(pos(5)), pos(5)),
                FenceAction::Proceed,
                "{policy:?}: equal is reached"
            );
        }
    }

    #[test]
    fn an_unknown_commit_is_never_satisfied() {
        // The regression this guards: treating "evicted from the ring" as
        // satisfied would let a stale read through a fence that was supposed to
        // prevent exactly that.
        assert_eq!(
            fence_action(MinVersionPolicy::Wait, CommitLookup::Unknown, pos(5)),
            FenceAction::Wait
        );
        assert_eq!(
            fence_action(MinVersionPolicy::Fail, CommitLookup::Unknown, pos(5)),
            FenceAction::Fail
        );
        assert_eq!(
            fence_action(MinVersionPolicy::ServeStale, CommitLookup::Unknown, pos(5)),
            FenceAction::ServeStale
        );
    }

    #[test]
    fn a_commit_applied_above_the_latched_position_is_not_yet_reached() {
        // The writer publishes a commit's position just before its watermark,
        // so a reader can see a position it has not latched.
        assert_eq!(
            fence_action(
                MinVersionPolicy::Fail,
                CommitLookup::Applied(pos(6)),
                pos(5)
            ),
            FenceAction::Fail
        );
    }

    #[test]
    fn streaming_shapes_are_recognized() {
        let mut headers = HeaderMap::new();
        assert!(!wants_streaming(&headers));

        for accept in [
            "text/event-stream",
            "application/x-ndjson",
            "application/json, text/event-stream",
            "TEXT/EVENT-STREAM",
        ] {
            headers.insert(http::header::ACCEPT, HeaderValue::from_str(accept).unwrap());
            assert!(wants_streaming(&headers), "{accept}");
        }

        headers.insert(
            http::header::ACCEPT,
            HeaderValue::from_static("application/json"),
        );
        assert!(!wants_streaming(&headers));
    }

    #[test]
    fn a_branch_request_is_repointed_at_a_commit() {
        let target = Target::parse("/api/document/admin/mydb/local/branch/main");
        assert_eq!(
            pinned_path(&target, &CommitId::new("abc123"), "id=Person/alice"),
            Some("/api/document/admin/mydb/local/commit/abc123?id=Person/alice".to_owned())
        );
    }

    #[test]
    fn the_bare_namespace_form_is_repointed_too() {
        let target = Target::parse("/api/document/admin/mydb");
        assert_eq!(
            pinned_path(&target, &CommitId::new("abc123"), ""),
            Some("/api/document/admin/mydb/local/commit/abc123".to_owned())
        );
    }

    #[test]
    fn an_already_pinned_request_keeps_its_pin() {
        let target = Target::parse("/api/woql/admin/mydb/local/commit/abc123");
        assert_eq!(
            pinned_path(&target, &CommitId::new("abc123"), ""),
            Some("/api/woql/admin/mydb/local/commit/abc123".to_owned())
        );
    }

    #[test]
    fn a_request_with_no_namespace_in_its_path_cannot_be_pinned() {
        // Bare `POST /api/woql` carries its resource in the body, so there is
        // no URL to pin and therefore nothing we may cache. The
        // `/api/woql/<path>` form is fine — that asymmetry is deliberate.
        let target = Target::parse("/api/woql");
        assert_eq!(pinned_path(&target, &CommitId::new("abc123"), ""), None);
        assert!(target.namespace().is_none());
    }

    #[test]
    fn namespaces_round_trip_through_the_pinned_path() {
        let ns = ns();
        let target = Target::Branch {
            endpoint: "document".to_owned(),
            ns: ns.clone(),
        };
        let path = pinned_path(&target, &CommitId::new("c1"), "").unwrap();
        assert!(path.contains(&ns.commit_path(&CommitId::new("c1"))));
        assert_eq!(
            Target::parse(&path).namespace().map(|n| n.db.clone()),
            Some("mydb".to_owned())
        );
    }
}
