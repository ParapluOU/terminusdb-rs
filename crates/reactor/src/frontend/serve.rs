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

use crate::analyze::{self, Augment, Decompose, ListSpec, Plan, ReqView, Rewrite, Target};
use crate::capture::CaptureHint;
use crate::compose::{self, Spine};
use crate::config::MinVersionPolicy;
use crate::engine::{
    BodyRef, CommitLookup, Decomposition, FragmentKey, FragmentVersion, Lookup, ResponseMeta,
    RootKey,
};
use crate::frontend::fill::{FillOutcome, SharedOutcome};
use crate::frontend::proxy::{self, UpstreamRequest};
use crate::frontend::scope;
use crate::frontend::AppState;
use crate::ids::{ContentHash, DocId, Namespace, Position, Watermark};
use crate::strip::StripSpec;
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
        // Already pinned: the client asked for a fixed commit, and that is the
        // commit we must fetch. `commit` is where *we* would have pinned an
        // unpinned request — the watermark — and using it here would answer a
        // pinned read with the live state, then cache that answer under the
        // commit-qualified key, where it would be wrong forever.
        Target::Pinned {
            endpoint,
            ns,
            commit: pin,
        } => {
            format!("/api/{}/{}", endpoint, ns.commit_path(pin))
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

/// Remove whatever Reactor injected into the query from the bytes about to be
/// served, or say that it could not be done.
///
/// `None` is not "serve it anyway": the body contains fields the client never
/// asked for, so the caller forwards the client's own request to the origin
/// instead. That costs a request and keeps the proxy transparent, which is the
/// right way round.
fn deliverable(state: &AppState, strip: &StripSpec, body: Bytes) -> Option<Bytes> {
    if strip.is_empty() {
        return Some(body);
    }
    match crate::strip::strip(&body, strip) {
        Some(stripped) => Some(stripped),
        None => {
            state.engine.metrics().responses_unstrippable.inc();
            tracing::warn!(
                bytes = body.len(),
                "could not remove the injected ids from a response; forwarding the client's \
                 own query instead"
            );
            None
        }
    }
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
    strip: &StripSpec,
) -> Response {
    hit_response_tagged(
        state,
        version,
        version.hash,
        body,
        watermark,
        request_headers,
        None,
        strip,
    )
}

/// As [`hit_response`], with the `ETag` stated separately.
///
/// A composed response's identity is not the hash of its own bytes: it is the
/// combination of its parent's and its children's hashes. Deriving it that way
/// rather than from the spliced bytes is what makes backdating visible to a
/// client — a refilled child whose content did not actually move leaves the
/// `ETag` alone, so a conditional request still gets a 304.
fn hit_response_tagged(
    state: &AppState,
    version: &FragmentVersion,
    hash: ContentHash,
    body: Bytes,
    watermark: &Watermark,
    request_headers: &HeaderMap,
    detail: Option<&str>,
    strip: &StripSpec,
) -> Response {
    // The `ETag` has to name the bytes the *client* got. Two clients can share
    // one fragment and strip differently from it, so the strip is part of the
    // identity of what was served.
    let etag = if strip.is_empty() {
        format!("\"{}\"", hash.to_hex())
    } else {
        format!("\"{}-{:016x}\"", hash.to_hex(), strip.fingerprint())
    };
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
        decorate(headers, state, Some(watermark), CacheState::Hit, detail);
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
pub(crate) fn decorate(
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

/// Tell the scope that labelled this read — and the client — what the answer
/// depended on.
///
/// This is what makes an application-level derived value's dependencies
/// *observed* rather than *declared*. The alternative — asking the application
/// to name the classes its transformation touched — is what the prototype this
/// project replaces did, and a wrong declaration is the one way to get stale
/// data out of a cache that is otherwise sound.
///
/// `deps: None` means "we could not attribute this read", and it is reported as
/// `everything` and poisons the scope. A bypass, a forwarded write and a
/// streamed body all leave a computation with inputs Reactor did not track, so
/// silence has to read as "do not cache" rather than as independence.
///
/// The response header is for clients that *can* see headers (anything not
/// going through the ORM, and anything not written in Rust); the scope is what
/// the ORM path relies on. Both say the same thing.
pub(crate) fn record(
    state: &AppState,
    request: &UpstreamRequest,
    response: &mut HeaderMap,
    ns: Option<&Namespace>,
    pinned: Option<&CommitId>,
    deps: Option<&[Term]>,
    untracked: &'static str,
) {
    let scope = request
        .headers
        .get(scope::SCOPE_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|id| !id.is_empty());
    let Some(scope_id) = scope else {
        return;
    };

    match (ns, deps) {
        (Some(ns), Some(terms)) => state.recorder.record(scope_id, ns, pinned, terms),
        _ => state.recorder.poison(scope_id, untracked),
    }

    let value = match deps {
        None => Term::Everything.to_string(),
        Some([]) => "nothing".to_owned(),
        Some(terms) => terms
            .iter()
            .map(|term| term.to_string())
            .collect::<Vec<_>>()
            .join(", "),
    };
    if let Some(value) = proxy::header_value(&value) {
        response.insert(crate::frontend::value::DEPS_HEADER, value);
    }
}

/// The dependency set to report for a response we are about to serve.
///
/// The *plan's* terms, not the stored version's: a commit-pinned entry is
/// immortal and so stores no dependencies at all, but a computation that read
/// through it still depends on what it read. Reporting the stored set would tell
/// a recording client that its value depends on nothing — the under-tracking
/// hazard, and silent.
///
/// Where a response reveals ids, those are part of the answer's read set too.
/// The decomposer is tried first because it understands NDJSON and `as_list`
/// alike, where a whole-body JSON walk sees a document list as several values
/// and gives up (reporting `Everything`, which correctly refuses to cache).
fn record_terms(plan: &[Term], augment: Augment, body: &Bytes) -> Vec<Term> {
    let mut terms = plan.to_vec();
    if augment == Augment::DocIds {
        match crate::compose::decompose(body) {
            Ok(decomposed) => terms.extend(decomposed.ids().into_iter().map(Term::Doc)),
            Err(_) => analyze::augment_terms_from_response(body, &mut terms),
        }
    }
    terms
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
    if !state.registry.tracks(&ns) {
        // The safe default for a database nobody configured — unless the
        // operator asked for the other one. An application that creates a
        // database per tenant cannot list them in a file, and registering each
        // one by hand is bookkeeping it will get wrong on some path.
        if state.config.capture.auto_register {
            match state
                .registry
                .track(ns.clone(), crate::registry::Source::Automatic)
            {
                Ok(true) => {
                    state.hint(crate::capture::CaptureHint::Track { ns: ns.clone() });
                    tracing::info!(%ns, "registered a namespace on first read");
                }
                Ok(false) => {}
                Err(full) => tracing::warn!(
                    %ns, %full,
                    "not registering this namespace; its reads stay uncached"
                ),
            }
        }
        // This read is forwarded either way: registration has to establish a
        // watermark before anything here can be cached, and there is no
        // position to pin a fill at until it does.
        return forward(&state, &request, CacheState::Bypass, Some("untracked")).await;
    }
    // Warmth, for capture: a namespace being read is polled at the hot
    // interval, one that has gone quiet at the idle one.
    state.registry.note_read(&ns);

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
            decompose,
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
                decompose,
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
        // Learned framings need no invalidating — nothing a commit does changes
        // what `[` means — but "purge" should mean it, so they go too.
        state.framings.clear();
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
        Ok(upstream) => {
            let mut response = relay(state, upstream, None, cache_state, detail);
            record(
                state,
                request,
                response.headers_mut(),
                None,
                None,
                None,
                "the read was forwarded uncached",
            );
            response
        }
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
                // A streamed body is never cached, so nothing is tracking what
                // it depended on.
                record(
                    state,
                    &request,
                    headers,
                    None,
                    None,
                    None,
                    "the response was streamed rather than cached",
                );
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
    decompose: Decompose,
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
    let policy = state
        .engine
        .config()
        .policy
        .for_path(request.path_and_query.as_str())
        .clone();

    // Russian-doll composition applies only where the analyzer said the response
    // is a sequence of documents *and* the operator left it on. A pinned read is
    // excluded because it is immortal: there is no invalidation for a child to
    // absorb, so splitting it would be pure overhead.
    let spec: Option<ListSpec> = match (&decompose, policy.decompose, immutable) {
        (Decompose::Documents(spec), Decomposition::NdjsonList, false) => Some(spec.clone()),
        _ => None,
    };

    // A recording client wants the dependency set of every answer it reads, so
    // that a value it computes from them can be stored against the union. The
    // plan's terms are cloned only when asked for, because the common case is
    // nobody asking.
    let plan_terms: Option<Vec<Term>> = request
        .headers
        .contains_key(scope::SCOPE_HEADER)
        .then(|| terms.clone());

    // What this request's analysis added to the query, and therefore what has to
    // come back out of the response before a client sees it. Stored bodies keep
    // the injected ids — that is how a fragment is mapped back to documents, and
    // why `{Person{name}}` and `{Person{name _id}}` can share one entry — so the
    // removal is per request, on the way out, for hits and misses alike.
    let strip = rewrite
        .as_ref()
        .map(|rewrite| StripSpec::new(&rewrite.injected))
        .unwrap_or_default();

    // A caller may refuse a cached answer older than it can tolerate. Distinct
    // from the fence: `X-Reactor-Min-Version` is about *which commit* this node
    // has reached, this is about how long ago we looked. A caller that cares
    // about freshness in seconds rather than in commits has no commit id to
    // name.
    let max_staleness = request
        .headers
        .get("x-reactor-max-staleness")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_millis);

    if let Lookup::Hit(version) = state.engine.lookup(&key, at) {
        if let Some(body) = resolve_body(state, &version) {
            if policy.fresh_enough(version.age())
                && max_staleness.is_none_or(|bound| version.age() <= bound)
            {
                match composition(state, ns, &version, &body, spec.as_ref(), at, max_staleness) {
                    // Every child resolved from cache: no upstream request at
                    // all, which is the point of the whole feature.
                    Composition::Ready { body, hash } => {
                        // Recorded before stripping: the ids a GraphQL answer
                        // reveals are part of what it depends on, and stripping
                        // is precisely what takes them out of the bytes.
                        let recorded = plan_terms
                            .as_ref()
                            .map(|plan| record_terms(plan, augment, &body));
                        let Some(body) = deliverable(state, &strip, body) else {
                            return forward(
                                state,
                                request,
                                CacheState::Bypass,
                                Some("unstrippable"),
                            )
                            .await;
                        };
                        state.engine.metrics().lists_composed.inc();
                        state.engine.metrics().lookup_hits.inc();
                        let mut response = hit_response_tagged(
                            state,
                            &version,
                            hash,
                            body,
                            &watermark,
                            &request.headers,
                            Some("composed"),
                            &strip,
                        );
                        record(
                            state,
                            request,
                            response.headers_mut(),
                            Some(ns),
                            target.commit(),
                            recorded.as_deref(),
                            "a composed read could not be attributed",
                        );
                        return response;
                    }
                    // A plain, undecomposed entry. Serve the bytes.
                    Composition::Plain => {
                        let recorded = plan_terms
                            .as_ref()
                            .map(|plan| record_terms(plan, augment, &body));
                        let Some(body) = deliverable(state, &strip, body) else {
                            return forward(
                                state,
                                request,
                                CacheState::Bypass,
                                Some("unstrippable"),
                            )
                            .await;
                        };
                        state.engine.metrics().lookup_hits.inc();
                        let mut response = hit_response(
                            state,
                            &version,
                            body,
                            &watermark,
                            &request.headers,
                            &strip,
                        );
                        record(
                            state,
                            request,
                            response.headers_mut(),
                            Some(ns),
                            target.commit(),
                            recorded.as_deref(),
                            "a cached read could not be attributed",
                        );
                        return response;
                    }
                    // The parent is valid but some children are not: fill them
                    // all in ONE batched upstream request and splice.
                    Composition::Incomplete { spine, resolved } => {
                        return recompose(
                            state,
                            request,
                            ns,
                            target,
                            &key,
                            &watermark,
                            &version,
                            spine,
                            resolved,
                            spec.as_ref().expect("a spine implies a spec"),
                            augment,
                        )
                        .await;
                    }
                    // Decomposition was turned off while a spine was cached, so
                    // these bytes are a membership list and must never reach a
                    // client. Treat it as a miss and refill whole: that is what
                    // makes the switch an escape hatch rather than a trap.
                    Composition::StaleSpine => {
                        tracing::debug!(
                            "discarding a cached spine because decomposition is disabled"
                        );
                    }
                }
            }
        }
        // The body was evicted from the store, or the entry is past its TTL:
        // both are plain misses.
    }

    // Sharing, the other direction. A by-id read whose child fragment a list
    // read already warmed can be answered outright, as soon as we know how this
    // shape wraps one document — and framing is a property of the parameters,
    // not of the data, so one observation of the shape is enough. Without this
    // the warming would be one-way: the list would populate the child and the
    // by-id read would still pay an upstream request to learn its own wrapper.
    if let Some(response) = compose_single(
        state,
        ns,
        spec.as_ref(),
        at,
        &watermark,
        &key,
        request,
        plan_terms.as_deref(),
        augment,
        max_staleness,
    ) {
        return response;
    }

    // Miss: fill once, pinned, however many callers are waiting.
    let outcome = fill_now(
        state, request, ns, target, query, &key, &watermark, terms, immutable, rewrite, augment,
        spec,
    )
    .await;

    match outcome.as_ref().as_ref() {
        Ok(fill) => {
            let recorded = plan_terms
                .as_ref()
                .map(|plan| record_terms(plan, augment, &fill.body));
            let Some(body) = deliverable(state, &strip, fill.body.clone()) else {
                return forward(state, request, CacheState::Bypass, Some("unstrippable")).await;
            };
            let mut builder = Response::builder()
                .status(StatusCode::from_u16(fill.status).unwrap_or(StatusCode::OK));
            {
                let headers = builder.headers_mut().expect("a fresh builder has headers");
                proxy::copy_response_headers(&fill.headers, headers);
                if !strip.is_empty() {
                    // The origin counted bytes we just removed.
                    headers.remove(http::header::CONTENT_LENGTH);
                }
                decorate(
                    headers,
                    state,
                    Some(&watermark),
                    CacheState::Miss,
                    fill.version.is_none().then_some("uncacheable"),
                );
                // A fill that could not be stored is a fill whose dependencies
                // nobody is watching, so a value computed from it must not be
                // cached either.
                let recorded = fill.version.as_ref().and(recorded.as_deref());
                record(
                    state,
                    request,
                    headers,
                    Some(ns),
                    target.commit(),
                    recorded,
                    "the response could not be cached",
                );
            }
            builder
                .body(Body::from(body))
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

/// What resolving a cached parent against its children produced.
enum Composition {
    /// Not a decomposed entry: the bytes are the response.
    Plain,
    /// Composed from children with no upstream request.
    Ready {
        /// The spliced response.
        body: Bytes,
        /// Its identity, derived from the parent's and children's hashes.
        hash: ContentHash,
    },
    /// The parent is valid; these children still have to be fetched.
    Incomplete {
        /// The parent's framing and ordered membership.
        spine: Spine,
        /// One slot per member, `None` where the child is missing.
        resolved: Vec<Option<(Bytes, ContentHash)>>,
    },
    /// A spine is cached but decomposition is off, so it cannot be composed and
    /// must not be served.
    StaleSpine,
}

/// Resolve a cached parent against its children.
///
/// The hash is `combine(parent, children…)` rather than a hash of the spliced
/// bytes, for two reasons. It is cheaper — no second pass over the body — and it
/// is what makes backdating observable: the writer gives a refilled child whose
/// content did not move the *same* hash, so the combination is unchanged and a
/// client's `If-None-Match` still gets a 304.
fn composition(
    state: &AppState,
    ns: &Namespace,
    version: &FragmentVersion,
    body: &Bytes,
    spec: Option<&ListSpec>,
    at: Position,
    max_staleness: Option<Duration>,
) -> Composition {
    let Some(spine) = Spine::decode(body) else {
        // Not a spine. Either decomposition is off, or this key was filled
        // before it was turned on; either way the bytes are a response.
        return Composition::Plain;
    };
    let Some(spec) = spec else {
        return Composition::StaleSpine;
    };

    let mut resolved: Vec<Option<(Bytes, ContentHash)>> = Vec::with_capacity(spine.ids.len());
    let mut complete = true;
    for id in &spine.ids {
        let child = child_key(ns, id, spec);
        let found = match state.engine.lookup(&child, at) {
            // A child older than the caller's bound counts as *missing*, which
            // is exactly how an evicted one is treated: the composition becomes
            // incomplete and those children are refilled. A composed answer is
            // as old as its oldest part, so applying the bound to the spine
            // alone would answer "fresh" with stale bytes in it.
            Lookup::Hit(child) if max_staleness.is_none_or(|bound| child.age() <= bound) => {
                resolve_body(state, &child).map(|bytes| (bytes, child.hash))
            }
            Lookup::Hit(_) | Lookup::Miss => None,
        };
        complete &= found.is_some();
        resolved.push(found);
    }

    if !complete {
        return Composition::Incomplete { spine, resolved };
    }
    let parts: Vec<(Bytes, ContentHash)> = resolved
        .into_iter()
        .map(|slot| slot.expect("checked complete"))
        .collect();
    Composition::Ready {
        hash: composed_hash(version.hash, &parts),
        body: compose::compose(
            &spine.framing,
            &parts
                .iter()
                .map(|(bytes, _)| bytes.clone())
                .collect::<Vec<_>>(),
        ),
    }
}

/// The identity of a composed response: the parent's content hash combined with
/// its children's.
///
/// The children's *stored* hashes, not a rehash of the spliced bytes. That is
/// the backdating guarantee: the writer hands a refilled child the same hash
/// when its content did not move, so an unchanged refill leaves this value — and
/// therefore the client's `ETag` — alone.
fn composed_hash(spine_hash: ContentHash, parts: &[(Bytes, ContentHash)]) -> ContentHash {
    let mut hashes = Vec::with_capacity(parts.len() + 1);
    hashes.push(spine_hash);
    hashes.extend(parts.iter().map(|(_, hash)| *hash));
    ContentHash::combine(&mut hashes)
}

/// Answer a single-document read from its cached child plus a learned framing,
/// with no upstream request and no parent fragment.
///
/// Soundness, which is not obvious: the composed answer asserts both that the
/// document's bytes are these *and* that the document exists. The second half
/// comes free, because a delete fires `Doc(id)` and the child's only dependency
/// is `Doc(id)` — so a child that is a valid hit at this position is a document
/// that existed at this position. There is nothing left for a parent fragment to
/// contribute.
#[allow(clippy::too_many_arguments)]
fn compose_single(
    state: &AppState,
    ns: &Namespace,
    spec: Option<&ListSpec>,
    at: Position,
    watermark: &Watermark,
    parent_key: &FragmentKey,
    request: &UpstreamRequest,
    plan_terms: Option<&[Term]>,
    augment: Augment,
    max_staleness: Option<Duration>,
) -> Option<Response> {
    let spec = spec?;
    let id = spec.single()?;
    let framing = state.framings.get(ns, parent_key.shape)?;
    let child = match state.engine.lookup(&child_key(ns, id, spec), at) {
        Lookup::Hit(child) if max_staleness.is_none_or(|bound| child.age() <= bound) => child,
        // Too old for this caller, or absent: either way this shortcut cannot
        // answer, and the ordinary fill path can.
        Lookup::Hit(_) | Lookup::Miss => return None,
    };
    let bytes = resolve_body(state, &child)?;
    // Reconstruct the parent this read *would* have had, so the `ETag` is the
    // one a parent-composed answer gives. Two paths to the same bytes must not
    // produce two identities, or a client's `If-None-Match` would miss for no
    // reason the client can see.
    let spine = Spine {
        framing,
        ids: vec![id.clone()],
    };
    let spine_hash = ContentHash::of(&spine.encode());
    let parts = [(bytes, child.hash)];
    let body = compose::compose(
        &spine.framing,
        &parts
            .iter()
            .map(|(bytes, _)| bytes.clone())
            .collect::<Vec<_>>(),
    );
    state.engine.metrics().lists_composed.inc();
    let recorded = plan_terms.map(|plan| record_terms(plan, augment, &body));
    let mut response = hit_response_tagged(
        state,
        &child,
        composed_hash(spine_hash, &parts),
        body,
        watermark,
        &request.headers,
        Some("composed-shared"),
        // Documents, not GraphQL: nothing was injected into this request.
        &StripSpec::default(),
    );
    record(
        state,
        request,
        response.headers_mut(),
        Some(&parent_key.ns),
        // This path exists only for live-branch reads: `spec` is `None` for a
        // pinned one, and the function returns early without it.
        None,
        recorded.as_deref(),
        "a shared-child read could not be attributed",
    );
    Some(response)
}

/// The key of the child fragment holding one document of a decomposed response.
fn child_key(ns: &Namespace, id: &DocId, spec: &ListSpec) -> FragmentKey {
    FragmentKey {
        ns: ns.clone(),
        root: RootKey::Doc(id.clone()),
        shape: spec.child_shape,
    }
}

/// Fetch a version's bytes, wherever they live.
pub(crate) fn resolve_body(state: &AppState, version: &FragmentVersion) -> Option<Bytes> {
    match &version.body {
        BodyRef::Inline(bytes) => Some(bytes.clone()),
        // A miss here is eviction, not corruption: the fragment is simply
        // refilled.
        BodyRef::Cas(hash) => state.engine.store().get(hash),
    }
}

/// Response headers for a body we assembled rather than relayed.
fn headers_from_meta(meta: &ResponseMeta) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Some(content_type) = &meta.content_type {
        if let Some(value) = proxy::header_value(content_type) {
            headers.insert(http::header::CONTENT_TYPE, value);
        }
    }
    if let Some(data_version) = &meta.data_version {
        if let Some(value) = proxy::header_value(data_version) {
            headers.insert("terminusdb-data-version", value);
        }
    }
    headers
}

/// A valid parent with missing children: fetch them all in **one** request and
/// splice.
///
/// The batch is the whole point. A list of a thousand documents with a hundred
/// dirtied members must cost one upstream request, not a hundred: N+1 fan-out
/// against the node the cache exists to protect would dominate every other cost
/// in the system, and the hand-written prototype this replaces got that right.
///
/// Routed through the fill coordinator so the batch is single-flighted and takes
/// an upstream permit, exactly like an ordinary fill.
#[allow(clippy::too_many_arguments)]
async fn recompose(
    state: &AppState,
    request: &UpstreamRequest,
    ns: &Namespace,
    target: &Target,
    key: &FragmentKey,
    watermark: &Watermark,
    parent: &FragmentVersion,
    spine: Spine,
    resolved: Vec<Option<(Bytes, ContentHash)>>,
    spec: &ListSpec,
    augment: Augment,
) -> Response {
    let outcome = {
        let state_for_fill = state.clone_handle();
        let request = request.clone();
        let ns = ns.clone();
        let target = target.clone();
        let watermark = watermark.clone();
        let parent_hash = parent.hash;
        let meta = parent.meta.clone();
        let spec = spec.clone();
        state
            .fills
            .fill(key.clone(), watermark.pos, move || async move {
                execute_recompose(
                    state_for_fill,
                    request,
                    ns,
                    target,
                    watermark,
                    parent_hash,
                    meta,
                    spine,
                    resolved,
                    spec,
                    augment,
                )
                .await
            })
            .await
    };

    match outcome.as_ref().as_ref() {
        Ok(fill) => fill_response(state, fill, Some(watermark), "recomposed", &request.headers),
        Err(err) => {
            tracing::warn!(error = %err, "recomposition failed");
            bad_gateway(state, "recomposition failed")
        }
    }
}

/// Issue the one batched child fetch, store what came back, and splice.
#[allow(clippy::too_many_arguments)]
async fn execute_recompose(
    state: Arc<AppState>,
    request: UpstreamRequest,
    ns: Namespace,
    target: Target,
    watermark: Watermark,
    parent_hash: ContentHash,
    meta: ResponseMeta,
    spine: Spine,
    mut resolved: Vec<Option<(Bytes, ContentHash)>>,
    spec: ListSpec,
    augment: Augment,
) -> anyhow::Result<FillOutcome> {
    // Which documents are missing, deduplicated: one `?ids=` list, one request.
    let mut missing: Vec<DocId> = spine
        .ids
        .iter()
        .zip(&resolved)
        .filter(|(_, slot)| slot.is_none())
        .map(|(id, _)| id.clone())
        .collect();
    missing.sort();
    missing.dedup();

    let query = analyze::projection_query(&spec.projection, &missing);
    let Some(path) = pinned_path(&target, &watermark.commit, &query) else {
        return abandon(&state, &request, "unpinnable").await;
    };

    let mut pinned = request.clone();
    pinned.path_and_query = path;
    pinned.body = Bytes::new();
    pinned.headers.remove(http::header::CONTENT_LENGTH);

    state.engine.metrics().child_batches.inc();
    let upstream = state.proxy.send(&pinned).await?;
    if !upstream.is_success() {
        tracing::debug!(
            %ns, status = %upstream.status, path = %pinned.path_and_query,
            "batched child fetch was not successful"
        );
        return abandon(&state, &request, "child-fetch-failed").await;
    }

    let fetched = match compose::decompose(&upstream.body) {
        Ok(fetched) => fetched,
        Err(why) => {
            tracing::debug!(%ns, ?why, "batched child fetch did not split into documents");
            return abandon(&state, &request, "child-fetch-unsplittable").await;
        }
    };

    // The commit these child bytes are from, on the same terms as any other
    // fill: without a position we cannot say when they are valid, so we serve
    // them and store nothing.
    let placement = child_placement(&state, &ns, &watermark, &upstream);

    let child_meta = ResponseMeta {
        status: upstream.status.as_u16(),
        content_type: upstream.content_type(),
        data_version: upstream.data_version(),
    };

    let mut filled = 0u64;
    for member in &fetched.members {
        let mut deps = vec![Term::Doc(member.id.clone())];
        if augment == Augment::DocIds {
            // An unfolded document embeds others, and only its own bytes say
            // which. Per child, so a list no longer has to settle for the
            // `Everything` an unparseable NDJSON body forced.
            crate::analyze::augment_terms_from_response(&member.bytes, &mut deps);
        }
        let hash = ContentHash::of(&member.bytes);
        for (id, slot) in spine.ids.iter().zip(resolved.iter_mut()) {
            if slot.is_none() && id == &member.id {
                *slot = Some((member.bytes.clone(), hash));
            }
        }
        if let Some((position, commit)) = &placement {
            let stored = state.engine.send(
                &ns,
                crate::engine::WriterMsg::Insert(Box::new(crate::engine::FillResult {
                    key: child_key(&ns, &member.id, &spec),
                    at: *position,
                    commit: commit.clone(),
                    body: member.bytes.clone(),
                    meta: child_meta.clone(),
                    deps,
                    children: Vec::new(),
                    reply: None,
                })),
            );
            if stored.is_ok() {
                filled += 1;
            }
        }
    }
    state.engine.metrics().children_filled.add(filled);

    // Constraint: never serve a half-composed answer. A document the origin did
    // not return (deleted between the parent's fill and now, with the commit not
    // yet applied here) leaves a hole, and a hole is not an answer.
    let mut parts = Vec::with_capacity(resolved.len());
    for slot in resolved {
        match slot {
            Some(part) => parts.push(part),
            None => {
                tracing::debug!(%ns, "a child could not be resolved; forwarding whole");
                return abandon(&state, &request, "child-missing").await;
            }
        }
    }

    let body = compose::compose(
        &spine.framing,
        &parts
            .iter()
            .map(|(bytes, _)| bytes.clone())
            .collect::<Vec<_>>(),
    );
    state.engine.metrics().lists_composed.inc();
    Ok(FillOutcome {
        status: meta.status,
        headers: headers_from_meta(&meta),
        body,
        version: None,
        etag: Some(composed_hash(parent_hash, &parts)),
    })
}

/// Give up on composing and forward the client's own request instead.
///
/// The one rule this enforces: a composition that cannot be completed produces a
/// whole upstream fetch, never a partial body.
async fn abandon(
    state: &AppState,
    request: &UpstreamRequest,
    reason: &str,
) -> anyhow::Result<FillOutcome> {
    state.engine.metrics().compositions_abandoned.inc();
    let upstream = state.proxy.send(request).await?;
    Ok(uncacheable(upstream, reason))
}

/// Where a child fetch's bytes belong in this namespace's history.
///
/// `None` means we cannot place them, and therefore must not store them —
/// invariant 2. The bytes are still a correct answer at the pin, so the caller
/// serves them uncached rather than failing.
fn child_placement(
    state: &AppState,
    ns: &Namespace,
    watermark: &Watermark,
    upstream: &proxy::UpstreamResponse,
) -> Option<(Position, CommitId)> {
    match upstream.commit_id() {
        Some(returned) if returned != watermark.commit => {
            match state.engine.position_of(ns, &returned) {
                CommitLookup::Applied(pos) => Some((pos, returned)),
                CommitLookup::Unknown => None,
            }
        }
        Some(returned) => Some((watermark.pos, returned)),
        None => None,
    }
}

/// Turn a completed fill (or composition) into a client response.
fn fill_response(
    state: &AppState,
    fill: &FillOutcome,
    watermark: Option<&Watermark>,
    detail: &str,
    request_headers: &HeaderMap,
) -> Response {
    let etag = fill.etag.map(|hash| format!("\"{}\"", hash.to_hex()));
    if let Some(etag) = &etag {
        let matched = request_headers
            .get(http::header::IF_NONE_MATCH)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.split(',').any(|candidate| candidate.trim() == etag))
            .unwrap_or(false);
        if matched {
            let mut response = StatusCode::NOT_MODIFIED.into_response();
            decorate(
                response.headers_mut(),
                state,
                watermark,
                CacheState::Miss,
                Some(detail),
            );
            return response;
        }
    }

    let mut builder =
        Response::builder().status(StatusCode::from_u16(fill.status).unwrap_or(StatusCode::OK));
    {
        let headers = builder.headers_mut().expect("a fresh builder has headers");
        proxy::copy_response_headers(&fill.headers, headers);
        if let Some(etag) = &etag {
            if let Some(value) = proxy::header_value(etag) {
                headers.insert(http::header::ETAG, value);
            }
        }
        let detail = if fill.version.is_none() && fill.etag.is_none() {
            Some("uncacheable")
        } else {
            Some(detail)
        };
        decorate(headers, state, watermark, CacheState::Miss, detail);
    }
    builder
        .body(Body::from(fill.body.clone()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
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
    spec: Option<ListSpec>,
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
                spec,
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
    spec: Option<ListSpec>,
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
        // nothing. Every read endpoint does accept `/local/commit/<id>`, GraphQL
        // included (`tests/spikes.rs::spike_is_graphql_pinnable_by_commit`), so
        // in practice this branch is the absent-document case — but it stays,
        // because a 404 must not become a cached entry either.
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
                // An immutable answer is valid at *every* position, so it does
                // not need one: a commit-pinned read of a commit the ring has
                // already forgotten is still perfectly cacheable, and refusing
                // it would lose caching on exactly the oldest — and therefore
                // most reused — pinned reads. The watermark is used as `from`
                // only to place the entry on this namespace's axis; the commit
                // recorded is still the pinned one, so the entry explains
                // itself. Deps are empty for an immutable entry, so nothing can
                // close the interval either.
                CommitLookup::Unknown if immutable => (watermark.pos, returned.clone()),
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

    let meta = ResponseMeta {
        status: upstream.status.as_u16(),
        content_type: upstream.content_type(),
        data_version: upstream.data_version(),
    };

    // Decompose, if the analyzer said this is a list of documents and the bytes
    // bear that out. The response we return is still the origin's own body, so
    // the first read of a list is byte-identical by construction; only *later*
    // reads are composed, and `compose` reproduces exactly these bytes from the
    // same pieces.
    let mut spine_body: Option<(Bytes, Vec<FragmentKey>, Vec<Term>)> = None;
    if let Some(spec) = &spec {
        match compose::decompose(&body) {
            Ok(parts) => {
                state.engine.metrics().lists_decomposed.inc();
                if spec.single().is_some() {
                    // One member, so this wrapper fits any other single
                    // document of the same shape. See `FramingCache`: that is
                    // why only single-document shapes are remembered.
                    state.framings.remember(&ns, key.shape, &parts.framing);
                }
                let mut children = Vec::with_capacity(parts.members.len());
                for member in &parts.members {
                    let mut deps = vec![Term::Doc(member.id.clone())];
                    if augment == Augment::DocIds {
                        crate::analyze::augment_terms_from_response(&member.bytes, &mut deps);
                    }
                    let key = child_key(&ns, &member.id, spec);
                    children.push(key.clone());
                    // No reply channel: the parent's own insert is what the
                    // request waits on, and a child that the writer rejects
                    // simply costs a refill later.
                    state.engine.send(
                        &ns,
                        crate::engine::WriterMsg::Insert(Box::new(crate::engine::FillResult {
                            key,
                            at: position,
                            commit: commit.clone(),
                            body: member.bytes.clone(),
                            meta: meta.clone(),
                            deps,
                            children: Vec::new(),
                            reply: None,
                        })),
                    )?;
                }
                state
                    .engine
                    .metrics()
                    .children_filled
                    .add(parts.members.len() as u64);
                spine_body = Some((
                    Spine::of(&parts).encode(),
                    children,
                    spec.parent_terms.clone(),
                ));
            }
            Err(why) => {
                // Not a clean sequence of documents. Cache the whole response,
                // which is what Reactor did before decomposition existed.
                state.engine.metrics().lists_undecomposable.inc();
                tracing::debug!(%ns, ?why, "list response not decomposable; caching whole");
            }
        }
    }

    // Complete the read set from the response where the request alone could not
    // determine it. Skipped for a decomposed parent: its dependencies are
    // membership, and an unfolded document's embedded ids are the *children's*
    // business — which is also strictly better, because an NDJSON list body
    // does not parse as one JSON value and this call would have fired
    // `Everything`.
    if augment == Augment::DocIds && spine_body.is_none() {
        crate::analyze::augment_terms_from_response(&body, &mut terms);
    }

    let (stored_body, children, deps) = match spine_body {
        Some((spine, children, parent_terms)) => (spine, children, parent_terms),
        None => (
            body.clone(),
            Vec::new(),
            if immutable { Vec::new() } else { terms },
        ),
    };

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    let fill = crate::engine::FillResult {
        key,
        at: position,
        commit,
        body: stored_body,
        meta,
        deps,
        children,
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
        etag: None,
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
        etag: None,
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
        // The second argument is where an *unpinned* request would have been
        // pinned — the watermark. It must not win: passing the same commit for
        // both, as this test once did, cannot tell the two apart, and the
        // difference is a pinned read answered with the live state.
        assert_eq!(
            pinned_path(&target, &CommitId::new("later456"), ""),
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
