//! `/_reactor/value/*` — application-computed values in the dependency graph.
//!
//! Everything else Reactor caches, it *derived itself*: a request came in, the
//! analyzer proved a read set, a fill fetched the bytes. This endpoint is the
//! other direction. An application computes something Reactor cannot see inside
//! — a rendered fragment, a projection, an aggregate over a transformation — and
//! hands it over with two facts attached: **the commit it was computed at**, and
//! **the terms its inputs depended on**.
//!
//! That is all the engine ever needed. [`crate::engine::FillResult`] says
//! nothing about HTTP; it says "these bytes, at this position, depending on
//! these terms". So a derived value goes through the same writer, the same
//! [`crate::engine::TermIndex`], the same validity intervals and — the part that
//! matters most — the same insert protocol: if a commit landed between the
//! caller's reads and this store, the entry is not rejected and not stale, it
//! gets a bounded validity interval and is reported back as `raced`.
//!
//! The two facts are the caller's to get right, and the client crate
//! (`terminusdb-cache`) exists so that they are not hand-written: it latches a
//! commit, issues every read pinned at it, and collects the `X-Reactor-Deps`
//! header off each response. What arrives here is the union.
//!
//! # Why this endpoint authenticates where the rest of `/_reactor/` does not
//!
//! `head`, `metrics` and `config` report Reactor's own state. A derived value is
//! *application data*, so it is gated exactly like a cached TerminusDB read:
//! the caller must present credentials the node accepts for this namespace.
//! Unlike the proxy path there is no upstream to defer an undecidable case to,
//! so `AuthDecision::Unknown` — which includes an anonymous request — is a
//! refusal rather than a pass-through. A client of this API authenticates even
//! against a public database.
//!
//! Note what that does and does not promise. It is "whoever may read this
//! namespace may read values derived from it". A derivation that mixes data from
//! several users is not separated by it, which is why the key is the caller's:
//! put the user in it.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};

use crate::engine::{
    CommitLookup, FillResult, ForeignRegistration, ForeignVerdict, Lookup, ResponseMeta, WriterMsg,
};
use crate::ids::{DerivedKey, Namespace};
use crate::term::{Dep, Term};
use terminusdb_client::CommitId;

use super::scope;
use super::serve::{decorate, record, resolve_body, CacheState};
use super::AppState;
use crate::frontend::proxy::UpstreamRequest;

/// The commit a value was computed at.
pub const AT_HEADER: &str = "x-reactor-at";

/// The terms a value's inputs depended on, comma-separated, or `nothing`.
///
/// Reactor sets it on a read to *report* a dependency set; a client sets it on a
/// store to *declare* one. Declaring is the fallback for a computation whose
/// reads Reactor cannot see; a recorded scope is the better path, because a
/// declaration can be wrong.
pub const DEPS_HEADER: &str = "x-reactor-deps";

/// The commit each *other* namespace's reads were pinned at.
///
/// `org/db/local/branch/main=<commit>,…`. A value composed from several
/// databases is not true at one commit — there is no such commit — so it
/// records which revision of each contributing history it was built from, and
/// Reactor checks each one against that namespace's own fired log. Set
/// automatically when the dependency set comes from a recorded scope; required
/// when foreign dependencies are *declared*, since a declaration carries no
/// pin of its own.
pub const FOREIGN_AT_HEADER: &str = "x-reactor-foreign-at";

/// Union this value's dependencies into a second scope as well.
///
/// Set by a *nested* derivation: the inner value is backed by its own scope, and
/// the outer computation depends on everything the inner one read. Until the
/// engine dirties a derived parent from its children, inheriting the terms is
/// what keeps the outer value from outliving the inner one's inputs.
pub const INTO_HEADER: &str = "x-reactor-into";

/// `?key=…`
#[derive(Debug, Deserialize)]
pub struct ValueParams {
    /// The caller's key. Opaque to Reactor beyond being non-empty.
    pub key: String,
}

/// What a store reports back.
#[derive(Debug, Serialize, PartialEq)]
pub struct StoreReport {
    /// Whether the stored version is the current one.
    ///
    /// `false` means the value was *accepted and immediately superseded*: it was
    /// correct at the commit it was computed at, a later commit has already
    /// invalidated it, and the next reader will recompute. Not an error, and
    /// worth counting rather than retrying.
    pub current: bool,
    /// Whether a commit invalidated the value between the caller's reads and
    /// this store. The race the insert protocol exists to make exactly solvable.
    pub raced: bool,
    /// The content hash of the stored bytes, as an `ETag` would quote it.
    pub etag: String,
    /// Position the value is valid from, as `epoch.seq`.
    pub from: String,
    /// Position it stops being valid at, if a commit has already closed it.
    pub to: Option<String>,
}

/// `GET /_reactor/value/{org}/{db}/{branch}?key=…`
///
/// A hit returns the stored bytes verbatim with an `ETag`; anything else is a
/// `404` with a `Cache-Status` saying which "anything else" it was. A miss is
/// not an error — it is the normal half of get-or-compute.
pub async fn get(
    State(state): State<Arc<AppState>>,
    Path((org, db, branch)): Path<(String, String, String)>,
    Query(params): Query<ValueParams>,
    headers: HeaderMap,
) -> Response {
    let ns = Namespace::new(org, db, branch);
    let Some(key) = DerivedKey::new(&params.key) else {
        return (StatusCode::BAD_REQUEST, "an empty key names nothing\n").into_response();
    };

    if !state
        .auth
        .allows(headers.get(http::header::AUTHORIZATION), &ns)
        .await
        .is_allowed()
    {
        return unauthorized();
    }
    let Some(watermark) = state.engine.watermark(&ns) else {
        return miss(&state, &ns, "no-watermark");
    };

    // A nested derivation must resolve its inner values *at its own snapshot*,
    // not at whatever the watermark has reached since: a value that is valid now
    // but was not valid at the outer computation's commit would make the outer
    // result a mixture of two points in history.
    let at = match header_text(&headers, AT_HEADER) {
        None => watermark.pos,
        Some(at) => match state.engine.position_of(&ns, &CommitId::new(at)) {
            CommitLookup::Applied(position) => position,
            CommitLookup::Unknown => return miss(&state, &ns, "unknown-commit"),
        },
    };

    let fragment = crate::engine::FragmentKey::derived(ns.clone(), key);
    let Lookup::Hit(version) = state.engine.lookup(&fragment, at) else {
        state.engine.metrics().derived_misses.inc();
        return miss(&state, &ns, "miss");
    };
    let policy = state.engine.config().policy.default_policy();
    if !policy.fresh_enough(version.age()) {
        state.engine.metrics().derived_misses.inc();
        return miss(&state, &ns, "ttl");
    }
    let Some(body) = resolve_body(&state, &version) else {
        // Evicted from the store. A miss, and the body will be recomputed.
        state.engine.metrics().derived_misses.inc();
        return miss(&state, &ns, "evicted");
    };

    state.engine.metrics().derived_hits.inc();
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(http::header::ETAG, format!("\"{}\"", version.hash.to_hex()))
        .header(http::header::AGE, version.age().as_secs())
        .body(Body::from(body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
    if let Some(content_type) = &version.meta.content_type {
        if let Ok(value) = http::HeaderValue::from_str(content_type) {
            response
                .headers_mut()
                .insert(http::header::CONTENT_TYPE, value);
        }
    }
    decorate(
        response.headers_mut(),
        &state,
        Some(&watermark),
        CacheState::Hit,
        None,
    );
    // A derived value's stored dependency set *is* its whole read set — nothing
    // about it is augmented from the bytes — so reporting it is exact, and it is
    // what lets an outer computation inherit an inner one's inputs.
    let pinned = header_text(&headers, AT_HEADER).map(CommitId::new);
    record(
        &state,
        &scope_request(&headers),
        response.headers_mut(),
        Some(&ns),
        pinned.as_ref(),
        Some(&version.deps),
        "a derived value was read without a pinned commit",
    );
    response
}

/// `POST /_reactor/value/{org}/{db}/{branch}?key=…`
///
/// Headers: `X-Reactor-At: <commit>` and `X-Reactor-Deps: <terms>` (or
/// `nothing`). Body: the value, verbatim.
///
/// Every refusal here is a refusal to cache something whose validity Reactor
/// could not establish, and each one names the reason, because a store that
/// silently did nothing would look exactly like a cache that works.
pub async fn put(
    State(state): State<Arc<AppState>>,
    Path((org, db, branch)): Path<(String, String, String)>,
    Query(params): Query<ValueParams>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let ns = Namespace::new(org, db, branch);
    let Some(key) = DerivedKey::new(&params.key) else {
        return (StatusCode::BAD_REQUEST, "an empty key names nothing\n").into_response();
    };

    if !state
        .auth
        .allows(headers.get(http::header::AUTHORIZATION), &ns)
        .await
        .is_allowed()
    {
        return unauthorized();
    }
    if !state.registry.tracks(&ns) {
        // Nothing watches this namespace's commits, so nothing could ever
        // invalidate the value. Storing it would be storing something stale.
        return (
            StatusCode::CONFLICT,
            format!("{ns} is not a tracked namespace, so a value stored here could never be invalidated\n"),
        )
            .into_response();
    }

    let Some(at) = header_text(&headers, AT_HEADER) else {
        return (
            StatusCode::BAD_REQUEST,
            format!("{AT_HEADER} must state the commit the value was computed at\n"),
        )
            .into_response();
    };
    let commit = CommitId::new(at);

    // Where the dependency set comes from. A recorded scope is the sound path —
    // Reactor watched the reads — and a declared list is the fallback for a
    // computation whose reads it could not see.
    let Some(mut foreign_at) = parse_foreign_at(&headers) else {
        return (
            StatusCode::BAD_REQUEST,
            format!(
                "{FOREIGN_AT_HEADER} must be `org/db/local/branch/<branch>=<commit>` pairs, \
                 comma separated\n"
            ),
        )
            .into_response();
    };

    let deps = match scope_label(&headers) {
        Some(id) => {
            // Taken, not copied: one scope backs one value, so a client cannot
            // record a few reads and then store a dozen values against them.
            let Some(record) = state.recorder.take(id) else {
                // Indistinguishable server-side from an expired scope, so the
                // message has to cover both — and lead with the likely one.
                return (
                    StatusCode::CONFLICT,
                    format!(
                        "no read was recorded in scope `{id}`, so a value derived from it has \
                         no dependencies to be invalidated by. Either the computation read \
                         nothing — say so with `{DEPS_HEADER}: nothing` — or its reads did not \
                         carry the {} header, or the scope expired.\n",
                        scope::SCOPE_HEADER
                    ),
                )
                    .into_response();
            };
            if let Some(why) = record.refusal(&ns, &commit) {
                return (StatusCode::CONFLICT, format!("{why}\n")).into_response();
            }
            // A recorded scope knows every pin it read at, so the caller does
            // not have to state them: the header is for the declared path.
            for other in record.namespaces() {
                if other == &ns {
                    continue;
                }
                match record.commit_for(other) {
                    Some(at) => {
                        foreign_at.insert(other.clone(), CommitId::new(at));
                    }
                    // `refusal` has already rejected several commits for one
                    // namespace, so this is the unpinned case, which it has
                    // also rejected. Belt and braces.
                    None => {
                        return (
                            StatusCode::CONFLICT,
                            format!("the reads of {other} were not pinned at one commit\n"),
                        )
                            .into_response()
                    }
                }
            }
            record.deps(&ns)
        }
        None => match parse_deps(&headers) {
            Some(deps) => deps,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    format!(
                        "state what the value depends on: either name a recorded scope with \
                         {} (the sound path — Reactor watched the reads), or list terms in \
                         {DEPS_HEADER}, or write `{DEPS_HEADER}: nothing` to declare the value \
                         independent of the database. An absent header is not an empty one: a \
                         value with no dependencies never expires, which is only ever right \
                         when you say so.\n",
                        scope::SCOPE_HEADER
                    ),
                )
                    .into_response()
            }
        },
    };

    if deps.iter().any(|dep| matches!(dep.term, Term::Everything)) {
        return (
            StatusCode::BAD_REQUEST,
            format!(
                "{DEPS_HEADER} names `everything`, so the value would be invalidated by the \
                 very next commit in {ns}. That is a sound dependency and a useless cache \
                 entry; it usually means a read in the computation was not tracked.\n"
            ),
        )
            .into_response();
    }

    let (deps, foreign) = partition(deps, &ns);

    // Every contributing namespace must be tracked and must have told us where
    // the value's reads of it sit in its history. Without that there is nothing
    // to check the race against and nothing that could fire an eviction, so the
    // value would be memoized against inputs nobody is watching.
    for other in foreign.keys() {
        if !state.registry.tracks(other) {
            return (
                StatusCode::CONFLICT,
                format!(
                    "this value depends on {other}, which is not tracked: nothing watches its \
                     commits, so the value could never be invalidated\n"
                ),
            )
                .into_response();
        }
        if !foreign_at.contains_key(other) {
            return (
                StatusCode::BAD_REQUEST,
                format!(
                    "this value depends on {other} but {FOREIGN_AT_HEADER} does not say which \
                     commit of it the value was computed from\n"
                ),
            )
                .into_response();
        }
    }

    let position = match state.engine.position_of(&ns, &commit) {
        CommitLookup::Applied(position) => position,
        CommitLookup::Unknown => {
            // We cannot place the value in time, so we cannot say when it stops
            // being true. The caller should re-read the head and recompute.
            return (
                StatusCode::CONFLICT,
                format!(
                    "commit {commit} is not applied on this node (or is no longer remembered), \
                     so a value computed at it cannot be placed in time\n"
                ),
            )
                .into_response();
        }
    };

    let limit = state.config.max_cacheable_body_bytes();
    let body = match axum::body::to_bytes(body, limit).await {
        Ok(body) => body,
        Err(_) => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("a derived value may not exceed {limit} bytes\n"),
            )
                .into_response()
        }
    };

    let content_type = header_text(&headers, http::header::CONTENT_TYPE.as_str())
        .map(str::to_owned)
        .or_else(|| Some("application/json".to_owned()));
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    let fragment = crate::engine::FragmentKey::derived(ns.clone(), key);
    let fill = FillResult {
        key: fragment.clone(),
        at: position,
        commit: commit.clone(),
        body,
        meta: ResponseMeta {
            status: 200,
            content_type,
            // A derived value is not a TerminusDB response, so there is no
            // data-version of its own to replay.
            data_version: None,
        },
        deps,
        // Nesting currently unions the inner value's terms into the outer one's
        // rather than recording an edge; see `terminusdb-cache`'s docs on why,
        // and what child-dirtying would buy.
        children: Vec::new(),
        reply: Some(reply_tx),
    };
    if let Err(err) = state.engine.send(&ns, WriterMsg::Insert(Box::new(fill))) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("could not reach the writer for {ns}: {err}\n"),
        )
            .into_response();
    }

    let Ok(version) = reply_rx.await else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("the writer for {ns} did not answer\n"),
        )
            .into_response();
    };
    state.engine.metrics().derived_stores.inc();

    // Now the cross-namespace half of the insert protocol. The value is
    // already in the graph, which is deliberate: each foreign writer checks its
    // own fired log *and* registers the entry in one message, so a commit there
    // is either before the check (reported here) or after the registration
    // (and fires an eviction). Doing it the other way round — register, then
    // insert — would let an eviction arrive for a key that does not exist yet
    // and be lost.
    let mut raced = version.to.is_some();
    for (other, terms) in foreign {
        let at = foreign_at
            .get(&other)
            .expect("checked above: every foreign namespace has a pin");
        let since = match state.engine.position_of(&other, at) {
            CommitLookup::Applied(position) => Some(position),
            // We no longer remember where that commit sits in its history, so
            // we cannot prove nothing relevant has happened since.
            CommitLookup::Unknown => None,
        };
        let verdict = match since {
            None => ForeignVerdict::Fired,
            Some(since) => {
                let (tx, rx) = tokio::sync::oneshot::channel();
                let registration = ForeignRegistration {
                    key: fragment.clone(),
                    from: version.from,
                    deps: terms,
                    since,
                    reply: tx,
                };
                match state
                    .engine
                    .send(&other, WriterMsg::ForeignRegister(Box::new(registration)))
                {
                    Ok(()) => rx.await.unwrap_or(ForeignVerdict::Fired),
                    // No writer there: nothing would ever evict this.
                    Err(_) => ForeignVerdict::Fired,
                }
            }
        };
        if verdict == ForeignVerdict::Fired {
            tracing::debug!(
                %ns, %other,
                "a commit in a contributing namespace landed between this value's reads and \
                 its store; dropping it"
            );
            state.engine.metrics().foreign_registrations_refused.inc();
            raced = true;
        }
    }
    if raced && version.to.is_none() {
        // Correct at the commit it was computed at, already superseded by a
        // commit somewhere it read. Drop it rather than leave it open: there
        // is no position in *this* namespace at which to close it.
        let _ = state.engine.send(&ns, WriterMsg::Evict(fragment.clone()));
    }
    if raced {
        state.engine.metrics().derived_stores_raced.inc();
    }
    // A nested derivation: the outer computation depends on everything this
    // inner one read.
    if let Some(outer) = header_text(&headers, INTO_HEADER) {
        state
            .recorder
            .record(outer, &ns, Some(&commit), &version.deps);
    }
    Json(StoreReport {
        current: version.is_open() && !raced,
        raced,
        etag: format!("\"{}\"", version.hash.to_hex()),
        from: version.from.to_string(),
        to: version.to.map(|to| to.to_string()),
    })
    .into_response()
}

/// `401`, with the reason stated once.
fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        "a derived value is application data: present credentials this node accepts for the \
         namespace. Unlike a proxied read there is no upstream to defer to, so an anonymous \
         request is refused even where the database is public.\n",
    )
        .into_response()
}

/// Build the request shim `record` needs.
///
/// `record` takes an [`UpstreamRequest`] because every other caller has one;
/// here only the headers exist, and only the scope label is read out of them.
fn scope_request(headers: &HeaderMap) -> UpstreamRequest {
    UpstreamRequest {
        method: http::Method::GET,
        path_and_query: String::new(),
        headers: headers.clone(),
        body: bytes::Bytes::new(),
    }
}

/// A `404` that says which kind of miss it was.
fn miss(state: &AppState, ns: &Namespace, detail: &str) -> Response {
    let mut response = (StatusCode::NOT_FOUND, "").into_response();
    let watermark = state.engine.watermark(ns);
    decorate(
        response.headers_mut(),
        state,
        watermark.as_ref(),
        CacheState::Miss,
        Some(detail),
    );
    response
}

/// The scope label on a request, if it carries one.
fn scope_label(headers: &HeaderMap) -> Option<&str> {
    header_text(headers, scope::SCOPE_HEADER)
}

fn header_text<'h>(headers: &'h HeaderMap, name: &str) -> Option<&'h str> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

/// Parse the dependency header.
///
/// `nothing` is the one way to say "this value does not depend on the database",
/// and it has to be said out loud: an absent header would otherwise parse as an
/// empty dependency set, which is an entry that never expires. All-or-nothing
/// on the terms themselves, via [`Term::parse_list`] — a partially understood
/// dependency set is not a smaller one, it is a wrong one.
fn parse_deps(headers: &HeaderMap) -> Option<Vec<Dep>> {
    let text = header_text(headers, DEPS_HEADER)?;
    if text.eq_ignore_ascii_case("nothing") {
        return Some(Vec::new());
    }
    Dep::parse_list(text)
}

/// Parse `X-Reactor-Foreign-At`.
///
/// `None` is "the header was malformed", which must refuse rather than quietly
/// mean "no foreign pins" — the two differ by whether a composition gets
/// checked for the race at all.
fn parse_foreign_at(headers: &HeaderMap) -> Option<BTreeMap<Namespace, CommitId>> {
    let Some(text) = header_text(headers, FOREIGN_AT_HEADER) else {
        return Some(BTreeMap::new());
    };
    let mut out = BTreeMap::new();
    for element in text.split(',') {
        let element = element.trim();
        if element.is_empty() {
            continue;
        }
        let (path, commit) = element.split_once('=')?;
        let ns = Namespace::from_path(path.trim())?;
        let commit = commit.trim();
        if commit.is_empty() {
            return None;
        }
        out.insert(ns, CommitId::new(commit));
    }
    Some(out)
}

/// Split a dependency set into this namespace's terms and everyone else's.
fn partition(deps: Vec<Dep>, home: &Namespace) -> (Vec<Term>, BTreeMap<Namespace, Vec<Term>>) {
    let mut local = Vec::new();
    let mut foreign: BTreeMap<Namespace, Vec<Term>> = BTreeMap::new();
    for dep in deps {
        match dep.ns {
            Some(ns) if &ns != home => foreign.entry(ns).or_default().push(dep.term),
            _ => local.push(dep.term),
        }
    }
    (local, foreign)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{ClassName, DocId};

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.insert(
                http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                http::HeaderValue::from_str(value).unwrap(),
            );
        }
        headers
    }

    #[test]
    fn an_absent_dependency_header_is_not_an_empty_one() {
        assert_eq!(parse_deps(&headers(&[])), None);
        assert_eq!(parse_deps(&headers(&[(DEPS_HEADER, "   ")])), None);
    }

    #[test]
    fn independence_has_to_be_said_out_loud() {
        assert_eq!(
            parse_deps(&headers(&[(DEPS_HEADER, "nothing")])),
            Some(vec![])
        );
        assert_eq!(
            parse_deps(&headers(&[(DEPS_HEADER, "NOTHING")])),
            Some(vec![])
        );
    }

    #[test]
    fn terms_round_trip_through_the_header() {
        let deps = parse_deps(&headers(&[(
            DEPS_HEADER,
            "doc:Person/alice, members:Person, classfield:Person.name",
        )]))
        .expect("all three parse");
        // `parse_list` normalizes the order, so compare as a set.
        assert_eq!(deps.len(), 3);
        for expected in [
            Term::Doc(DocId::parse("Person/alice")),
            Term::Members(ClassName::new("Person")),
            Term::ClassField(ClassName::new("Person"), crate::ids::PropName::new("name")),
        ] {
            assert!(
                deps.contains(&Dep::home(expected.clone())),
                "{expected} missing from {deps:?}"
            );
        }
    }

    #[test]
    fn a_dependency_may_name_another_namespace() {
        let deps = parse_deps(&headers(&[(
            DEPS_HEADER,
            "doc:Person/alice, admin/shared/local/branch/main#doc:Chunk/intro",
        )]))
        .expect("both parse");
        let home = Namespace::new("admin", "room-42", "main");
        let (local, foreign) = partition(deps, &home);
        assert_eq!(local, vec![Term::Doc(DocId::parse("Person/alice"))]);
        assert_eq!(
            foreign.get(&Namespace::new("admin", "shared", "main")),
            Some(&vec![Term::Doc(DocId::parse("Chunk/intro"))])
        );
    }

    #[test]
    fn a_dependency_on_the_home_namespace_is_local_however_it_is_written() {
        let home = Namespace::new("admin", "room-42", "main");
        let deps = parse_deps(&headers(&[(
            DEPS_HEADER,
            "admin/room-42/local/branch/main#doc:Person/alice",
        )]))
        .expect("it parses");
        let (local, foreign) = partition(deps, &home);
        assert_eq!(local, vec![Term::Doc(DocId::parse("Person/alice"))]);
        assert!(
            foreign.is_empty(),
            "naming your own namespace is not foreign"
        );
    }

    #[test]
    fn foreign_pins_parse_and_a_malformed_one_refuses() {
        let pins = parse_foreign_at(&headers(&[(
            FOREIGN_AT_HEADER,
            "admin/shared/local/branch/main=abc123, admin/other/local/branch/main=def456",
        )]))
        .expect("both parse");
        assert_eq!(
            pins.get(&Namespace::new("admin", "shared", "main"))
                .map(CommitId::as_str),
            Some("abc123")
        );
        assert_eq!(pins.len(), 2);

        // Absent is an empty map; malformed is a refusal. The difference
        // decides whether a composition gets checked for the race at all.
        assert_eq!(
            parse_foreign_at(&HeaderMap::new()).map(|m| m.len()),
            Some(0)
        );
        assert_eq!(
            parse_foreign_at(&headers(&[(FOREIGN_AT_HEADER, "admin/shared=abc")])),
            None
        );
        assert_eq!(
            parse_foreign_at(&headers(&[(
                FOREIGN_AT_HEADER,
                "admin/shared/local/branch/main"
            )])),
            None
        );
    }

    #[test]
    fn one_unparseable_term_fails_the_whole_list() {
        assert_eq!(
            parse_deps(&headers(&[(
                DEPS_HEADER,
                "doc:Person/alice, nonsense:whatever"
            )])),
            None,
            "a partially understood dependency set is a wrong one"
        );
    }

    #[test]
    fn an_empty_key_names_nothing() {
        assert!(DerivedKey::new("").is_none());
        assert!(DerivedKey::new("   ").is_none());
        assert_eq!(
            DerivedKey::new(" overview/q4 ").map(|k| k.as_str().to_owned()),
            Some("overview/q4".to_owned())
        );
    }

    #[test]
    fn a_store_report_says_whether_the_value_survived_the_race() {
        let report = StoreReport {
            current: false,
            raced: true,
            etag: "\"abc\"".to_owned(),
            from: "0.1".to_owned(),
            to: Some("0.2".to_owned()),
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["current"], serde_json::json!(false));
        assert_eq!(json["raced"], serde_json::json!(true));
        assert_eq!(json["to"], serde_json::json!("0.2"));
    }
}
