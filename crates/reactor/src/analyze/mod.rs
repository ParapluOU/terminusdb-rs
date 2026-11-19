//! Request analysis: what may be cached, and on what does it depend.
//!
//! This is the layer where soundness is decided. The engine will faithfully
//! invalidate whatever dependencies it is told about, which means a dependency
//! this module fails to emit is data that goes stale and never recovers. So the
//! rules here are deliberately lopsided:
//!
//! * **The default is `Bypass`.** An endpoint we do not recognize, a query
//!   parameter we have never seen, a body we cannot parse — all forward
//!   uncached. Invariant 6: that default must never invert.
//! * **Coarse beats clever.** Where a tight term cannot be *proved*, we emit a
//!   coarser one ([`Term::Class`], [`Term::Everything`]) or bypass. Over-
//!   invalidation is a performance bug; under-invalidation is a correctness bug.
//! * **Unknown means unknown.** With an empty [`SchemaIndex`] we do not guess
//!   whether a property is a value or a link; we emit terms that are sound under
//!   either reading.
//!
//! # Key normalization
//!
//! `/api/document/admin/mydb` and `/api/document/admin/mydb/local/branch/main`
//! are the same namespace and must share cache entries, so keys are never built
//! from the raw path. [`CanonicalRequest`] builds them from a normalized tuple
//! of `(endpoint, org, db, branch, recognized params, body hash)`.
//!
//! [`ShapeId`] is the same tuple with the *content-selecting* parameter values
//! (`id`, `ids`, `skip`, `count`, `id_prefix`) replaced by placeholders and the
//! namespace dropped, so "which projection" is separable from "of what". Nothing
//! in this phase depends on that separation — every plan is a whole-response
//! fragment — but decomposition and per-shape metrics will key off it, so it has
//! to be deterministic from the start.

mod document;
mod graphql;
mod woql;

pub use document::RECOGNIZED_DOCUMENT_PARAMS;
pub use graphql::{augment_terms_from_response, normalize_graphql, GraphQlAnalysis};
pub use woql::{woql_type_names, WoqlAnalysis};

use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::engine::{FragmentKey, RootKey};
use crate::ids::{ContentHash, DocId, Namespace, ShapeId};
use crate::schema_index::SchemaIndex;
use crate::term::Term;
use terminusdb_client::CommitId;

/// Everything the analyzer is allowed to see of a request.
///
/// Borrowed rather than owned because analysis happens on the request path and
/// must not copy the body. `target` is pre-parsed because the frontend needs the
/// namespace before it decides anything else (to look up the watermark).
pub struct ReqView<'a> {
    /// The HTTP method.
    pub method: &'a http::Method,
    /// The path, without the query string.
    pub path: &'a str,
    /// The raw query string, without the leading `?`.
    pub query: &'a str,
    /// Request headers.
    pub headers: &'a http::HeaderMap,
    /// The request body. Empty for a GET.
    pub body: &'a Bytes,
    /// Parsed from the path: which namespace, and whether it was commit-pinned.
    pub target: Target,
}

impl<'a> ReqView<'a> {
    /// Build a view, parsing the target from the path.
    pub fn new(
        method: &'a http::Method,
        path: &'a str,
        query: &'a str,
        headers: &'a http::HeaderMap,
        body: &'a Bytes,
    ) -> Self {
        let target = Target::parse(path);
        Self {
            method,
            path,
            query,
            headers,
            body,
            target,
        }
    }

    /// The namespace this request addresses, if it addresses one.
    pub fn namespace(&self) -> Option<&Namespace> {
        self.target.namespace()
    }

    /// Recognized query parameters, in the order they appeared.
    fn params(&self) -> Vec<(String, String)> {
        let mut params: Vec<(String, String)> = url::form_urlencoded::parse(self.query.as_bytes())
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        params.extend(self.overridden_body_params());
        params
    }

    /// Parameters from the JSON body of an overridden document read.
    ///
    /// `terminusdb-client` sends `{"id": "...", "unfold": false, …}` as the body
    /// of a `POST` with `X-HTTP-Method-Override: GET`, where a `GET` would have
    /// sent the same names in the query string. Lifting them into the parameter
    /// list means the whole document analyzer — the whitelist, the shape split,
    /// the `ListSpec` — works on the POST form without knowing it exists.
    ///
    /// Only for `document`: GraphQL and WOQL bodies are queries, not parameters,
    /// and have their own analyzers.
    fn overridden_body_params(&self) -> Vec<(String, String)> {
        if self.target.endpoint() != "document"
            || is_read_method(self.method)
            || !is_read_request(self)
        {
            return Vec::new();
        }
        let Ok(serde_json::Value::Object(fields)) =
            serde_json::from_slice::<serde_json::Value>(self.body)
        else {
            // Not an object: not the form we understand. An empty parameter
            // list leaves the request looking like a read of everything, and
            // the analyzer's whitelist and `ListSpec` rules take it from there.
            return Vec::new();
        };
        fields
            .into_iter()
            .map(|(name, value)| {
                let rendered = match value {
                    serde_json::Value::String(text) => text,
                    // `ids` arrives as a JSON array and has to stay one: the
                    // origin requires `?ids=["A","B"]`, and `parse_ids` reads
                    // exactly that.
                    other => other.to_string(),
                };
                (name, rendered)
            })
            .collect()
    }
}

/// What resource a path addresses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    /// `/api/<ep>/<org>/<db>/local/branch/<b>` or the bare `/<org>/<db>` form.
    Branch {
        /// The API endpoint name (`document`, `graphql`, `woql`, …).
        endpoint: String,
        /// The namespace addressed.
        ns: Namespace,
    },
    /// `/api/<ep>/<org>/<db>/local/commit/<id>` — immutable.
    Pinned {
        /// The API endpoint name.
        endpoint: String,
        /// The namespace whose history the commit belongs to.
        ns: Namespace,
        /// The pinned commit.
        commit: CommitId,
    },
    /// Anything else (system endpoints, `/api/db`, `/api/info`, …).
    Other {
        /// The API endpoint name, or the empty string if there was not one.
        endpoint: String,
    },
}

/// The branch a path without an explicit branch segment addresses.
const DEFAULT_BRANCH: &str = "main";

impl Target {
    /// Classify a path.
    ///
    /// Purely syntactic: it reports *which namespace a path names*, not whether
    /// that path is cacheable. `/api/db/admin/mydb` parses as a `Branch` even
    /// though creating a database is obviously not cacheable, because the
    /// resync signal for such endpoints needs the namespace. [`plan`] decides
    /// what to do with it.
    ///
    /// The single-segment `_system` forms (`/api/graphql/_system`) land in
    /// `Other`: the system database has a different schema and its own access
    /// rules, and it is not a namespace we track.
    pub fn parse(path: &str) -> Target {
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let rest: &[&str] = match segments.split_first() {
            Some((&"api", rest)) => rest,
            // Not an API path at all (`/healthz`, `/_reactor/...`, a bare
            // `/<org>/<db>`): we cannot name an endpoint, so it is not ours.
            _ => {
                return Target::Other {
                    endpoint: String::new(),
                }
            }
        };
        let (endpoint, rest) = match rest.split_first() {
            Some((ep, rest)) => ((*ep).to_owned(), rest),
            None => {
                return Target::Other {
                    endpoint: String::new(),
                }
            }
        };
        match rest {
            [org, db] => Target::Branch {
                endpoint,
                ns: Namespace::new(*org, *db, DEFAULT_BRANCH),
            },
            [org, db, "local", "branch", branch] => Target::Branch {
                endpoint,
                ns: Namespace::new(*org, *db, *branch),
            },
            // NOTE on the branch: `…/local/commit/<id>` carries no branch
            // segment, so there is nothing in the path to read one from — a
            // commit belongs to a repository, and may be reachable from several
            // branches or none. `DEFAULT_BRANCH` is therefore a placeholder, not
            // a claim. Cached content is unaffected (it is keyed by the commit
            // hash, which is what makes a pinned read immutable), and the
            // authorization gate only uses `org`/`db`; the one cost is that a
            // pinned read on a non-main branch is attributed to `main` in
            // per-namespace metrics. Fixing that would mean asking the server
            // which branches contain the commit, on the request path, to label
            // a metric — not worth it.
            [org, db, "local", "commit", commit] => Target::Pinned {
                endpoint,
                ns: Namespace::new(*org, *db, DEFAULT_BRANCH),
                commit: CommitId::new(*commit),
            },
            _ => Target::Other { endpoint },
        }
    }

    /// The endpoint name.
    pub fn endpoint(&self) -> &str {
        match self {
            Target::Branch { endpoint, .. }
            | Target::Pinned { endpoint, .. }
            | Target::Other { endpoint } => endpoint,
        }
    }

    /// The namespace, for the forms that name one.
    pub fn namespace(&self) -> Option<&Namespace> {
        match self {
            Target::Branch { ns, .. } | Target::Pinned { ns, .. } => Some(ns),
            Target::Other { .. } => None,
        }
    }

    /// The pinned commit, if this path is commit-pinned.
    pub fn commit(&self) -> Option<&CommitId> {
        match self {
            Target::Pinned { commit, .. } => Some(commit),
            _ => None,
        }
    }
}

/// How much cached state a topology call invalidates.
///
/// Branch-rewriting endpoints do not merely move a head forward — `reset`,
/// `rebase`, `squash` and `apply` can move it sideways, `pull` can land a
/// divergent history, and `optimize` may not move it at all while still
/// rewriting the layers underneath. Inferring a non-fast-forward from the
/// outside is possible but fragile, and the cost of getting it wrong is serving
/// pre-rewrite bytes. So these calls purge outright rather than relying on
/// inference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PurgeScope {
    /// Nothing is dropped; the head is merely re-examined.
    #[default]
    None,
    /// Drop everything cached for the branch named in the path.
    Branch,
    /// Drop everything cached for *every* branch of the database.
    ///
    /// For `DELETE /api/db`: the data is gone, so keeping any of it cached is
    /// indefensible.
    Database,
}

impl PurgeScope {
    /// True when anything is dropped.
    pub fn purges(self) -> bool {
        !matches!(self, PurgeScope::None)
    }
}

/// What the frontend should do with a request.
///
/// `Serialize` because `/_reactor/explain` returns a plan as JSON without
/// executing the request, and `X-Reactor-Explain` surfaces it on a live one.
/// "Why did this miss / why is this stale" has to be answerable from outside
/// the process.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Plan {
    /// Forward, cache nothing. Always safe.
    Bypass {
        /// Why, for `Cache-Status: ...; detail=` and for metrics.
        reason: &'static str,
    },
    /// A mutation: forward, then tell capture to expect a new commit.
    Write,
    /// Forward, and after a 2xx tell capture to resynchronize this namespace.
    ///
    /// Not in the original plan vocabulary. Branch-topology and migration
    /// endpoints (`/api/branch`, `/api/rebase`, `/api/squash`, `/api/reset`,
    /// `/api/apply`, `/api/pull`, `/api/migration`, …) are not writes in the
    /// ordinary sense: they can move the head somewhere that is not a descendant
    /// of our watermark, which is the non-fast-forward case. Calling that a
    /// `Write` would understate it, and calling it `Bypass` would lose the
    /// signal entirely.
    Resync {
        /// Why.
        reason: &'static str,
        /// The namespace to resynchronize, when the path named one.
        ns: Option<Namespace>,
        /// How much cached state this call invalidates.
        purge: PurgeScope,
    },
    /// Cacheable with these dependencies.
    Read {
        /// The cache key.
        key: FragmentKey,
        /// The read set.
        terms: Vec<Term>,
        /// True for a commit-pinned request: one correct answer, forever, on
        /// every replica that holds the commit. Never invalidated.
        immutable: bool,
        /// A rewritten upstream request, when we had to change it to make the
        /// response mappable to documents (GraphQL `_id`/`_type` injection).
        rewrite: Option<Rewrite>,
        /// Whether the response must be walked for ids to complete the read set.
        augment: Augment,
        /// Whether this response may be cached as a spine over per-document
        /// fragments instead of as one blob.
        decompose: Decompose,
    },
}

/// Whether a response may be cached as per-document fragments plus a spine.
///
/// Russian-doll decomposition, design doc §5.4 as revised by R4. The point is
/// that a field change to one member of a list must not throw the list away.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Default)]
pub enum Decompose {
    /// One fragment for the whole response. Always correct, and the fallback
    /// whenever the bytes turn out not to be a clean sequence of documents.
    #[default]
    No,
    /// The response is a sequence of `@id`-bearing documents joined by uniform
    /// framing: cache one child fragment per document, plus a parent holding
    /// only the ordered membership and the framing.
    Documents(ListSpec),
}

impl Decompose {
    /// The list spec, when decomposition applies.
    pub fn spec(&self) -> Option<&ListSpec> {
        match self {
            Decompose::Documents(spec) => Some(spec),
            Decompose::No => None,
        }
    }
}

/// Everything the serve path needs to decompose and recompose one list route.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ListSpec {
    /// The shape of a child fragment: the *projection*, with the selection and
    /// the framing factored out. Shared with a by-id read of the same
    /// projection, which is what makes a list read warm by-id reads.
    pub child_shape: ShapeId,
    /// The projection parameters, for the one batched `?ids=` request that
    /// refills every missing child.
    pub projection: Vec<(String, String)>,
    /// The **parent's** dependencies, which are membership only.
    ///
    /// Deliberately *not* the whole-response read set in
    /// [`Plan::Read::terms`]: that set contains [`Term::Class`], which fires on
    /// every field change to any instance. A parent that depended on it would
    /// be dirtied — and so refetched upstream — by exactly the change
    /// decomposition exists to absorb, which would defeat the entire feature
    /// while still looking correct. Soundness does not suffer: the parent's
    /// content is the framing plus the ordered membership, and nothing but a
    /// create or a delete can change that. A field update is absorbed by the
    /// child whose `Doc(id)` dependency does fire.
    ///
    /// If `/api/document` ever grows a value filter, an
    /// [`Term::Index`] belongs here too, because then membership would depend
    /// on a field.
    pub parent_terms: Vec<Term>,
    /// The documents the request named by id, if it named any. Empty for
    /// `?type=`, where only the origin knows the membership.
    pub named: Vec<DocId>,
}

impl ListSpec {
    /// The one document this request returns, when it returns exactly one.
    ///
    /// This is what makes a learned framing reusable: a one-member body's
    /// wrapper is the same whichever document is in it, so a by-id read whose
    /// child fragment is already cached — warmed by a list read, or by an
    /// earlier by-id read — can be answered with no upstream request at all. A
    /// multi-document selector cannot borrow a framing, because the origin's
    /// suffix after one member is not its suffix after three.
    pub fn single(&self) -> Option<&DocId> {
        match self.named.as_slice() {
            [only] => Some(only),
            _ => None,
        }
    }
}

impl Plan {
    /// True for a cacheable read.
    pub fn is_read(&self) -> bool {
        matches!(self, Plan::Read { .. })
    }

    /// The dependencies, for a read.
    pub fn terms(&self) -> &[Term] {
        match self {
            Plan::Read { terms, .. } => terms,
            _ => &[],
        }
    }

    /// Shorthand for a bypass.
    pub fn bypass(reason: &'static str) -> Plan {
        Plan::Bypass { reason }
    }
}

/// A request we must alter before forwarding it upstream.
///
/// Only GraphQL needs this, and only because the response has to be mappable
/// back to documents: without `_id` on every object we cannot tell which
/// document a response node came from, and the entry can only depend on whole
/// classes. Rewriting a *request* body is fine — we construct it ourselves —
/// whereas rewriting a *response* body is forbidden (invariant 1).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Rewrite {
    /// The body to send upstream in place of the client's.
    pub body: String,
    /// Where fields were injected, so the response can be stripped before it
    /// reaches the client. Phase D must not have to re-derive this.
    pub injected: Vec<InjectedField>,
}

/// One injected field, and the response path it has to be stripped from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InjectedField {
    /// Response path to the object whose selection set we added to, as response
    /// keys (aliases where the client used them), outermost first. Empty means
    /// the root `data` object.
    pub path: Vec<String>,
    /// The field name added (`_id` or `_type`).
    pub field: String,
}

/// Whether the response has to be walked to complete the read set.
///
/// Some read sets cannot be derived from the request alone. A GraphQL selection
/// says "some Person documents", not *which*; a document read with `unfold=true`
/// embeds documents whose identity is only known once we see the bytes. In both
/// cases the analyzer emits what it can prove up front and the serve path calls
/// [`augment_terms_from_response`] to add a `Doc` term per id actually returned.
/// That is what makes these entries entity-precise instead of class-coarse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Augment {
    /// The read set is complete as it stands.
    #[default]
    None,
    /// Walk the response for `_id`/`@id` values and add a `Doc` term for each.
    DocIds,
}

/// The header through which a client states when a cached answer dies.
///
/// `X-Reactor-Invalidate-On: class:Person, members:Company`
///
/// This exists because of a hard limit on what a proxy can infer. A WOQL query
/// can return data without revealing which entities the data was derived from —
/// `select ?n where { alice friend ?f . ?f name ?n }` returns a name and nothing
/// that identifies `?f` — so neither the response nor the AST tells us what the
/// answer depends on. Only the caller knows. See [`crate::analyze::woql`].
pub const INVALIDATE_ON_HEADER: &str = "x-reactor-invalidate-on";

/// The conditions a request stated, if any.
///
/// * `None` — the header was absent.
/// * `Some(Err(()))` — the header was present but did not parse. The caller
///   must **bypass**: a condition we misunderstood is worse than no condition,
///   because we would cache against a term that never fires.
/// * `Some(Ok(terms))` — use these as the dependency set.
#[allow(clippy::result_unit_err)]
pub fn stated_conditions(headers: &http::HeaderMap) -> Option<Result<Vec<Term>, ()>> {
    let raw = headers.get(INVALIDATE_ON_HEADER)?;
    let text = match raw.to_str() {
        Ok(text) => text,
        Err(_) => return Some(Err(())),
    };
    Some(Term::parse_list(text).ok_or(()))
}

/// Analyzer tuning.
#[derive(Clone, Debug)]
pub struct AnalyzeConfig {
    /// Refuse to cache anything we cannot analyze precisely, rather than
    /// falling back to a coarse term.
    ///
    /// Coarse terms are *correct* but can be expensive: one
    /// [`Term::Everything`] dependency means every commit in the namespace
    /// invalidates that entry. Strict mode trades those hit-rate cliffs for
    /// plain bypasses, which is the better default when you are measuring.
    pub strict: bool,
    /// Bodies larger than this are not parsed, and therefore not cached.
    pub max_body_bytes: usize,
}

impl Default for AnalyzeConfig {
    fn default() -> Self {
        Self {
            strict: false,
            max_body_bytes: 1024 * 1024,
        }
    }
}

/// Endpoints whose *pinned* form is immutable and safe to cache forever.
///
/// A request against `…/local/commit/<id>` has one correct answer on every
/// replica that holds the commit, so this is the highest-value, lowest-risk
/// case in the whole design.
const PINNABLE_READ_ENDPOINTS: &[&str] = &[
    "document", "graphql", "woql", "log", "changes", "diff", "schema", "prefixes", "history",
    "triples", "frame",
];

/// Endpoints that can move a branch head, including non-fast-forward.
const TOPOLOGY_ENDPOINTS: &[&str] = &[
    "db",
    "branch",
    "squash",
    "reset",
    "rebase",
    "apply",
    "pull",
    "fetch",
    "push",
    "clone",
    "optimize",
    "migration",
    "rollup",
    "unpack",
];

/// Endpoints that rewrite a branch's data or history, and therefore purge it.
///
/// `optimize` is in the list precisely because it may *not* move the head: it
/// rewrites the layers a commit is made of, so a head comparison would conclude
/// nothing happened while every cached body could be affected.
const BRANCH_REWRITING_ENDPOINTS: &[&str] = &[
    "squash",
    "reset",
    "rebase",
    "apply",
    "pull",
    "optimize",
    "migration",
];

/// How much a successful topology call invalidates.
///
/// `fetch`, `push` and `clone` are deliberately absent: none of them changes
/// the data of the branch named in the path (fetch writes remote-tracking
/// state, push and clone are outbound). They only warrant a resync.
fn purge_scope(endpoint: &str, method: &http::Method) -> PurgeScope {
    if BRANCH_REWRITING_ENDPOINTS.contains(&endpoint) {
        return PurgeScope::Branch;
    }
    // Deleting the thing is the most complete invalidation there is.
    if *method == http::Method::DELETE {
        return match endpoint {
            "db" => PurgeScope::Database,
            "branch" => PurgeScope::Branch,
            _ => PurgeScope::None,
        };
    }
    PurgeScope::None
}

/// Classify a request.
pub fn plan(req: &ReqView, schema: &SchemaIndex, cfg: &AnalyzeConfig) -> Plan {
    let endpoint = req.target.endpoint().to_owned();

    if req.body.len() > cfg.max_body_bytes {
        return Plan::bypass("request body too large to analyze");
    }

    // Commit-pinned reads first: nothing about them can change.
    if let Target::Pinned { ns, commit, .. } = &req.target {
        return plan_pinned(req, schema, cfg, &endpoint, ns, commit);
    }

    let ns = match req.target.namespace() {
        Some(ns) => ns.clone(),
        None => {
            // No namespace: a system or management route. Still worth a resync
            // signal if it can change a database's topology.
            if TOPOLOGY_ENDPOINTS.contains(&endpoint.as_str()) && !is_read_method(req.method) {
                // No namespace in the path, so there is nothing to purge by
                // name; capture re-examines whatever it tracks.
                return Plan::Resync {
                    reason: "topology endpoint",
                    ns: None,
                    purge: PurgeScope::None,
                };
            }
            return Plan::bypass("not a branch-scoped endpoint");
        }
    };

    if TOPOLOGY_ENDPOINTS.contains(&endpoint.as_str()) {
        if is_read_method(req.method) {
            return Plan::bypass("topology endpoint, read");
        }
        return Plan::Resync {
            reason: "topology endpoint",
            purge: purge_scope(&endpoint, req.method),
            ns: Some(ns),
        };
    }

    match endpoint.as_str() {
        "document" => document::plan(req, &ns, schema, cfg),
        "graphql" => graphql::plan(req, &ns, schema, cfg),
        "woql" => woql::plan(req, &ns, schema, cfg),
        // Invariant 6: the default must never invert.
        _ => Plan::bypass("unknown endpoint"),
    }
}

/// A commit-pinned request.
///
/// For `GET`-style endpoints this is unconditionally an immutable read. For
/// GraphQL and WOQL the body still has to be inspected, because a mutation or a
/// write query must never be cached even if it is addressed to a commit (the
/// server will reject it, and we must not memoize the rejection — invariant 7).
/// A read body, however, gets `immutable: true` and an empty dependency list:
/// at a fixed commit there is nothing for a term to depend on.
fn plan_pinned(
    req: &ReqView,
    schema: &SchemaIndex,
    cfg: &AnalyzeConfig,
    endpoint: &str,
    ns: &Namespace,
    commit: &CommitId,
) -> Plan {
    if !PINNABLE_READ_ENDPOINTS.contains(&endpoint) {
        return Plan::bypass("endpoint is not a pinnable read");
    }
    let body_bearing_read = matches!(endpoint, "graphql" | "woql");
    // An overridden read (`POST` + `X-HTTP-Method-Override: GET`) is a read, and
    // this is the path that matters most for it: a derivation's reads are all
    // commit-pinned, so without this an application-level cached value could
    // never be built out of typed client reads.
    if endpoint == "document" && is_read_request(req) {
        // Reuse the whole document analyzer — the whitelist, the terms, the
        // augmentation rules — and then make the result immutable.
        //
        // Two things are deliberate. The key is **commit-qualified**, because a
        // pinned entry is immortal and sharing a key with a live read would
        // serve a frozen answer forever. And the terms are **kept**, even
        // though a pinned entry stores no dependencies (see `serve`): they are
        // the logical read set, which is what a recording scope needs in order
        // to know what an application-level value computed from this read
        // depends on. Reporting nothing there would produce a derived value
        // that never expires.
        return match document::plan(req, ns, schema, cfg) {
            Plan::Read { terms, augment, .. } => {
                let canonical = CanonicalRequest::from_request(req, endpoint, ns);
                Plan::Read {
                    key: canonical.request_key(ns, Some(commit)),
                    terms,
                    immutable: true,
                    rewrite: None,
                    augment,
                    // A pinned answer is immortal, so there is no invalidation
                    // for a child to absorb.
                    decompose: Decompose::No,
                }
            }
            other => other,
        };
    }
    if !is_read_method(req.method) && !body_bearing_read {
        return Plan::bypass("write method against a pinned commit");
    }
    if body_bearing_read && !is_read_method(req.method) {
        // Inspect the body: a mutation stays a write even when pinned.
        let inner = match endpoint {
            "graphql" => graphql::plan(req, ns, schema, cfg),
            _ => woql::plan(req, ns, schema, cfg),
        };
        return match inner {
            Plan::Read {
                key,
                rewrite,
                augment,
                ..
            } => Plan::Read {
                key,
                // A pinned commit cannot change, so whatever the body reads,
                // the answer is fixed. The dependencies are dropped on purpose.
                terms: Vec::new(),
                immutable: true,
                rewrite,
                augment,
                // A pinned answer is immortal, so splitting it would buy
                // nothing: there is no invalidation for a child to absorb.
                decompose: Decompose::No,
            },
            other => other,
        };
    }
    let canonical = CanonicalRequest::from_request(req, endpoint, ns);
    Plan::Read {
        key: FragmentKey {
            ns: ns.clone(),
            root: RootKey::Request(canonical.root_hash(Some(commit))),
            shape: canonical.shape_id(),
        },
        terms: Vec::new(),
        immutable: true,
        rewrite: None,
        augment: Augment::None,
        decompose: Decompose::No,
    }
}

/// True for methods that cannot change server state.
pub fn is_read_method(method: &http::Method) -> bool {
    matches!(*method, http::Method::GET | http::Method::HEAD)
}

/// The header `terminusdb-client` uses to send a read as a POST.
pub const METHOD_OVERRIDE_HEADER: &str = "x-http-method-override";

/// Whether a request is a read, including one sent as `POST` with
/// `X-HTTP-Method-Override: GET`.
///
/// This matters more than it looks. `terminusdb-client` reads documents with a
/// POST and this header, because a lexical or hash key id cannot be
/// round-tripped through a query string — so **the workspace's own client, and
/// therefore the ORM, never issues the `GET /api/document` form at all**.
/// Without honouring the override, every typed read is classified as a write
/// and forwarded, and an application using the ORM gets a cache that reports
/// hits on nothing.
///
/// Only a `GET` override is honoured, and only towards *more* caution: it can
/// turn a POST into a read, never a GET into a write. The direction matters —
/// an attacker-supplied header must not be able to make Reactor treat a
/// mutation as cacheable, and `PUT`/`DELETE` overrides are therefore ignored
/// rather than obeyed.
pub fn is_read_request(req: &ReqView) -> bool {
    if is_read_method(req.method) {
        return true;
    }
    req.method == http::Method::POST
        && req
            .headers
            .get(METHOD_OVERRIDE_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.trim().eq_ignore_ascii_case("GET"))
            .unwrap_or(false)
}

/// Query-parameter names whose *value* selects which content is returned rather
/// than how it is shaped. Their values are placeholders in a [`ShapeId`].
const CONTENT_SELECTING_PARAMS: &[&str] = &["id", "ids", "skip", "count", "id_prefix"];

/// Document parameters that change how **one document** renders.
///
/// This is the *projection* half of the projection/framing split that
/// decomposition rests on (see [`crate::compose`]). A child fragment holds one
/// document's bytes, so its shape must depend on exactly these and on nothing
/// else — in particular not on `id`/`ids`/`type` (which document), nor on
/// `as_list`/`skip`/`count` (how documents are joined). That is what lets a
/// by-id read and a `?type=` list read share one child fragment, so each warms
/// the other.
///
/// `graph_type` is here because the schema graph renders entirely differently
/// from the instance graph. Deliberately absent: `as_list`, which the origin
/// applies as a wrapper around unchanged document bytes — an assumption
/// `the_per_document_rendering_does_not_depend_on_framing` checks against a
/// live node, because if it ever stopped holding, children could not be shared
/// across the two framings.
const PROJECTION_PARAMS: &[&str] = &[
    "unfold",
    "minimized",
    "compress_ids",
    "prefixed",
    "raw_json",
    "format",
    "identifiers",
    "raw_id",
    "xsd",
    "graph_type",
];

/// Build the `?ids=`-style query that fetches one document with the same
/// projection a list used, so a refilled child's bytes match the ones the list
/// was decomposed from.
///
/// Only projection parameters survive: `type`, `as_list`, `skip` and `count`
/// would change which documents come back or how they are wrapped, and the
/// caller splits the framing off anyway.
pub fn projection_query(params: &[(String, String)], ids: &[crate::ids::DocId]) -> String {
    let mut out = url::form_urlencoded::Serializer::new(String::new());
    for (name, value) in params {
        if PROJECTION_PARAMS.contains(&name.as_str()) {
            out.append_pair(name, value);
        }
    }
    if !ids.is_empty() {
        out.append_pair("ids", &render_ids(ids));
    }
    out.finish()
}

/// Render an `ids` query-parameter value.
///
/// MEASURED against 12.1.0-rc3: the document API wants a **JSON array** here,
/// not a comma-separated list. `?ids=Person/p1,Person/p2` is rejected with
/// `api:JSONInvalid` / `illegal_json`; `?ids=["Person/p1","Person/p2"]` returns
/// both documents. Pinned by `the_origin_wants_ids_as_a_json_array`.
pub fn render_ids(ids: &[crate::ids::DocId]) -> String {
    let array: Vec<serde_json::Value> = ids
        .iter()
        .map(|id| serde_json::Value::String(id.as_str().to_owned()))
        .collect();
    serde_json::Value::Array(array).to_string()
}

/// Parse an `ids` query-parameter value into document ids.
///
/// A JSON array is the form the server accepts (see [`render_ids`]), so it is
/// the form we must be able to read: getting this wrong is not a cosmetic bug
/// but a stale-forever one, because an entry keyed on `Doc("[\"Person/p1\"")`
/// depends on a term no commit can ever fire. The comma-separated fallback is
/// kept because it costs nothing and a coarser-but-parsed id is better than an
/// unparsed one.
pub fn parse_ids(raw: &str) -> Vec<DocId> {
    if let Ok(serde_json::Value::Array(items)) = serde_json::from_str::<serde_json::Value>(raw) {
        if items.iter().all(serde_json::Value::is_string) {
            return items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(DocId::parse)
                .collect();
        }
    }
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(DocId::parse)
        .collect()
}

/// The normalized form a cache key is built from.
///
/// Both hashes are over a length-prefixed encoding (`<len>:<value>` per field),
/// which is injective: no two distinct tuples can encode to the same string, so
/// two different requests cannot collide into one cache entry through a
/// separator ambiguity.
#[derive(Clone, Debug)]
pub struct CanonicalRequest {
    endpoint: String,
    org: String,
    db: String,
    branch: String,
    /// Recognized parameters only, sorted by name then value.
    params: Vec<(String, String)>,
    /// Hash of the request body, or of the empty string when there is none.
    body_hash: ContentHash,
    /// A normalized, value-free rendering of the body, when the analyzer
    /// produced one (GraphQL query text, WOQL canonical JSON). Used for the
    /// shape only.
    shape_body: Option<String>,
}

impl CanonicalRequest {
    /// Build from a request, keeping every parameter.
    ///
    /// Callers that have a parameter whitelist should use
    /// [`CanonicalRequest::with_params`] so an unrecognized parameter cannot
    /// silently vanish from the key.
    pub fn from_request(req: &ReqView, endpoint: &str, ns: &Namespace) -> Self {
        let params = req.params();
        Self::with_params(endpoint, ns, params, req.body, None)
    }

    /// Build from explicit parts.
    pub fn with_params(
        endpoint: &str,
        ns: &Namespace,
        mut params: Vec<(String, String)>,
        body: &Bytes,
        shape_body: Option<String>,
    ) -> Self {
        params.sort();
        Self {
            endpoint: endpoint.to_owned(),
            org: ns.org.clone(),
            db: ns.db.clone(),
            branch: ns.branch.clone(),
            params,
            body_hash: ContentHash::of(body),
            shape_body,
        }
    }

    /// The root hash: identity of *this* request, namespace included.
    ///
    /// `commit` is folded in for a pinned request so that the same query at two
    /// commits cannot share an entry.
    pub fn root_hash(&self, commit: Option<&CommitId>) -> ContentHash {
        let mut out = String::new();
        push_field(&mut out, "root");
        push_field(&mut out, &self.endpoint);
        push_field(&mut out, &self.org);
        push_field(&mut out, &self.db);
        push_field(&mut out, &self.branch);
        push_field(&mut out, commit.map(CommitId::as_str).unwrap_or(""));
        for (name, value) in &self.params {
            push_field(&mut out, name);
            push_field(&mut out, value);
        }
        push_field(&mut out, &self.body_hash.to_hex());
        ContentHash::of(out.as_bytes())
    }

    /// The shape id: "which projection", with ids, offsets and variable values
    /// factored out, and without the namespace — the same query against two
    /// branches is the same shape.
    pub fn shape_id(&self) -> ShapeId {
        let mut out = String::new();
        push_field(&mut out, "shape");
        push_field(&mut out, &self.endpoint);
        for (name, value) in &self.params {
            push_field(&mut out, name);
            if CONTENT_SELECTING_PARAMS.contains(&name.as_str()) {
                push_field(&mut out, "*");
            } else {
                push_field(&mut out, value);
            }
        }
        match &self.shape_body {
            Some(shape) => push_field(&mut out, shape),
            // No normalized rendering available: fall back to the body hash,
            // which is correct (two identical bodies have one shape) but
            // coarser than it could be.
            None => push_field(&mut out, &self.body_hash.to_hex()),
        }
        ShapeId::of(out.as_bytes())
    }

    /// A `FragmentKey` for a whole-response entry.
    pub fn request_key(&self, ns: &Namespace, commit: Option<&CommitId>) -> FragmentKey {
        FragmentKey {
            ns: ns.clone(),
            root: RootKey::Request(self.root_hash(commit)),
            shape: self.shape_id(),
        }
    }

    /// A `FragmentKey` rooted at one document, for a single-document read.
    ///
    /// Using the document as the root (rather than the request) is what lets two
    /// differently-spelled requests for the same document and projection share
    /// one entry, and is the hook decomposition will hang off.
    pub fn doc_key(&self, ns: &Namespace, id: DocId) -> FragmentKey {
        FragmentKey {
            ns: ns.clone(),
            root: RootKey::Doc(id),
            shape: self.shape_id(),
        }
    }

    /// The *projection* shape: which bytes one document renders as, with both
    /// the selection (`id`, `ids`, `type`) and the framing (`as_list`, `skip`,
    /// `count`) factored out.
    ///
    /// Distinct from [`CanonicalRequest::shape_id`] by construction — the two
    /// encodings start with different tags — so a child fragment can never
    /// collide with the whole-response entry for the same document.
    pub fn projection_shape_id(&self) -> ShapeId {
        let mut out = String::new();
        push_field(&mut out, "projection");
        push_field(&mut out, &self.endpoint);
        for (name, value) in &self.params {
            if PROJECTION_PARAMS.contains(&name.as_str()) {
                push_field(&mut out, name);
                push_field(&mut out, value);
            }
        }
        ShapeId::of(out.as_bytes())
    }

    /// The key of the child fragment holding one document of this response.
    pub fn child_key(&self, ns: &Namespace, id: DocId) -> FragmentKey {
        FragmentKey {
            ns: ns.clone(),
            root: RootKey::Doc(id),
            shape: self.projection_shape_id(),
        }
    }

    /// The projection parameters, for building a child refill request.
    pub fn projection_params(&self) -> Vec<(String, String)> {
        self.params
            .iter()
            .filter(|(name, _)| PROJECTION_PARAMS.contains(&name.as_str()))
            .cloned()
            .collect()
    }
}

/// Append one length-prefixed field to a canonical encoding.
fn push_field(out: &mut String, value: &str) {
    out.push_str(&value.len().to_string());
    out.push(':');
    out.push_str(value);
    out.push('|');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::schema_fixture;
    use http::{HeaderMap, Method};

    fn headers() -> HeaderMap {
        HeaderMap::new()
    }

    fn view<'a>(
        method: &'a Method,
        path: &'a str,
        query: &'a str,
        headers: &'a HeaderMap,
        body: &'a Bytes,
    ) -> ReqView<'a> {
        ReqView::new(method, path, query, headers, body)
    }

    // ------------------------------------------------------- target parsing

    #[test]
    fn target_parses_the_bare_org_db_form() {
        assert_eq!(
            Target::parse("/api/document/admin/mydb"),
            Target::Branch {
                endpoint: "document".into(),
                ns: Namespace::new("admin", "mydb", "main")
            }
        );
    }

    #[test]
    fn target_parses_the_explicit_branch_form() {
        assert_eq!(
            Target::parse("/api/document/admin/mydb/local/branch/feature"),
            Target::Branch {
                endpoint: "document".into(),
                ns: Namespace::new("admin", "mydb", "feature")
            }
        );
    }

    #[test]
    fn target_parses_the_pinned_commit_form() {
        let target = Target::parse("/api/woql/admin/mydb/local/commit/abc123");
        assert_eq!(
            target,
            Target::Pinned {
                endpoint: "woql".into(),
                ns: Namespace::new("admin", "mydb", "main"),
                commit: CommitId::new("abc123")
            }
        );
        assert_eq!(target.commit().map(CommitId::as_str), Some("abc123"));
    }

    #[test]
    fn target_parses_the_system_graphql_form_as_other() {
        let target = Target::parse("/api/graphql/_system");
        assert_eq!(
            target,
            Target::Other {
                endpoint: "graphql".into()
            }
        );
        assert_eq!(target.namespace(), None);
    }

    #[test]
    fn target_parses_a_bare_endpoint() {
        assert_eq!(
            Target::parse("/api/woql"),
            Target::Other {
                endpoint: "woql".into()
            }
        );
    }

    #[test]
    fn target_parses_garbage_as_other() {
        for path in [
            "",
            "/",
            "/healthz",
            "/_reactor/head",
            "/api",
            "/api/document/admin/mydb/local/nonsense/x",
            "/not/an/api/path/at/all",
        ] {
            assert!(
                matches!(Target::parse(path), Target::Other { .. }),
                "{path} should be Other"
            );
        }
    }

    #[test]
    fn target_tolerates_trailing_and_doubled_slashes() {
        assert_eq!(
            Target::parse("/api/document/admin/mydb/"),
            Target::parse("/api/document/admin//mydb")
        );
    }

    // ------------------------------------------------------- pinned reads

    #[test]
    fn pinned_reads_are_immutable_for_every_read_endpoint() {
        let body = Bytes::new();
        let h = headers();
        for endpoint in PINNABLE_READ_ENDPOINTS {
            if matches!(*endpoint, "graphql" | "woql") {
                continue; // body-bearing, covered separately
            }
            let path = format!("/api/{endpoint}/admin/mydb/local/commit/abc123");
            let req = view(&Method::GET, &path, "", &h, &body);
            match plan(&req, &schema_fixture(), &AnalyzeConfig::default()) {
                Plan::Read {
                    terms,
                    immutable,
                    key,
                    ..
                } => {
                    assert!(immutable, "{endpoint} pinned read must be immutable");
                    if *endpoint == "document" {
                        // The exception, and a deliberate one: see
                        // `a_pinned_document_read_keeps_its_logical_read_set`.
                        assert!(!terms.is_empty(), "document: the logical read set is kept");
                    } else {
                        assert!(terms.is_empty(), "{endpoint}: nothing can invalidate it");
                    }
                    assert_eq!(key.ns, Namespace::new("admin", "mydb", "main"));
                }
                other => panic!("{endpoint}: expected an immutable read, got {other:?}"),
            }
        }
    }

    /// A pinned read stores no dependencies — nothing can change under it — but
    /// it still *has* a read set, and a recording scope needs it: an
    /// application-level value computed out of pinned reads depends on exactly
    /// what those reads returned. Reporting an empty set there would produce a
    /// derived value that no commit ever invalidates.
    ///
    /// The terms are dropped where they would do harm (`serve`'s fill path
    /// stores `Vec::new()` for an immutable entry) rather than at the analyzer,
    /// which is the only place that knows what they are.
    #[test]
    fn a_pinned_document_read_keeps_its_logical_read_set() {
        let body = Bytes::new();
        let h = headers();
        let req = view(
            &Method::GET,
            "/api/document/admin/mydb/local/commit/abc123",
            "id=Person/alice",
            &h,
            &body,
        );
        match plan(&req, &schema_fixture(), &AnalyzeConfig::default()) {
            Plan::Read {
                terms, immutable, ..
            } => {
                assert!(immutable);
                assert_eq!(terms, vec![Term::Doc(DocId::parse("Person/alice"))]);
            }
            other => panic!("expected an immutable read, got {other:?}"),
        }
    }

    /// The form `terminusdb-client` actually sends. Without this the ORM's reads
    /// are classified as writes and the cache does nothing for an application
    /// that uses it.
    #[test]
    fn a_document_read_sent_as_a_post_override_is_a_read() {
        let body = Bytes::from(r#"{"id":"Person/alice","unfold":false}"#);
        let mut h = headers();
        h.insert(
            http::HeaderName::from_static(METHOD_OVERRIDE_HEADER),
            http::HeaderValue::from_static("GET"),
        );
        let req = view(&Method::POST, "/api/document/admin/mydb", "", &h, &body);
        match plan(&req, &schema_fixture(), &AnalyzeConfig::default()) {
            Plan::Read { terms, .. } => {
                assert_eq!(
                    terms,
                    vec![Term::Doc(DocId::parse("Person/alice"))],
                    "the id came out of the body"
                );
            }
            other => panic!("expected a read, got {other:?}"),
        }

        // Without the header it is a write, and must stay one.
        let bare = headers();
        let plain = view(&Method::POST, "/api/document/admin/mydb", "", &bare, &body);
        assert!(matches!(
            plan(&plain, &schema_fixture(), &AnalyzeConfig::default()),
            Plan::Write
        ));
    }

    /// An override may only make Reactor *more* careful. A header that turned a
    /// mutation into a cacheable read would be a hole anyone could reach.
    #[test]
    fn only_a_get_override_is_honoured() {
        let body = Bytes::from(r#"{"id":"Person/alice"}"#);
        for override_value in ["PUT", "DELETE", "POST", "get "] {
            let mut h = headers();
            h.insert(
                http::HeaderName::from_static(METHOD_OVERRIDE_HEADER),
                http::HeaderValue::from_str(override_value).unwrap(),
            );
            let req = view(&Method::POST, "/api/document/admin/mydb", "", &h, &body);
            let plan = plan(&req, &schema_fixture(), &AnalyzeConfig::default());
            if override_value.trim().eq_ignore_ascii_case("GET") {
                assert!(matches!(plan, Plan::Read { .. }), "{override_value}");
            } else {
                assert!(
                    matches!(plan, Plan::Write),
                    "{override_value} must not make a write cacheable, got {plan:?}"
                );
            }
        }
    }

    #[test]
    fn pinned_reads_at_different_commits_do_not_share_an_entry() {
        let body = Bytes::new();
        let h = headers();
        let keys: Vec<FragmentKey> = ["abc123", "def456"]
            .iter()
            .map(|commit| {
                let path = format!("/api/document/admin/mydb/local/commit/{commit}");
                let req = view(&Method::GET, &path, "id=Person/alice", &h, &body);
                match plan(&req, &schema_fixture(), &AnalyzeConfig::default()) {
                    Plan::Read { key, .. } => key,
                    other => panic!("expected a read, got {other:?}"),
                }
            })
            .collect();
        assert_ne!(keys[0], keys[1]);
        assert_eq!(keys[0].shape, keys[1].shape, "same projection, though");
    }

    #[test]
    fn a_write_method_against_a_pinned_commit_is_bypassed() {
        let body = Bytes::new();
        let h = headers();
        let req = view(
            &Method::POST,
            "/api/document/admin/mydb/local/commit/abc123",
            "",
            &h,
            &body,
        );
        assert!(matches!(
            plan(&req, &schema_fixture(), &AnalyzeConfig::default()),
            Plan::Bypass { .. }
        ));
    }

    #[test]
    fn an_unpinnable_endpoint_is_bypassed_even_when_pinned() {
        let body = Bytes::new();
        let h = headers();
        let req = view(
            &Method::GET,
            "/api/nonsense/admin/mydb/local/commit/abc123",
            "",
            &h,
            &body,
        );
        assert!(matches!(
            plan(&req, &schema_fixture(), &AnalyzeConfig::default()),
            Plan::Bypass { .. }
        ));
    }

    // ------------------------------------------------------------- routing

    #[test]
    fn unknown_endpoints_are_forwarded_never_cached() {
        let body = Bytes::new();
        let h = headers();
        for path in [
            "/api/info",
            "/api/ok",
            "/api/nonsense/admin/mydb",
            "/api/graphql/_system",
            "/healthz",
        ] {
            let req = view(&Method::GET, path, "", &h, &body);
            assert!(
                matches!(
                    plan(&req, &schema_fixture(), &AnalyzeConfig::default()),
                    Plan::Bypass { .. }
                ),
                "{path} must bypass"
            );
        }
    }

    #[test]
    fn every_branch_rewriting_endpoint_purges_the_branch() {
        // The user's list, verbatim. Each of these can change the data of the
        // branch in the path — `optimize` without even moving the head — so
        // inference from the head is not good enough.
        let body = Bytes::new();
        let h = headers();
        for endpoint in [
            "squash",
            "reset",
            "rebase",
            "apply",
            "pull",
            "optimize",
            "migration",
        ] {
            let path = format!("/api/{endpoint}/admin/mydb/local/branch/main");
            let req = view(&Method::POST, &path, "", &h, &body);
            match plan(&req, &schema_fixture(), &AnalyzeConfig::default()) {
                Plan::Resync { ns, purge, .. } => {
                    assert_eq!(
                        purge,
                        PurgeScope::Branch,
                        "{endpoint} must purge the branch"
                    );
                    assert_eq!(ns, Some(Namespace::new("admin", "mydb", "main")));
                }
                other => panic!("{endpoint}: expected a purging resync, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_read_against_a_topology_endpoint_purges_nothing() {
        let body = Bytes::new();
        let h = headers();
        for endpoint in [
            "squash",
            "reset",
            "rebase",
            "apply",
            "pull",
            "optimize",
            "migration",
            "db",
            "branch",
        ] {
            let path = format!("/api/{endpoint}/admin/mydb/local/branch/main");
            let req = view(&Method::GET, &path, "", &h, &body);
            let plan = plan(&req, &schema_fixture(), &AnalyzeConfig::default());
            assert!(
                matches!(plan, Plan::Bypass { .. }),
                "a GET against {endpoint} changes nothing, got {plan:?}"
            );
        }
    }

    #[test]
    fn the_outbound_and_metadata_endpoints_only_resync() {
        // None of these changes the data of the branch in the path: `fetch`
        // writes remote-tracking state, `push` and `clone` are outbound, and
        // creating a db or branch adds rather than rewrites.
        let body = Bytes::new();
        let h = headers();
        for endpoint in ["fetch", "push", "clone", "db", "branch"] {
            let path = format!("/api/{endpoint}/admin/mydb");
            let req = view(&Method::POST, &path, "", &h, &body);
            match plan(&req, &schema_fixture(), &AnalyzeConfig::default()) {
                Plan::Resync { purge, ns, .. } => {
                    assert_eq!(purge, PurgeScope::None, "{endpoint} must not purge");
                    assert_eq!(ns, Some(Namespace::new("admin", "mydb", "main")));
                }
                other => panic!("{endpoint}: expected a plain resync, got {other:?}"),
            }
        }
    }

    #[test]
    fn deleting_a_database_purges_every_branch_of_it() {
        let body = Bytes::new();
        let h = headers();
        let req = view(&Method::DELETE, "/api/db/admin/mydb", "", &h, &body);
        match plan(&req, &schema_fixture(), &AnalyzeConfig::default()) {
            Plan::Resync { purge, .. } => assert_eq!(
                purge,
                PurgeScope::Database,
                "the data is gone; caching any of it is indefensible"
            ),
            other => panic!("expected a purging resync, got {other:?}"),
        }
    }

    #[test]
    fn deleting_a_branch_purges_that_branch() {
        let body = Bytes::new();
        let h = headers();
        let req = view(
            &Method::DELETE,
            "/api/branch/admin/mydb/local/branch/feature",
            "",
            &h,
            &body,
        );
        match plan(&req, &schema_fixture(), &AnalyzeConfig::default()) {
            Plan::Resync { purge, ns, .. } => {
                assert_eq!(purge, PurgeScope::Branch);
                assert_eq!(ns, Some(Namespace::new("admin", "mydb", "feature")));
            }
            other => panic!("expected a purging resync, got {other:?}"),
        }
    }

    #[test]
    fn a_topology_read_is_a_plain_bypass() {
        let body = Bytes::new();
        let h = headers();
        let req = view(&Method::GET, "/api/db/admin/mydb", "", &h, &body);
        assert!(matches!(
            plan(&req, &schema_fixture(), &AnalyzeConfig::default()),
            Plan::Bypass { .. }
        ));
    }

    #[test]
    fn an_oversized_body_is_not_analyzed() {
        let body = Bytes::from(vec![b'x'; 64]);
        let h = headers();
        let req = view(&Method::POST, "/api/woql/admin/mydb", "", &h, &body);
        let cfg = AnalyzeConfig {
            max_body_bytes: 8,
            ..AnalyzeConfig::default()
        };
        assert!(matches!(
            plan(&req, &schema_fixture(), &cfg),
            Plan::Bypass { .. }
        ));
    }

    // -------------------------------------------------- key normalization

    #[test]
    fn the_two_spellings_of_a_namespace_share_a_root_key() {
        let body = Bytes::new();
        let h = headers();
        let keys: Vec<FragmentKey> = [
            "/api/document/admin/mydb",
            "/api/document/admin/mydb/local/branch/main",
        ]
        .iter()
        .map(|path| {
            let req = view(&Method::GET, path, "id=Person/alice", &h, &body);
            match plan(&req, &schema_fixture(), &AnalyzeConfig::default()) {
                Plan::Read { key, .. } => key,
                other => panic!("expected a read, got {other:?}"),
            }
        })
        .collect();
        assert_eq!(
            keys[0], keys[1],
            "the same namespace spelled two ways must share one cache entry"
        );
    }

    #[test]
    fn different_branches_do_not_share_a_root_key() {
        let body = Bytes::new();
        let h = headers();
        let keys: Vec<FragmentKey> = ["main", "feature"]
            .iter()
            .map(|branch| {
                let path = format!("/api/document/admin/mydb/local/branch/{branch}");
                let req = view(&Method::GET, &path, "id=Person/alice", &h, &body);
                match plan(&req, &schema_fixture(), &AnalyzeConfig::default()) {
                    Plan::Read { key, .. } => key,
                    other => panic!("expected a read, got {other:?}"),
                }
            })
            .collect();
        assert_ne!(keys[0], keys[1]);
    }

    #[test]
    fn parameter_order_does_not_change_the_key() {
        let ns = Namespace::new("admin", "mydb", "main");
        let body = Bytes::new();
        let a = CanonicalRequest::with_params(
            "document",
            &ns,
            vec![
                ("unfold".into(), "true".into()),
                ("id".into(), "P/1".into()),
            ],
            &body,
            None,
        );
        let b = CanonicalRequest::with_params(
            "document",
            &ns,
            vec![
                ("id".into(), "P/1".into()),
                ("unfold".into(), "true".into()),
            ],
            &body,
            None,
        );
        assert_eq!(a.root_hash(None), b.root_hash(None));
        assert_eq!(a.shape_id(), b.shape_id());
    }

    #[test]
    fn content_selecting_params_are_placeholders_in_the_shape() {
        let ns = Namespace::new("admin", "mydb", "main");
        let body = Bytes::new();
        let shape_of = |id: &str, unfold: &str| {
            CanonicalRequest::with_params(
                "document",
                &ns,
                vec![("id".into(), id.into()), ("unfold".into(), unfold.into())],
                &body,
                None,
            )
            .shape_id()
        };
        assert_eq!(
            shape_of("Person/alice", "true"),
            shape_of("Person/bob", "true"),
            "the id selects content, it does not shape it"
        );
        assert_ne!(
            shape_of("Person/alice", "true"),
            shape_of("Person/alice", "false"),
            "unfold changes the shape of the response"
        );
    }

    #[test]
    fn the_shape_ignores_the_namespace_but_the_root_does_not() {
        let body = Bytes::new();
        let one = CanonicalRequest::with_params(
            "graphql",
            &Namespace::new("admin", "one", "main"),
            vec![],
            &body,
            Some("query{Person{name}}".into()),
        );
        let two = CanonicalRequest::with_params(
            "graphql",
            &Namespace::new("admin", "two", "main"),
            vec![],
            &body,
            Some("query{Person{name}}".into()),
        );
        assert_eq!(one.shape_id(), two.shape_id());
        assert_ne!(one.root_hash(None), two.root_hash(None));
    }

    #[test]
    fn the_canonical_encoding_is_injective_across_field_boundaries() {
        // Without length prefixes, ("ab","c") and ("a","bc") would encode alike.
        let ns = Namespace::new("a", "bc", "main");
        let other = Namespace::new("ab", "c", "main");
        let body = Bytes::new();
        assert_ne!(
            CanonicalRequest::with_params("document", &ns, vec![], &body, None).root_hash(None),
            CanonicalRequest::with_params("document", &other, vec![], &body, None).root_hash(None)
        );
    }

    #[test]
    fn body_bytes_are_part_of_the_root_key() {
        let ns = Namespace::new("admin", "mydb", "main");
        let one =
            CanonicalRequest::with_params("woql", &ns, vec![], &Bytes::from_static(b"a"), None);
        let two =
            CanonicalRequest::with_params("woql", &ns, vec![], &Bytes::from_static(b"b"), None);
        assert_ne!(one.root_hash(None), two.root_hash(None));
    }

    #[test]
    fn plans_serialize_for_explain_output() {
        let body = Bytes::new();
        let h = headers();
        let req = view(
            &Method::GET,
            "/api/document/admin/mydb",
            "id=Person/alice",
            &h,
            &body,
        );
        let plan = plan(&req, &schema_fixture(), &AnalyzeConfig::default());
        let json = serde_json::to_value(&plan).expect("a plan must be explainable");
        assert_eq!(json["Read"]["immutable"], serde_json::json!(false));
        assert_eq!(
            json["Read"]["terms"][0]["Doc"],
            serde_json::json!("Person/alice")
        );
        // The hash is hex, not a 32-element byte array.
        assert!(json["Read"]["key"]["shape"].as_str().is_some());
    }

    #[test]
    fn bypass_and_resync_serialize_with_their_reasons() {
        let json = serde_json::to_value(Plan::bypass("because")).unwrap();
        assert_eq!(json["Bypass"]["reason"], serde_json::json!("because"));
        let json = serde_json::to_value(Plan::Resync {
            reason: "rebase",
            ns: None,
            purge: PurgeScope::Branch,
        })
        .unwrap();
        assert_eq!(json["Resync"]["reason"], serde_json::json!("rebase"));
        assert_eq!(json["Resync"]["purge"], serde_json::json!("Branch"));
    }
}
