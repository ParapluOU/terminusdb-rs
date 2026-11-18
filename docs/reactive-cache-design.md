# A reactive, graph-tracked cache engine for TerminusDB 12

*Design proposal — 2026-11-18*

Working name: **Reactor** (`crates/reactor`, binary `tdb-reactor`). It is a
server-side wrapper around the TerminusDB 12 HTTP API that forwards every
request, maintains a dependency graph of what every cached answer was derived
from, and invalidates/recomputes fragments from the upstream commit stream.

It is, deliberately, a Rust re-make of the
[SkipLabs Skip](https://skiplabs.github.io/skip/docs/introduction) reactive-cache
model with TerminusDB as the data source, and a proper second attempt at the
prototype in `pubbin/reference-impl/proxy/proxy/lib/module/cache`.

---

## 1. Executive summary

| | |
|---|---|
| **What** | A reverse proxy in front of TerminusDB that caches `/api/document`, `/api/graphql`, `/api/woql` and friends as a composable graph of *fragments*, and invalidates them from `_ChangeSet` GraphQL subscriptions + per-commit diffs. |
| **Why now** | TDB 12.1 ships the two primitives the 2023/24 prototype lacked: real mutation subscriptions (`_ChangeSet` over SSE, batched per commit, with commit metadata and pre-commit resolution of deletes) and a usable history/diff/changes API. |
| **Core model** | Skip's *reactive cache*: a demanded-computation graph of memoized, side-effect-free derivations over input cells, under MVCC revisions — with **TerminusDB commit ids as the revision counter**, which Skip has to invent and we get for free. |
| **Core bet** | Cache *fragments keyed by (document, shape, commit)*, not responses. Responses are assembled by composition (russian doll). A content hash per fragment gives us Skip's "backdating": a change that does not alter the projected shape stops propagating. |
| **Key correctness lever** | Every request to a node goes *through* its Reactor, so we see each write's `TerminusDB-Data-Version` synchronously. That buys read-your-writes and monotonic reads on that node via watermark fencing — the thing a pure CDN-style GraphQL cache cannot do. |
| **Scale model** | TerminusDB is scaled horizontally as eventually-consistent replicas. One Reactor per node, sidecar, tracking only that node's head. **Reactor's guarantee is relative: never weaker than the node it fronts** (§5.5). Cached fragments are immutable and content-addressed, so nodes may share a cache tier or keep entirely private views with no coherence protocol either way (§5.7). |
| **Biggest free win** | Any request pinned to a commit (`/api/.../local/commit/<id>`) is **immutable** and cacheable forever with zero invalidation risk — and, because TDB commit ids are content-addressed, valid on *every* replica holding that commit. Milestone 2 ships value before any dependency tracking exists. |
| **Recommendation** | Build the engine in Rust (Option C in §8), reusing `terminusdb-client` for the fill/capture path and raw byte proxying for passthrough. Do *not* port Skiplang. Optionally ship a ~200-line Skip `ExternalService` adapter as a model-validation side quest. |

---

## 2. What the old prototype did, and what it got wrong

Files: `reference-impl/proxy/proxy/lib/module/cache/src/lib.rs` (3261 lines),
`module/cache-macros`, and the consumers in
`instance/aws/src/{cache/*,cache_impl.rs,backend/cache.rs,background/cache_*.rs}`.

### The shape of it

- **A declarative condition algebra.** `Condition::{AnyChanged, CountChanged, Field, Related, Ttl, DerivedFromTrait, DerivedFromClosure}` (`cache/src/lib.rs:430`). Cache sites declared *when* an entry dies, decoupled from the code that mutates data — the right instinct, and the thing to keep.
- **Russian-doll composition, by hand.** `backend/cache.rs:958` `document_get_cached` is the outer entry; on a miss it calls `document_get_from_nested_caches`, which composes `PublicationChunkListCache` (`cache/chunk_list.rs`) over N `ChunkContentCache` entries (`cache/chunk_content.rs`), batching the misses into one `get_many` and warming the inner layer. That is exactly the right data flow, hand-written once per route.
- **Derived conditions.** `derived!` / `CacheableWithContext` let the invalidation set be computed *from the fetched value* — e.g. `standard_get` derives a condition per referenced chunk (`cache/standard_get.rs`). This is the prototype's best idea: a read-set discovered at fill time.
- **Change capture.** `background/cache_change_listener.rs` registered typed `on_added_batch` / `on_changed_batch` / `on_deleted` callbacks per model and funnelled them into `CACHE.notify_change`. It was built on the `sse_manager` changeset plugin — our own unstable, now-deprecated server plugin.
- **Storage.** One JSON file per entry, key = SHA256, plus an in-memory inverted index `TypeId → [keys]` *and* `model_name → [keys]` (`ConditionIndex`, `cache/src/lib.rs:898`). Shared between instances by **committing the cache directory to a git branch** (`cache/src/git_sync.rs`).

### Where it broke — and what each failure teaches

1. **Change capture was the weak link.** It depended on our own SSE plugin, which is unmaintained and disabled in docker (see `changeset-sse-deprecated`). *Fix: `_ChangeSet` over SSE is now a first-class server feature, plus `/api/changes` and `/api/diff` as the authoritative, replayable backstop.*
2. **No notion of "as of which commit".** Entries carried `created_at: DateTime<Utc>` and nothing else. An entry could be filled from a read that raced a concurrent commit and be silently wrong forever; and a client that had just written could read its own stale data. *Fix: every fragment is tagged with the commit it was computed at, fills are **pinned** to that commit, and reads are fenced against a watermark.*
3. **Invalidation matched on strings, so it over- and under-fired.** The tell is `cache/review_state.rs:47`:
   ```rust
   vec![
       field!(Annotation, "documentId" => chunk_id.to_string()),
       field!(Annotation, "documentId" => chunk_id.to_address().to_string()),
       field!(Annotation, "documentId" => chunk_id.id().to_string()),
   ]
   ```
   three spellings of one id, because nobody could tell which form the change event would carry. And `Field` conditions on non-id fields only fired if the event happened to carry full model JSON, else they fell back to "was this field name in the changed list" (`cache/src/lib.rs` `should_invalidate`). *Fix: dependency terms are typed and built from `EntityIDFor`/`TdbIRI`, never from `format!` (see `no-hand-formatted-iris`), and the firing side derives terms from a real before/after patch, not from a field-name list.*
4. **A reverse-edge query (`Annotation.documentId == chunk`) was expressed as a value predicate.** There was no graph index, so "the set of annotations pointing at this chunk" could not be a dependency — only "some annotation whose documentId field equals this string changed" could. *Fix: first-class `Backlink` and `Index` terms, computed from the mirror/patch.*
5. **`TypeId` could not survive a restart,** hence the duplicate `by_model_name` index and `Box::leak`ed `&'static str`s reconstructed from strings (`StoredCondition::to_runtime_condition`, `cache/src/lib.rs:711`). *Fix: terms are plain serializable data; types are a derive-time convenience, not the identity.*
6. **Invalidation, not recomputation.** A commit deleted entries; the next reader paid full price. For a 200-chunk publication where one chunk changed, the outer entry died and was rebuilt from the inner layer — good — but only because someone wrote `document_get_from_nested_caches` by hand. *Fix: composition and eager recomputation are engine features, declared per resource, not per route.*
7. **Storage choices aged badly.** A JSON file per entry (fine for debugging — keep that property) synced over **git** (`git_sync.rs`, debounced commits + a 30s background pull) as the multi-instance story. *Fix: content-addressed fragments in an embedded KV, with an explicit shared tier if multi-node is needed.*
8. **It was per-route, per-call, manual.** 40+ hand-written `*_cached` methods in `backend/cache.rs`, each with its own key tuple and condition list, and each route had to call the `_cached` variant (`routes/custom/*.rs`). *Fix: it is a proxy. Nothing in the application has to opt in.*

What survives into the new design: the condition algebra (generalized into dependency *terms*), derived read-sets, russian-doll composition, cache warming, and human-readable entries.

---

## 3. The Skip model, in the terms we need

Sources: [Skip docs](https://skiplabs.github.io/skip/docs/introduction),
[Services and Resources](https://skiplabs.io/docs/resources),
[writing functions](https://skiplabs.io/docs/functions),
[external data sources](https://skiplabs.io/docs/externals),
[Reactivity overview, 2017](https://skiplang.com/blog/2017/01/04/how-memoization-works.html).

**Programming model.** A *service* maps inputs to *resources*. *Collections* are
the graph's vertices: key → values, keys and values arbitrary JSON. Collections
are **eager** (kept current) or **lazy** (computed per key on demand).
*Mappers* are the edges: `mapEntry(key, values) -> Iterable<[key, value]>`, and
they must be **deterministic and side-effect-free** — that is the whole licence
for memoizing them. A `Resource` implements `instantiate(inputs) -> EagerCollection`;
each distinct parameter set instantiates a node that extends the shared graph,
and is garbage-collected after inactivity. External systems enter through
`ExternalService` (`subscribe`/`unsubscribe`), with shipped adapters for Postgres
(table subscriptions → eager collection), Kafka, and plain HTTP polling.

**Serving model.** Streaming port: `GET /v1/streams/:uuid` → SSE, an `init`
event with everything, then `update` events carrying only changed keys. Control
port: `POST /v1/streams/:resource` → uuid, `DELETE /v1/streams/:uuid`,
`POST /v1/snapshot/:resource` (and `/lookup` for one key),
`PATCH /v1/inputs/:collection`.

**Runtime model** — the part worth stealing verbatim:

- A **Demanded Computation Graph** in the ADAPTON sense, built at runtime from
  observed reads: edges from a memo entry to the *cells* it read, and to the
  memo entries it called (through non-memoized frames too).
- **MVCC revisions.** Every cell and every memo entry holds a linked list of
  revisions, each a value plus a non-overlapping `[begin, end)` transaction-id
  lifespan, newest at the head. A reader latches the global transaction id at
  start and sees only values as of that id. A mutator commit changes any number
  of cells, propagates invalidation, *then* bumps the global id — so the new
  version is invisible until it is complete, with no global lock. Revisions no
  live reader needs are collected.
- **Traces**, to avoid recomputing when a dependency changed but the derived
  value did not — i.e. avoiding false-positive invalidation.
- **Incremental coarsening** of the graph, to keep it from degenerating into
  per-read bookkeeping.

### Why this maps onto TerminusDB unusually well

| Skip | TerminusDB 12 / Reactor |
|---|---|
| global transaction id | **commit id** on a branch — already a total order, already durable, already exposed (`TerminusDB-Data-Version`) |
| cell revisions `[begin, end)` | a document version is pinned by the commit range in which it is current; reads at `/local/commit/<id>` are the snapshot read, natively |
| mutator commit → invalidate → bump id | a TDB commit lands → `_ChangeSet` event → we apply terms → bump watermark |
| `ExternalService.subscribe` | `_ChangeSet` SSE subscription + `/api/changes` + `/api/diff` backfill |
| eager vs lazy collections | per-resource `eager` (recompute on commit) vs `lazy` (fill on demand) policy |
| mapper purity enforced by the Skip compiler | enforced by trait discipline + a shadow-verify mode (§7.4); we cannot statically prove it |
| traces (false-positive avoidance) | **content hash per fragment** — if the re-derived fragment hashes the same, we keep its `changed_at` and stop propagation (this is salsa's backdating) |
| incremental coarsening | term granularity ladder: `DocField` → `Doc` → `Class` (§5.2), chosen per resource and degraded under pressure |

### Adjacent prior art we should borrow from, not re-derive

- **[Noria](https://pdos.csail.mit.edu/papers/noria:osdi18.pdf) (OSDI'18)** — partially-stateful dataflow. Derived state starts *empty* and is filled lazily by **upqueries**; entries are **evicted** under memory pressure and eviction notices flow downstream; a later read of evicted state recomputes recursively, to the base tables if necessary; partial state is disabled for operators whose upqueries would require a full scan. This is precisely the memory story a TDB cache needs — we cannot mirror a large database, and we must not pretend otherwise. (Reported up to 20× MySQL throughput, comparable to a tuned KV cache.)
- **[Stellate / GraphCDN](https://stellate.co/blog/akamai-caching)** — normalized GraphQL edge caching with entity/list/type invalidation policies. Two hard-won lessons: queries must *request ids* or the cache cannot map a response back to records (the [Hashnode stale-cache bug](https://blog.vicentereyes.org/how-i-fixed-the-hashnode-graphql-api-stale-cache-bug-stellate-cdn) was exactly this), and creations/deletions need a type/list-level purge, not just entity-level. We solve the first by **injecting `_id`/`_type` into every selection set** rather than asking clients to.
- **[salsa](https://docs.rs/salsa/latest/salsa/) 0.28** — the Rust incremental framework behind rust-analyzer/Ruff/Turbopack: `#[input]` structs, `#[tracked]` functions that record what they read, `Revision`, **backdating** (equal re-derived result keeps the old `changed_at`, so invalidation stops spreading), **durability** (a revision touching only low-durability inputs lets high-durability dependents skip validation), interning, LRU, cancellation. It is, for practical purposes, Skip's memo core already written in Rust — and therefore the reference design for §5, whether or not we depend on it (§6.1).
- **Rails russian-doll fragment caching** — the composition discipline: a parent fragment's key embeds its children's versions, so a child change rebuilds only the spine above it.

---

## 4. What TerminusDB 12.1 gives us (verified in this workspace)

- **`_ChangeSet` subscriptions over SSE.** `docs/terminusdb/graphql-subscriptions.md`; client support in `crates/client/src/http/subscription.rs:203` (`subscribe_graphql`); covered by `crates/client/tests/test_v12_1_graphql_subscriptions.rs`. Fires **once per commit** with list-valued `<Class>_added` / `_changed` / `_deleted` fields plus `_CommitMetadata { _id _timestamp _datetime _change_type }`. The selection set on each field resolves real document fields, under the same access control as a query. **Deleted documents resolve from the pre-commit layer**, so we get the dying document's full contents — exactly what we need to fire reverse-edge terms. A TDB-specific `connected` event precedes the first `next`, which removes the subscribe/mutate race. NDJSON mode is available as an alternative framing.
- **`_ChangeSet` on the Query root** — the changes of a given transaction context on demand; combined with commit pinning this is a per-commit replay primitive.
- **`GET /api/changes/<path>`** → `{added, changed, deleted}` document ids for a branch head or a pinned commit (`crates/client/src/http/v12_1.rs:147`, `ChangesResponse` at `:32`). Cheap, and the gap-recovery path.
- **`POST /api/diff` with `before_data_version`/`after_data_version` and `document_id: None`** → the patch for **all** documents between two commits (`crates/client/src/http/diff.rs:155` `diff_data_versions`, typed `PatchOperation` at `:318`). This is our authoritative before/after changelog — the thing that lets us compute *changed field sets* and fire `Index`/`Backlink` terms **without** mirroring the whole database.
- **Commit pinning on every path endpoint.** `crates/client/src/http/url_builder.rs:32` builds `{org}/{db}/local/commit/{id}`, so document reads, GraphQL and WOQL can all be executed as of a commit. Pinned responses are immutable.
- **History.** `/api/history/<path>` with v12.1's `diff`, `complete`, `fast`, `before`, `after`, `graph_type` params (`crates/client/src/document/history.rs`) and `streaming=true` NDJSON (`crates/client/src/http/v12_1.rs:343`).
- **GraphQL surface worth knowing about.** `_id`, `_type`, `_count` (index-resolved for an empty filter), automatic **backlinks** `_PROPERTY_of_CLASS`, and **path queries** `_path_to_CLASS` (`docs/terminusdb/graphql-query-reference.md:367`ff). Subclass matching is implicit; `_ChangeSet(include_children:)` controls it. Schema introspection via `client.introspect_schema` (`crates/client/src/http/graphql.rs:420`) and class frames / JSON Schema via `get_class_frame` / `get_json_schema`.
- **Known rc3 limitations.** Subscription `filter` arguments are parsed and schema-checked but **not applied** by the community server, so a subscriber must filter client-side (documented in `crates/client/src/http/subscription.rs` and asserted in the tests). For us this is an opportunity: the proxy can apply filters correctly and multiplex one upstream subscription to many clients.
- **Content-addressed commits and layers.** "Every write to a TerminusDB database produces a commit with an author, a timestamp, a message, and a content-addressed identifier" (`docs/terminusdb/knowledge-graph-database-version-control.md:65`); layers are SHA-1 content-addressed and "the same content always produces the same identifier" (`querying-layers-with-graphql.md:125`), which is why clone transfers only unique layers. **Consequence for horizontal scaling: a commit id means the same thing on every replica that holds it**, so a fragment keyed by `(…, ShapeId, CommitId)` is node-independent. This is the load-bearing assumption behind §5.7 and is the first thing M0 must verify empirically.
- **Optimistic concurrency, per-branch head pointer.** Writers take no locks, check at commit time whether the head moved, and retry (3 by default, `TERMINUSDB_SERVER_MAX_TRANSACTION_RETRIES`); readers hold an immutable layer snapshot and never conflict (`docs/terminusdb/immutability-and-concurrency.md:34-47`). So "the current commit of branch B" is a single atomically-published pointer per node — exactly the thing our watermark shadows.
- **Numeric fidelity.** v12 returns numbers as native JSON at full rational precision and the workspace enables `serde_json/arbitrary_precision` for that reason. **Consequence for this design: the cache must store and replay raw response bytes, never a re-serialized `serde_json::Value`.**

### Must-verify before committing to the design (M0 spikes)

Two of these are new, and both come from the horizontal-scaling requirement.
They are first because either answer can reshape §5.7.

- **S1 — Does replication emit change events?** On a replica, *all* changes arrive
  by `fetch`/`pull`/`apply`/fast-forward rather than by a local write. Does a
  `_ChangeSet` subscription on that node fire for commits that arrive this way,
  or does replication move the head pointer silently? **This is the highest-risk
  unknown in the whole design**: if replication is silent to subscriptions, then
  on read replicas the subscription is decorative and head polling becomes the
  primary capture mechanism rather than the backstop. Test: two embedded servers,
  subscribe on the downstream, push/pull, watch the stream.
- **S2 — Are commit ids byte-identical across replicas?** Clone a database, commit
  upstream, pull, then compare commit ids and `TerminusDB-Data-Version` values on
  both nodes. The shared-fragment-tier design in §5.7 is sound only if they match.
  If they don't, fragments must be keyed by `(node, commit)` and the shared tier
  collapses to per-node caches — survivable, but worth knowing on day one.
- **S3 — Subdocuments.** Does changing a subdocument surface as the subdocument's own class in `_ChangeSet`, as the parent's `_changed`, or both? Dependency terms for `@subdocument` trees hinge on this.
- **S4 — Gap behaviour on reconnect.** After a dropped SSE connection, are commits that landed in the gap lost? (Assume yes; design the walk-the-log recovery path either way and measure it.)
- **S5 — Wide-schema subscriptions.** `_ChangeSet` needs one field per class per op — what does a 300-class schema cost in query size, registration time and per-commit resolution? If it is bad, the upstream ask is a wildcard `_ChangeSet { _added { _id _type } }`.
- **S6 — `/api/diff` cost per commit** at realistic sizes, versus `/api/changes` + N pinned document reads.
- **S7 — Schema commits.** Do schema changes arrive on the instance subscription? We need to detect them to re-derive the subscription query and the term vocabulary.
- **S8 — `@shared` / cascade-delete and `unfold`** interaction with change reporting (we already have coverage for the schema side in `test_v12_1_schema_annotations.rs`).

---

## 5. The model

### 5.1 Objects

```
Namespace   = (org, db, branch)               -- the unit of a cache universe
Revision    = CommitId                        -- total order within a Namespace
Watermark   = Revision                         -- last commit fully applied

DocId       = EntityIDFor<T> / TdbIRI          -- never a format!'d string
ShapeId     = hash(normalized selection shape) -- "which projection of a doc"
Fragment    = { key: (DocId|RootKey, ShapeId)
              , computed_at: Revision
              , content: Bytes                 -- raw upstream bytes
              , hash: ContentHash              -- for backdating + ETag
              , deps: Vec<Term>                -- read set
              , children: Vec<FragmentKey> }   -- russian-doll spine
Resource    = { name, kind, params, policy }   -- a named cacheable derivation
```

A **Fragment** is the only cached object. A response is a Fragment whose
`children` are Fragments. `content` is bytes, not values — byte-faithful
replay is a correctness requirement, not an optimization.

### 5.2 Dependency terms — the invalidation algebra

Terms replace the prototype's `Condition`. A term is serializable data, is a key
in an inverted index, and is *fired* by a commit. Granularity ladder, precise → coarse:

| Term | Fires when | Replaces |
|---|---|---|
| `DocField(id, prop)` | that document's `prop` changed | — |
| `Doc(id)` | that document changed at all | `one_changed!` |
| `Edge(id, prop)` | the outgoing `prop` edge set of `id` changed | (was conflated into `Field`) |
| `Backlink(id, prop, Class)` | the set of `Class` documents pointing at `id` via `prop` changed | the three-spellings hack in `review_state.rs` |
| `Index(Class, prop, value)` | the set of `Class` docs with `prop == value` changed | `field!(C, p => v)` |
| `ClassField(Class, prop)` | any instance's `prop` changed | — |
| `Members(Class)` | an instance of `Class` (or subclass) was added/deleted | `count_changed!` |
| `Class(Class)` | anything about any instance of `Class` changed | `any_changed!` |
| `Path(from, pattern)` | conservative: any change to any class on the pattern | — (unsupported before) |
| `Schema` | the schema graph changed | — |
| `Ttl(d)` / `Always` | time / never cache | `Ttl` |

Two rules keep this honest:

- **Soundness over precision.** An analyzer that cannot prove a tight term must
  emit a coarser one (`Class`, or `Always` = bypass). Precision is a performance
  property; coarseness is never a correctness bug. This is Noria disabling
  partial state when an upquery would need a scan.
- **Terms are derived from the schema, not guessed.** Class frames and the
  schema graph (`crates/gql/src/frames.rs`, `get_class_frame`) tell us which
  properties are value properties and which are links — the VALUE-vs-LINK
  distinction in `CLAUDE.md` is the difference between `DocField` and `Edge`,
  and getting it wrong silently breaks traversal caching.

### 5.3 Firing: commit → terms

For each commit `C` on a namespace, the capture pipeline produces a **ChangeSet**
`{added, changed, deleted}` where each entry carries before/after document JSON,
then derives fired terms:

```
for doc in added:    fire Doc(id), Members(class), Class(class)
                     for each value prop p:  fire ClassField(class,p), Index(class,p,new)
                     for each link prop p:   fire Edge(id,p), Backlink(target,p,class)
for doc in deleted:  same, using the *pre-commit* document (which _ChangeSet gives us)
for doc in changed:  diff before/after:
                     for each changed value prop p: fire DocField(id,p), ClassField(class,p),
                                                    Index(class,p,old), Index(class,p,new)
                     for each changed link  prop p: fire DocField/Edge(id,p),
                                                    Backlink(old_target,p,class),
                                                    Backlink(new_target,p,class)
                     fire Doc(id), Class(class)
plus: for every superclass of class, fire the Members/Class terms too (subclass matching)
if the commit touched the schema graph: fire Schema (invalidates the namespace's shapes)
```

`Index(class, p, old)` and `Backlink(old_target, …)` are why we need before-state.
`_ChangeSet` gives it for deletes; `/api/diff` between the parent commit and `C`
gives it for changes. That is the whole reason the diff endpoint is in the design.

### 5.4 Invalidation and recomputation

```
apply(C):
  terms   = fire(changeset(C))
  dirty   = index.lookup(terms)                     -- inverted index: Term -> [FragmentKey]
  for f in topological_order(dirty):                -- leaves first, up the spine
      match policy(f):
        Lazy  => drop f; propagate dirt to parents
        Eager => f' = recompute(f, pinned_at = C)
                 if f'.hash == f.hash: keep parents' validity   -- BACKDATING
                 else: replace f; propagate dirt to parents
  watermark = C
```

Three properties fall out:

- **Backdating** is the whole game for russian-doll efficiency. A publication
  whose chunk list is unchanged keeps its envelope fragment even though the
  chunk's content fragment was replaced — only the spine's *composition* is
  redone, and only if a child hash actually moved. This is Skip's "traces"
  without needing traces.
- **Composition, not refetch.** A dirty parent whose children are all valid is
  rebuilt by splicing child bytes, with **no upstream query**. This is what
  `document_get_from_nested_caches` did by hand for exactly one route.
- **Eager vs lazy is a per-resource policy**, i.e. Skip's eager/lazy collections,
  i.e. the old `cache_warmer`, generalized.

### 5.5 Consistency guarantees (stated, not implied)

The cluster is eventually consistent, so an *absolute* guarantee would be a lie.
The guarantee is therefore **relative**, and it is the only one worth making:

> **Reactor is never weaker than the TerminusDB node it fronts.** For every
> request, the bytes Reactor returns are bytes that its local node did return, or
> would have returned, at some commit on that node's branch history; and never at
> an earlier commit than the client has already been shown *by this node*.

Everything else is a consequence. Let `W` be this Reactor's watermark for a
namespace (the last commit of its local node fully applied to the index).

1. **Pinned reads are immutable and immortal.** A request at
   `/local/commit/<id>` has one correct answer forever, on every replica that
   holds the commit. Cache, serve, never invalidate.
2. **Read-your-writes, per node.** Writes pass through us; we read the response's
   `TerminusDB-Data-Version`, record `expected = C_new`, and *immediately* pull
   `C_new`'s changeset rather than waiting for the SSE event. A later read on that
   session hitting **this** node is served from cache only if `W >= C_new`;
   otherwise we wait (bounded, default 500ms) or forward it pinned at `C_new`.
   A read that lands on a *different* node cannot be stronger than that node's
   own replication state — see (5).
3. **Monotonic reads, per session.** A client may echo the last data-version it
   saw (header or cookie); we never serve it something derived from an earlier
   commit *on this node's history*. Same mechanism as (2).
4. **Bounded staleness otherwise.** An unqualified read may be answered from any
   `W' <= W`; we report `W'` in `TerminusDB-Data-Version`, the node identity in
   `X-Reactor-Node`, the node's current head in `X-Reactor-Head`, and `Age`. The
   staleness is visible, not implicit.
5. **Across nodes, we fence but cannot conjure.** If a request asserts
   `X-Reactor-Min-Version: C` and the local node has not replicated `C`, Reactor
   has three honest options, selected per route by policy:
   `wait` (bounded, then fall through), `fail` (`412 Precondition Failed`, so a
   load balancer can retry elsewhere — the preferred setting for read-after-write
   paths), or `serve-stale` (answer and label it). What it must never do is
   silently pretend. Because it also publishes `X-Reactor-Head` and
   `GET /_reactor/head`, a smart client or LB can route by replication state
   instead of guessing.
6. **Snapshot-consistent fills.** Every fill is executed **pinned at a commit**,
   so a fragment is exactly "the value at `C`", never a torn read across commits.
   This is the single biggest correctness upgrade over the prototype, and it is
   also what makes a fragment portable between nodes.
7. **No cross-document atomicity claim.** A composed response may splice
   fragments computed at different commits *unless* the resource opts into
   `consistent_at` (all children recomputed at one revision). Default: splice,
   and report the minimum commit; strict resources: recompute. Say which, per
   resource, in config.

Note what *doesn't* get harder under horizontal scaling: because every cached
fragment is tagged with the commit it was computed at, and commit ids are content
addressed, two nodes at different points in a branch's history simply hold
different *subsets* of the same immutable fragment space. They cannot hold
*conflicting* entries for the same key. Divergence between nodes shows up as
"this node hasn't got that one yet", never as "these two nodes disagree about
what `(doc, shape, commit)` means". That is the property that makes replicating
the cache optional rather than necessary.

### 5.6 Authorization

Caching must not become an access-control bypass. TDB's access control is per
resource (db/graph), not per document, which makes this tractable:

- Cache **data** keyed by `(namespace, query)`, with no user in the key.
- Gate every request on a capability check for `(user, namespace, action)`,
  itself cached with a short TTL and invalidated by a `_system` subscription on
  `UserDatabase`/role changes (`subscribe_graphql_system` already exists).
- Never cache responses to requests that carry a token we cannot resolve to a
  capability set; forward them.
- `Vary`-style discipline: any request header that can change the *body*
  (`Accept`, `Accept-Encoding`, `X-...` format switches, `GetOpts` params like
  `unfold`, `compress_ids`, `minimized`, `as_list`) is part of the key.
- Flag explicitly: if TDB ever grows document-level ACLs, the "no user in the
  key" shortcut dies. Note it in the config as an assumption.
- Under horizontal scaling, capability state replicates like everything else, so
  a freshly granted capability may not be visible on every node yet. Keep the
  capability TTL short (seconds) and treat *denials* as non-cacheable, so the
  failure mode is a brief extra round trip rather than a brief extra permission.

### 5.7 Horizontal scaling: one Reactor per node, two classes of state

TerminusDB scales out as eventually-consistent replicas (clone/fetch/push/pull
over content-addressed layers), with optimistic single-head writes per branch per
node. The cache layer follows that shape rather than fighting it:

**Topology: strictly one Reactor per TerminusDB node, as a sidecar.** It talks
only to `localhost`, tracks only that node's heads, and makes no claim about any
other node. There is no cluster membership, no quorum, no leader election, and
nothing to split-brain. A Reactor that cannot see its node is simply down; a
Reactor that is down is bypassed by removing it from the LB.

**The state splits cleanly in two, and this is the whole trick:**

| | Immutable, node-independent | Mutable, node-local |
|---|---|---|
| **What** | Fragments keyed `(DocId \| RootKey, ShapeId, CommitId)` + their content hashes and recorded read-sets | Watermark per namespace, branch-head pointers, term → fragment inverted index, dirty/eviction state, in-flight single-flight map, capability cache |
| **Why** | A commit id is content-addressed, so the tuple identifies one byte string on every replica that holds the commit | "Which commit is current here" and "what is dirty here" are statements about *this* node |
| **Sharing** | Safe to share, replicate, gossip, or back by object storage — **with no coherence protocol, because nothing is ever updated in place** | Never shared |

So three deployment modes, all supported by the same binary and differing only in
the fragment store's backend:

1. **Independent (default, recommended to start).** Each node has its own
   private cache. Nodes warm separately; a given fragment may be computed N
   times across N nodes. Zero shared infrastructure, zero new failure modes.
   This is the "its own view of it" option, and it is the one I'd ship first.
2. **Shared immutable tier.** The content-addressed fragment store is backed by
   Redis / S3 / a shared filesystem, read-through and write-behind, in front of
   the node-local hot tier. Because entries are immutable and content-addressed,
   there is no invalidation message, no versioning, no tombstone, and no
   possibility of a node reading a *wrong* value — only a value it would not
   otherwise have had yet. Each node still keeps its own watermark and term
   index, so each node invalidates on its own schedule.
   Cost: one dependency, plus the latency of the shared tier on a hot miss.
   Win: fills are paid once per cluster, and a cold/new node starts warm.
3. **Peer read-through.** Before filling from the origin, ask a sibling Reactor
   for `(key, commit)` over HTTP. Cheapest version of (2), no new service, but
   it adds a failure mode and an N² fan-out; only worth it if measurements say so.

**The three replication mechanisms we must support** (operator, 2026-11-18):

1. **Shared disk over NAS, single writer / many readers** — every node reads the
   same content-addressed layer files; one node commits, the rest only read. A
   reader node has *no local write path at all*, so its `_ChangeSet` subscription
   is expected to be silent and head polling is the **primary** capture driver
   there, not a backstop. This is the mode to design for first.
2. **Kafka** as a replication/sync stream (future).
3. **Hypercore** append-only log as a sync stream (future).

All three reduce to the same contract: something *notices* that a commit exists,
and something else *confirms* the local node can actually read it. That split is
why capture is three traits rather than one (§6.1).

NAS-specific hazards, all of which the design must survive rather than assume
away: NFS attribute and negative-dentry caching (`actimeo`, `lookupcache`,
`nocto`) can make a head visible before its layers are, or hide a new head for
seconds; the server's own in-memory label/layer caches may not re-read a label
per request; a head can appear to go *backwards* on a reader; `inotify` does not
work over NFS, so file watching is not an option; and writer-side GC, `squash`
or `optimize` may remove layers a reader has pinned. The defence is uniform and
cheap: **the watermark only advances past a commit after a pinned read at that
commit has succeeded on this node** (the `ReadinessGate` canary — and the
changes/diff fetch we need anyway serves as it). Capture lag then costs
staleness, never correctness, and a fill that 404s simply retries or re-latches.

**Consequences to design for explicitly:**

- **Capture on replicas.** On a read replica, changes arrive by replication, not
  by local writes. If replication is silent to `_ChangeSet` (spike S1 in §4), then
  head polling via `/api/log` is the *primary* capture path there, with the
  subscription used only where writes are local. The capture module must therefore
  support both drivers behind one interface from M3 onward, not bolt polling on
  later. Poll interval becomes the staleness bound on replicas; make it a
  per-namespace config.
- **Non-fast-forward head movement is now routine, not exotic.** `pull`,
  `rebase`, `apply` and divergent-then-reconciled histories all move a local head
  to a commit that is not a descendant of `W`. Default behaviour stays: detect
  (the new head's ancestry does not contain `W`), then either diff `W → head` as
  one changeset or drop the namespace's *local* state. Note that this only ever
  drops node-local index/watermark state — shared immutable fragments survive,
  because they were never wrong.
- **Streams and resources carry a commit cursor.** An SSE stream's events are
  tagged with the commit they reflect. On reconnect — including reconnect to a
  *different* node after a failover — the client presents its cursor; if that
  commit is unknown to the node or ahead of its head, the node answers with a
  fresh `init` snapshot instead of deltas rather than inventing a diff. Sticky
  sessions make this rare; correctness does not depend on them.
- **Thundering herd** is per node: single-flight by `(key, commit)` locally, and
  mode (2) collapses it cluster-wide for free.
- **Optional write routing.** The frontend already classifies requests as reads
  or writes. If the deployment designates a write primary, Reactor can forward
  writes there and serve reads locally, which turns read-after-write into case (5)
  of §5.5 with a known target. Useful, cheap, and strictly optional — flagged as
  scope creep, not assumed.
- **Metrics gain a dimension.** Replication lag (local head vs. upstream head),
  per-node hit rate, cross-node fragment reuse rate in mode (2), and
  `412`-on-min-version counts, which are the signal that session affinity or
  write routing is needed.

---

## 6. Architecture

```
                              ┌──────────────────────────────────────────┐
  clients ───── HTTP ───────► │  tdb-reactor                             │
  (unchanged TDB API)         │                                          │
                              │  ┌────────────┐   miss / bypass          │
                              │  │  frontend  │ ─────────────────────────┼──► TerminusDB
                              │  │  (axum)    │ ◄── raw bytes ───────────┼─── (localhost)
                              │  └─────┬──────┘                          │
                              │        │ key + terms                     │
                              │  ┌─────▼──────┐    ┌──────────────┐      │
                              │  │  analyze   │    │   engine     │      │
                              │  │ gql/woql/  │───►│  DCG + memo  │      │
                              │  │ document   │    │  + policy    │      │
                              │  └────────────┘    └───┬──────┬───┘      │
                              │                        │      │          │
                              │                  ┌─────▼──┐ ┌─▼───────┐  │
                              │                  │ store  │ │  index  │  │
                              │                  │ CAS+KV │ │ Term→Fk │  │
                              │                  └────────┘ └────▲────┘  │
                              │                                  │       │
                              │  ┌────────────┐   ┌──────────────┴────┐  │
                              │  │  resources │   │     capture       │  │
                              │  │  SSE out   │◄──│ _ChangeSet SSE    │◄─┼─── TerminusDB
                              │  └────────────┘   │ + /changes /diff  │  │    (subscription)
                              │                   │ + log walk        │  │
                              │                   └───────────────────┘  │
                              └──────────────────────────────────────────┘
```

### 6.1 Modules (one crate, `crates/reactor`, split later if it earns it)

**`engine`** — the DCG. Fragments, revisions, terms, the inverted index,
backdating, eviction, policies. **No I/O, no HTTP, no TerminusDB types** — a pure
state machine over `(ChangeSet, Request) -> (Response, Vec<Fill>)`. This is what
makes it testable against an oracle (§7.4).

> **Decision: hand-roll the DCG, modelled on salsa, rather than depend on salsa.**
> Salsa has the right semantics (revisions, backdating, durability, LRU,
> cancellation) and I will copy them. But three requirements cut against using it
> directly: (a) fills are **async** and demand-driven, and salsa's tracked
> functions are sync — the workaround is a suspend/retry demand loop around
> `&mut db` revisions, which fights salsa at exactly the point where our
> concurrency lives; (b) fragments must be **persistent** across restarts and
> partly **on disk**, which salsa does not do; (c) invalidation arrives as
> *external terms*, not as input-struct setters, so the index is ours anyway.
> Keep the engine behind a trait so a salsa-backed implementation stays a live
> option; use salsa's docs and test suite as the semantics reference. Reversible
> either way, and worth revisiting once (a) is measured rather than assumed.

**`capture`** — one task per active namespace, behind a `CommitSource` trait with
**two interchangeable drivers from the start** (§5.7): `Subscription` (low
latency, needs replication to emit events) and `HeadPoll` (always works, latency
= poll interval, the only option on a silent replica). Both produce the same
`ChangeSet` stream, and a namespace may be configured with either or with
subscription-plus-poll-as-liveness-check. The subscription driver:
1. `subscribe_graphql(db, branch, _ChangeSet { … })`, query generated from the
   class list (introspection + class frames), regenerated on `Schema` fire.
2. Wait for `connected`, then record `start = current head` and **walk the log
   from `W` to `start`** so nothing in the subscription-setup gap is lost.
3. Per `next` event: take `_CommitMetadata._id`, the per-class added/changed/
   deleted lists with their resolved fields, then (if before-state is needed)
   `diff_data_versions(parent, C, None)` for the authoritative patch. Emit a
   `ChangeSet` to the engine; bump `W`.
4. On disconnect: reconnect, then gap-recover by walking `/api/log` and
   `/api/changes` per commit. On **non-fast-forward head movement** (rebase,
   reset, squash, apply, pull) detect that the new head is not a descendant of
   `W` and either diff `W → new head` as one giant changeset, or drop the
   namespace's cache. Correctness first: default to drop, optimize later.
5. Write-driven fast path: a mutation through the frontend hands `capture` the
   new commit id directly, so read-your-writes does not wait on SSE.

**`analyze`** — request → `(CacheKey, Vec<Term>, Policy)`, per protocol:

- *Document API*: `id=` → `Doc(id)`; `type=` → `Members(C)` + per-doc fragments;
  `graph_type=schema` → `Schema`; all of `GetOpts` in the key. NDJSON list
  responses are cached as a **list of doc fragments** and re-streamed by
  composition, so one changed document does not kill the list.
- *GraphQL*: parse (`graphql-parser`, already a `crates/gql` dependency),
  **normalize** (sort fields, inline fragments, hoist variables out of the key),
  then **inject `_id` and `_type` into every object selection set** so every
  response node maps to a `DocId` — Stellate's hardest-won lesson, solved on our
  side of the wire instead of asking clients to change. Injected fields are
  stripped from the client's response, kept in the fragment. Term extraction
  walks the typed AST against class frames: root field + filter → `Index`/`Members`;
  nested link field → `Edge(parent, prop)` + child fragment; backlink
  `_p_of_C` → `Backlink(id, p, C)`; `_count` → `Members`/`Index`;
  `_path_to_C` → `Path` (conservative). Mutations: never cached, always treated
  as writes. Subscriptions: never cached — multiplexed (see `resources`).
- *WOQL*: analyze the `woql2` AST (`crates/woql2/src/query.rs`). `Triple`/`Link`/
  `Data` with a bound subject → `Doc`/`Edge`; bound predicate only → `ClassField`;
  `IsA`/`TypeOf`/`Subsumption` → `Class`; `ReadDocument` → `Doc`; `Path` → `Path`;
  `Pin`/`Using` at a commit → immutable. Any `Add*`/`Delete*`/`*Document` write
  op anywhere in the tree → not cacheable, forward as a write. Key = canonical
  JSON-LD of the AST (`Query::to_woql_json`), which also normalizes the
  `graph:None` defaulting quirk noted in `xpath-crate-and-woql-graph-gotcha`.
  Unanalyzable shapes → bypass, counted in metrics so we can see what we're missing.
- *Everything else* (`/api/db`, `/api/branch`, `/api/log`, `/api/optimize`,
  clone/fetch/push/pull/rebase/squash/reset/apply, `/api/migration`, users/roles/
  organizations/capabilities): forwarded. Branch-topology and schema-migration
  routes additionally **signal** `capture` to resync or drop the namespace. There
  are 51 paths in `docs/openapi.yaml`; the default for an unknown path is
  *forward, cache nothing*, and that default must never be inverted.

**`store`** — three tiers: in-memory hot (`moka`, size- and count-bounded, LRU =
Noria eviction), embedded KV warm (`redb` — single-file, zero-copy reads, no
compaction story to babysit), content-addressed blob store for fragment bytes
(dedup across commits/branches is enormous for TDB, where most documents are
unchanged between commits). Keep the prototype's debuggability: an admin endpoint
that dumps any fragment as readable JSON with its terms and provenance. Optional
shared tier (Redis/S3) only if multi-node is a real requirement — and **not git**.

**`frontend`** — `axum` reverse proxy. Byte-faithful streaming passthrough
(`hyper` body relay; no serde round-trip, see the `arbitrary_precision` note),
`Authorization` passthrough, `Cache-Status` (RFC 9211) on every response,
ETag/`If-None-Match` from fragment hashes (TDB has no ETags; we add them — the
prototype only did this for static assets, `instance/aws/src/fairings/etag.rs`),
`TerminusDB-Data-Version` echo and fencing.

**`resources`** — the Skip layer, and the reason this is more than a cache:
named, parameterized, declaratively-defined denormalized views with
`POST /_reactor/resources/:name` → id, `GET /_reactor/streams/:id` → SSE
(`init` then keyed `update`s), `POST /_reactor/snapshot/:name`, mirroring
[Skip's control/streaming API](https://skiplabs.io/docs/resources) so Skip's
clients and idioms transfer. Also: **subscription multiplexing** — one upstream
`_ChangeSet` per namespace fanned out to N client subscriptions, **with filters
actually applied** (which rc3's server does not do). That is a user-visible
feature on day one of M7, independent of caching.

### 6.2 Deployment

Single binary **sidecar to each TerminusDB node** (`terminusdb-bin` can supervise
it), talking to `localhost:6363` via `terminusdb-client`, with the cluster's load
balancer pointed at the Reactors rather than at the nodes. Config in TOML:
node identity, namespaces to track, commit source per namespace
(`subscription` | `poll` | `both`, §5.7), fragment store backend (`local` |
`shared`, §5.7 modes 1–3), per-route policies, resource definitions, memory
budgets, consistency defaults including the `min-version` miss policy
(`wait` | `fail` | `serve-stale`). `--shadow` mode (§7.4) and `--passthrough`
kill switch (the prototype's `set_disabled`, which was the right instinct — keep
it, but make it a signal/endpoint so one node can be taken out of the cache path
without a redeploy, not an env var).

Horizontal scaling adds no orchestration: adding a node means starting a TDB
replica with a Reactor beside it and adding it to the LB. A new node is cold in
mode 1 and warm-ish immediately in mode 2.

---

## 7. Functionality

### 7.1 Route coverage

| Route | Cached | Key | Terms |
|---|---|---|---|
| `GET /api/document?id=` | ✅ fragment | id + `GetOpts` | `Doc(id)` (+`Edge` per unfolded link) |
| `GET /api/document?type=` | ✅ list-of-fragments | type + opts + paging | `Members(C)` + per-doc `Doc` |
| `GET /api/document` (schema) | ✅ | — | `Schema` |
| `POST /api/graphql` query | ✅ fragment tree | normalized query hash + vars | extracted (§6.1) |
| `POST /api/graphql` mutation | ❌ write | — | drives firing |
| `POST /api/graphql` subscription | ❌ multiplexed | — | — |
| `POST /api/woql` read-only | ✅ | canonical AST hash | extracted from AST |
| `POST /api/woql` with writes | ❌ write | — | drives firing |
| `GET /api/changes`, `/api/history`, `/api/log` | ✅ | path + params | `Doc`/`Members`; **immutable when pinned** |
| `GET /api/schema` (frame/jsonschema) | ✅ | class + opts | `Schema` |
| `POST /api/diff`, `/api/compare` | ✅ when both sides pinned | versions | immutable |
| anything at `/local/commit/<id>` | ✅ **forever** | path+body | none |
| `/api/db`, `/api/branch`, `/api/squash`, `/api/reset`, `/api/rebase`, `/api/apply`, `/api/pull`, `/api/fetch`, `/api/push`, `/api/clone`, `/api/optimize`, `/api/migration` | ❌ | — | signal resync/drop |
| `/api/users`, `/api/roles`, `/api/organizations`, `/api/capabilities` | ✅ short TTL | user+resource | `_system` subscription |
| unknown path | ❌ forward | — | — |

### 7.2 Client-visible features

- Transparent speedup with **no client changes** — the whole point of being a proxy.
- `Cache-Status`, `Age`, `ETag`/`304`, `TerminusDB-Data-Version` on reads, plus
  `X-Reactor-Node` and `X-Reactor-Head` so a client or LB can see *which* replica
  answered and how far along it is.
- Opt-in headers: `X-Reactor-Max-Staleness`, `X-Reactor-Min-Version` (fenced per
  §5.5 (5)), `X-Reactor-Bypass`, `X-Reactor-Explain` (returns the key, terms,
  hit/miss per fragment and why it was invalidated — the "why is this stale" tool
  the prototype never had and badly needed).
- `GET /_reactor/head` — this node's head and watermark per namespace, for
  replication-aware routing and for health checks that mean something more useful
  than "the process is up".
- Reactive resources + SSE streams, Skip-API-shaped.
- Correct, server-side-filtered subscriptions, multiplexed over one upstream connection.
- Change feeds / webhooks per class or per term.

### 7.3 Operability

Prometheus metrics: hit/miss/bypass by route and reason, fill latency, fragments
by tier, bytes, terms fired per commit, fragments invalidated per commit,
**backdating saves** (the key efficiency number), watermark lag, SSE
reconnects, gap recoveries, **replication lag** (local head vs. upstream head),
`412`s on `min-version`, and — in shared-tier mode — cross-node fragment reuse. Admin API: dump a fragment, dump the term index for
a document, force-invalidate a namespace, force resync, dump the dependency
graph as DOT/JSON for a given response.

### 7.4 Testing — the part that decides whether this is trustworthy

`CLAUDE.md` is blunt about this: nothing is proven until it runs against a real
database. Four layers:

1. **Engine unit/property tests, no server.** Generate random commit streams and
   random query sets; the oracle is recompute-from-scratch. Invariant: for every
   query at every commit, cached answer == oracle answer. This catches the entire
   class of bugs the prototype shipped with.
2. **Integration against `TerminusDBServer`** (`test_instance()` + `with_db_seed`,
   per `CLAUDE.md`): mutate, assert the invalidation happened, assert the
   recomputed answer matches a direct origin query. Parallel-safe, no `#[ignore]`.
3. **Run the existing client test suite *through the proxy*.** Point
   `TerminusDBHttpClient` at `tdb-reactor` and run all ~255 `crates/client`
   tests. M1's gate is "all green in passthrough"; every later milestone must keep
   it green. This is the cheapest possible proof of byte-faithfulness and it
   already exists.
4. **Shadow/verify mode.** In production, serve from cache *and* asynchronously
   re-execute against **the local node** at the same pinned commit, comparing
   bytes. Pinning is what makes this valid under eventual consistency: comparing
   against "whatever the origin says now" would produce false alarms every time
   replication moved, whereas comparing at a pinned commit is an exact test. (A
   divergence found on one node should also be checked against a sibling at the
   same commit — if the two replicas disagree at the same commit id, the bug is
   not ours.) Report divergence with the full dependency trace. Run it at a
   sampled rate forever, at 100% during rollout. A cache you cannot audit online is a cache
   nobody will trust with a publication pipeline.

---

## 8. Build vs port: three options

**Option A — run real Skip, write a `TerminusDBExternalService`.**
~200 lines of TypeScript implementing `ExternalService.subscribe`/`unsubscribe`
over `_ChangeSet` SSE, modelled on `PostgresExternalService`. You get Skip's
runtime, resources and streaming immediately. Costs: a Node/Bun process beside
TDB, the wasm runtime's 32-bit address space (or the harder native Skiplang
install), TS service definitions, and — decisively — **it does not cache the
TerminusDB API at all**. It gives reactive views to apps written against Skip,
not transparent speedup to apps written against TDB.
*Verdict: not the product, but a genuinely cheap way to validate the model and to
borrow Skip's resource semantics. Worth a weekend as a side quest.*

**Option B — port Skiplang + the Skip runtime to Rust.**
The native runtime is implemented in Skiplang itself: persistent data structures,
an MVCC heap, its own GC, plus a compiler that statically enforces purity. That
last part is the bit we cannot reproduce — Rust gives us no transitive-purity
checker, so a port inherits the engineering cost without the guarantee.
*Verdict: no. We need Skip's **model**, not its **runtime**.*

**Option C (recommended) — a Rust-native wrapper that implements the model.**
Reuse what the workspace already has: `terminusdb-client` (subscriptions, diff,
changes, history, pinned reads, typed documents), `terminusdb-schema`/`gql`
(class frames, the VALUE-vs-LINK distinction, introspection), `woql2` (an
analyzable query AST), `terminusdb-bin` (embedded server for tests). Take Skip's
model and salsa's mechanics. Expose Skip's resource API shape so Option A's
ecosystem still interoperates.

---

## 9. Milestones

Each milestone is independently shippable and each one's gate is "the client
suite still passes through the proxy".

| M | Deliverable | Gate |
|---|---|---|
| **M0** | The eight spikes in §4 — **starting with S1 and S2 (does replication emit events; are commit ids identical across replicas), because both can reshape §5.7** — plus a two-node harness and numbers for `_ChangeSet` at scale, diff cost, reconnect gaps, subdocument semantics | written answers + numbers |
| **M1** | Byte-faithful passthrough proxy: axum, streaming, auth, `Cache-Status`, node identity + `X-Reactor-Node`/`-Head` + `GET /_reactor/head`, metrics, watermark tracking from write responses. **No caching.** | all ~255 client tests green through the proxy, run against a 2-node replica pair |
| **M2** | Immutable cache: pinned-commit requests + ETag/304. Zero invalidation logic — and therefore already multi-node-safe. | measurable hit rate on a time-travel workload; suite green; same fragment served from both nodes |
| **M3** | `capture` pipeline behind the `CommitSource` trait — **both** subscription and head-poll drivers — gap recovery, non-fast-forward detection, diff-based changesets → terms → index → invalidation. Document API caching. `min-version` fencing with all three miss policies. | invalidation integration tests on a replica pair; shadow mode reports zero divergence; a pull-induced head move invalidates correctly |
| **M4** | GraphQL: normalize, inject `_id`/`_type`, extract terms, whole-response caching | Stellate-parity on a real app workload |
| **M5** | Fragment decomposition + composition + backdating. The russian doll. | one-chunk-changed rebuild touches O(1) fragments, not O(chunks) |
| **M6** | WOQL AST analysis + caching | property tests vs oracle on generated WOQL |
| **M7** | Resources: declarative views, eager maintenance, SSE streams, subscription multiplexing with working filters | Skip-shaped API; a demo app |
| **M8** | Eviction/memory budgets (Noria-style partial state), **shared immutable fragment tier (§5.7 mode 2)** if measurements justify it, optional write routing, ops hardening | soak test under memory pressure; cross-node reuse rate |

A realistic read: M1–M3 is the real work and the real risk; M2 delivers value
before any of the hard parts exist; M5 is where it stops being "a GraphQL CDN"
and becomes the thing the prototype was reaching for. Note that horizontal
scaling costs almost nothing extra in M1–M7 — one Reactor per node with a private
cache is the *simplest* topology, not a compromise — and only M8's optional
shared tier is genuinely new work attributable to it.

---

## 10. Risks and open questions

**Risks**

- *Change capture is still the single point of failure.* Mitigation: the diff/
  changes/log backstop is not optional, gap recovery is tested by fault
  injection, and shadow mode is the safety net. If the subscription proves
  unreliable, degrade to polling `/api/log` — slower, but the design does not change.
- **Replication may be invisible to `_ChangeSet`** (§4, spike S1). This is the one
  new risk horizontal scaling introduces, and it is a real one: on a read replica
  every change arrives by replication, so a silent replication path would leave
  the subscription driver with nothing to do. Mitigation is structural rather
  than hopeful — the `CommitSource` trait ships with a head-poll driver in M3,
  so the worst case is "replicas have poll-interval staleness instead of
  push latency", not a redesign.
- *Cross-node read-after-write.* A client that writes via node A and reads via
  node B can be served data older than its own write — by the *origin*, not by
  us. Mitigation: `X-Reactor-Min-Version` with the `fail` policy so the LB
  retries, published head state for affinity-aware routing, and optional write
  routing. Worth being blunt with consumers about: the cache cannot make an
  eventually-consistent cluster linearizable, and should not be asked to.
- *Read-set over-approximation on `_path_to_C` and unanalyzable WOQL* could make
  the cache thrash. Mitigation: coarse terms are counted in metrics, so we can
  see which shapes are poisoning hit rates and either special-case or bypass them.
- *Memory.* We cannot mirror a large database. The design is partial-state by
  construction (fragments only for what was asked, before-state from diffs rather
  than a mirror) — but this needs the soak test in M8 before anyone believes it.
- *Non-fast-forward branch movement* (rebase/reset/squash/apply, and now also
  routine `pull`s on replicas) is the ugliest correctness corner. Default to
  dropping the namespace's **node-local** state; optimize later. Immutable
  fragments are never dropped, because they were never wrong.
- *Schema migration* invalidates shapes, term vocabulary and the subscription
  query at once. Treat a schema commit as a namespace-wide barrier.
- *Scope.* This is a database-adjacent cache with a consistency model. It is not
  a weekend project, and a half-built one is worse than none. M1/M2 are designed
  so that "we stopped after M2" still leaves something useful.

**Questions for you**

1. **Target workload.** Is the first consumer the pubbin/Fonto publication path
   (deep document trees, chunk-level writes, few writers — ideal for russian
   doll), or a general-purpose cache for any TDB app? That changes whether M4
   (GraphQL responses) or M5 (fragments) comes first.
2. **Protocol priority.** GraphQL first, or the document API first? The document
   API is far easier to analyze and is what the old prototype actually cached.
3. **How does the cluster actually replicate, and are replicas read-only?**
   Scheduled `pull` from a primary, push-on-commit, or something else? And do
   writes go to one designated node or to any node (which allows divergent
   branch histories that later reconcile — survivable, but it makes
   non-fast-forward head movement the common case rather than the exception)?
   This is the one answer I need before M3's design is final.
4. **Do we want the Skip resource API at all**, or is transparent caching the
   whole ask? M7 roughly doubles the surface area.
5. **Staleness budget.** Is "bounded staleness with read-your-writes on the same
   node" acceptable, or does some path need §5.5 (5)'s `fail` policy plus
   affinity routing from day one? And does anything need strict snapshot
   consistency across a whole composed response (§5.5 (7))?
6. **Shared fragment tier: wanted, or deliberately not?** You said a private
   per-node view is fine, which lets us skip it entirely. I'd default to private
   (§5.7 mode 1) and only add the shared tier in M8 if fill cost measured across
   nodes justifies the dependency. Confirm that's the preference.
7. **Name.** `terminusdb-reactor` / `tdb-reactor`, or something else?

---

---

## 11. Design revisions after architecture review

An adversarial review of §5–§6 found two outright errors and several
improvements. The implementation (`crates/reactor/SPEC.md`) follows the revised
design; this section records what changed and why, so §5 can be read with these
amendments in mind.

**R1 — Commit ids carry no order (error in §5).** TerminusDB commit ids are
*content hashes*. §5 treated "commit id" as the revision counter and spoke of
`W' <= W`, which is not a comparison that exists. The revision counter is
Reactor-local: `Position = (Epoch, Seq)`, totally ordered, with the watermark
being `(Position, CommitId)`. `Epoch` is bumped on non-fast-forward head
movement. Nothing compares `CommitId`s.

**R2 — One `computed_at` per fragment is not enough (error in §5.1).** Each
fragment version needs a validity interval `[from, to)` over positions, `to =
None` while current — i.e. exactly the revision lists Skip uses, which §3
described and §5 then quietly dropped. This single change answers three separate
questions at once: snapshot reads (a reader latches a position and accepts
versions covering it), the fill/apply race (R3), and multi-version GC (drop
versions below the minimum latched position).

**R3 — The fill/apply race is now solved exactly, not probabilistically.** A
fill pinned at position `P` may complete after a commit at `K > P` has already
fired terms that intersect the fill's read set. Because all mutable state for a
namespace is owned by **one writer**, and fill completions are delivered to the
*same mailbox* as commits, the two are totally ordered, and on insert the writer
replays the fired-term log over `(P, head]` against the new dependencies:

- no intersection → store `[P, None)` and register the dependencies;
- intersection at `K` → store `[P, K)`, still serviceable to readers latched
  below `K`, but **do not** register it as current;
- `P` older than the log's retention, or a stale epoch → return the bytes to the
  requester and store nothing.

This is the memcache *lease* idea made exact, and it is the part of the design
most worth testing with a property test.

**R4 — Russian doll by reference, not by transitive dirtying.** If a parent
stores a skeleton plus child *keys*, and children are resolved at compose time,
then a child change does not change the parent's bytes at all and no transitive
dirty propagation is needed — only directly-hit fragments go dirty. Backdating
then matters only for the assembled response's ETag. Strictly simpler than §5.4
and strictly better. *Scope note:* the first implementation caches whole
responses and composes by reference only for the NDJSON document-list case,
where composition is concatenation. GraphQL body splicing is deferred, and
flagged as such in the crate README rather than half-built.

**R5 — Capture is three roles, not one trait (refines §6.1).**
`CommitSource` merely *notices* commits and is explicitly untrusted:
at-least-once, possibly out of order, possibly ahead of the local node's disk.
`ChangeResolver` turns a noticed commit into a `ChangeSet` by local pinned
reads. `ReadinessGate` is the canary that gates the watermark. An `Applier`
owns correctness and dispatches on **parent linkage**: parent equals the
watermark → apply; commit already applied → drop as a duplicate; unknown parent
→ gap, so walk `/api/log` back to the watermark and apply in order. That
log-walk reconciler is shared by every driver and is the only part that has to
be right — which is what lets Kafka and hypercore drivers drop in later without
redesign.

**R6 — Strict linear chain per branch, plus an epoch — not a commit DAG.**
TerminusDB commits are single-parent — verified: `LogEntry.parent` is a single
`Option<String>` (`crates/client/src/log/entry.rs:39`), and rebase/apply mint new
commits — so per-branch history is linear and the DAG only exists across
branches. (`LogEntry` also carries a `migration: Vec<Migration>` field, which is
a cheap schema-commit detector for R9.) Modelling
the DAG now buys nothing; `NonFastForward { old_head, new_head }` as a
first-class event that bumps the epoch buys everything. Immutable fragments
survive it and are re-adopted by content hash.

**R7 — Read-your-writes needs an explicit fence (sharpens §5.5 (2)).** A write
forwarded through the proxy returns *before* capture has applied it, so "we saw
the commit id" is not by itself read-your-writes. The write response is held
until the watermark reaches the commit in its `TerminusDB-Data-Version` (default
2s timeout, configurable, off-switchable). On a reader node that becomes
wait-with-timeout, which is the honest behaviour.

**R8 — Fill mechanics.** Recursive async with a single-flight map keyed by
`(FragmentKey, Position)`; fills **detached** onto their own task so a client
disconnect does not cancel a fill every other waiter is blocked on; child
fan-out through bounded `buffer_unordered` behind a global upstream semaphore;
a dataloader-style coalescer batching sibling document fills at the same commit
into one pinned multi-id request (without it, N+1 fan-out is the dominant cost);
a path set to catch cycles in the document graph; `Box::pin` for the recursion;
and errors are **never** memoized.

**R9 — A diff size threshold.** Above a configurable change count,
`/api/diff` per commit stops being worth it: fire coarse terms (`Members`/
`Class`, or `Everything`) instead and accept the over-invalidation. A schema
commit flushes the namespace.

**R10 — The engine is a projection of an append-only log.** Persisting
`(Position, CommitId, parent, fired_terms)` append-only makes the term index and
the "current version" pointers *projections*, which gives restart recovery,
audit, and the R3 check from one structure instead of three.

Two framings worth keeping from the review: differential dataflow and timely are
the wrong tool here (they want complete input collections and eager maintenance;
our values are opaque, demand-driven HTTP bodies), and the right frame is **a
validity-interval cache with invalidation tags over an event-sourced log** —
closer to TxCache (Ports et al., OSDI 2010) than to an IVM engine.

## Sources

- [Skip — Introduction](https://skiplabs.github.io/skip/docs/introduction) · [Services and Resources](https://skiplabs.io/docs/resources) · [Writing functions](https://skiplabs.io/docs/functions) · [External data sources](https://skiplabs.io/docs/externals) · [Client connections](https://skiplabs.io/docs/client) · [Glossary](https://skiplabs.io/docs/glossary)
- [SkipLabs/skip on GitHub](https://github.com/SkipLabs/skip) · [Reactivity Overview (how memoization works, 2017)](https://skiplang.com/blog/2017/01/04/how-memoization-works.html) · [skiplang.com](https://skiplang.com/) · [SkipLabs](https://skiplabs.io/) · [Cache invalidation and reactive systems](https://medium.com/skiplabs/cache-invalidation-and-reactive-systems-7a80dd4e156a) · [How SkipLabs is building React for the backend](https://www.amplifypartners.com/blog-posts/how-skiplabs-is-building-react-for-the-backend)
- [Noria: dynamic, partially-stateful dataflow (OSDI'18)](https://pdos.csail.mit.edu/papers/noria:osdi18.pdf) · [Morning Paper write-up](https://blog.acolyer.org/2018/10/29/noria-dynamic-partially-stateful-data-flow-for-high-performance-web-applications/) · [Micah Lerner's summary](https://www.micahlerner.com/2021/03/28/noria-dynamic.html)
- [Stellate: GraphQL edge caching](https://stellate.co/blog/akamai-caching) · [Stellate WordPress plugin (type-level purges)](https://stellate.co/docs/graphql-edge-cache/integrations/wordpress) · [A stale-cache bug caused by missing `id` fields](https://blog.vicentereyes.org/how-i-fixed-the-hashnode-graphql-api-stale-cache-bug-stellate-cdn)
- [salsa 0.28 docs](https://docs.rs/salsa/latest/salsa/) · [salsa-rs/salsa](https://github.com/salsa-rs/salsa)
- TxCache — Ports, Clements, Zhang, Madden & Liskov, [*Transactional Consistency and Automatic Management in an Application Data Cache*](https://pdos.csail.mit.edu/papers/txcache:osdi10.pdf), OSDI 2010 (validity intervals + invalidation tags: the closest match to §11's revised model)
- In-repo: `docs/terminusdb/graphql-subscriptions.md`, `docs/terminusdb/graphql-query-reference.md`, `docs/openapi.yaml`, `crates/client/src/http/{subscription,v12_1,diff,graphql,url_builder}.rs`, `crates/client/tests/test_v12_1_graphql_subscriptions.rs`, `crates/woql2/src/query.rs`, `crates/gql/src/frames.rs`
- Prototype: `pubbin/reference-impl/proxy/proxy/lib/module/cache/src/{lib,git_sync,macros}.rs`, `instance/aws/src/{cache/*,cache_impl.rs,backend/cache.rs,background/cache_change_listener.rs,fairings/etag.rs}`
