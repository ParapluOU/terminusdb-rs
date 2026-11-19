# Reactor

A reactive, graph-tracked cache engine for TerminusDB 12. Reactor runs as a
**sidecar in front of one TerminusDB node**, passes the whole HTTP API through
byte-for-byte, caches what it can *prove* it may cache, and learns what to
invalidate by watching commits.

It is a Rust realisation of the [SkipLabs Skip](https://skiplabs.github.io/skip/docs/introduction)
reactive-cache model with TerminusDB as the data source. Full rationale, prior
art and the consistency model: [`docs/reactive-cache-design.md`](../../docs/reactive-cache-design.md).
The implementation contract is [`SPEC.md`](SPEC.md).

## Why this is a good fit

Skip's runtime is a demanded-computation graph of memoized, side-effect-free
derivations over input cells, under MVCC revisions. Everything in that sentence
has a TerminusDB counterpart, and the two expensive ones come for free:

| Skip | TerminusDB / Reactor |
|---|---|
| global transaction id | **commit id** — already a durable total order per branch |
| cell revision `[begin, end)` | a pinned-commit read *is* the snapshot read, natively |
| mutator commit → invalidate → bump | commit lands → fired terms applied → watermark advances |
| `ExternalService.subscribe` | `_ChangeSet` SSE, `/api/changes`, `/api/log` |
| traces (false-positive avoidance) | a content hash per fragment (salsa's backdating) |

## The seven invariants

Everything else follows from these, and each is load-bearing:

1. **Byte-faithful.** Bodies are `Bytes`, never round-tripped through
   `serde_json::Value`. v12 returns numbers at full rational precision and this
   workspace enables `serde_json/arbitrary_precision`; re-serializing would
   reorder keys and could corrupt values. We parse to *inspect*, never to re-emit.
2. **Fills are pinned.** Every fill is issued at `…/local/commit/<id>`, so a
   fragment is exactly "the value at that commit", never a read torn across two.
3. **Commit ids carry no order.** They are content hashes. Ordering is a
   Reactor-local `Position = (Epoch, Seq)`; nothing compares `CommitId`s.
4. **The watermark only advances past a commit we have actually read, and whose
   invalidations we have actually applied.** Capture lag then costs staleness,
   never correctness — which is what makes a shared-disk reader node safe even
   if its filesystem view is strange. The second half is what makes indexing
   work (predicate evaluation) safe to be slow: the watermark *lags*, so a read
   in that window is answered from an older but self-consistent snapshot, and
   never told it is current when it is not.
5. **Soundness over precision.** An analyzer that cannot prove a tight term
   emits a coarser one, or bypasses. Over-invalidation is a performance bug;
   under-invalidation serves wrong bytes.
6. **Unknown routes are forwarded, never cached.** The default never inverts.
7. **Never memoize an error.**

## Status

All five phases of [`SPEC.md`](SPEC.md) are implemented, russian-doll
decomposition of document lists included. **565 tests pass (493 unit, 72
integration against an embedded TerminusDB 12.1.0-rc3 server.)** Not
production-ready — see the scope limits below — but soundness, precision *and*
fragment-level reuse are all demonstrated.

| Module | What | State |
|---|---|---|
| `ids`, `term`, `schema_index`, `change` | identity, the invalidation algebra, `fire()` | done |
| `predicate`, `engine/predicates` | filter predicates: the IR, the evaluator verified against the server, and the refcounted `(class, property)` dispatch index | done |
| `engine/*`, `store/*` | the dependency graph, single writer per namespace, content-addressed bodies | done |
| `analyze/*` | request → plan, for document / GraphQL / WOQL | done |
| `compose` | russian-doll decomposition: a list is a spine over per-document fragments | done |
| `capture/*` | `CommitSource` / `ChangeResolver` / `ReadinessGate` / `CommitHistory` + applier | done |
| `frontend/*`, `config`, `main` | the proxy itself | done |
| `tests/*` | fidelity, caching, dependencies, predicates, decomposition, auth, capture, spikes | done |

### What is cached, and what has to be asked for

**`/api/document` is fully cacheable, with no cooperation from the client.** Its
read set is derivable from the request: `id=` is one document, `type=` is a
class's membership, `graph_type=schema` is the schema. GraphQL likewise, because
Reactor injects `_id`/`_type` and maps the response back to documents.

**And a document list is not cached as one blob.** `GET /api/document/<ns>?type=Note`
is cached as a *membership spine* over **one fragment per document**, so a field
change to one member costs one child refill rather than the whole list — see
[Russian-doll decomposition](#russian-doll-decomposition) below.

**Counts and id lists depend on membership, not on field values.** This is the
other half of the same idea, and it matters because applications (and
`terminusdb-orm`) read in two steps: first ask *which* documents, then batch-load
*what is in them*. The two questions have different dependencies and Reactor
keeps them apart:

| Read | Depends on |
|---|---|
| `_count(Person: {})`, `?type=Person&counted=true` | `Members(Person)` — a create or a delete, nothing else |
| `_count(Person: {name: {eq: "Ann"}})` | `Index{Person,name,"Ann"}` — a document entering or leaving the bucket |
| `_count(Person: {age: {ge: "30"}})`, `_and`/`_or`/`_not` over the same | `Predicate(…)` — a document entering or leaving *that set*, decided per commit |
| `{ Person { _id tickets { _id } } }` | membership, plus the traversed properties and their targets' membership |
| `{ Person { _id name } }` | the above **and** every returned document's contents |

So a total count survives every field edit — the case it most needs to survive,
since paging metadata is re-read on every write — while an id-only GraphQL
selection survives field edits to the very documents it lists, and still dies
when an edge is re-pointed, a member appears, or a target is deleted. Put one
scalar field anywhere in the selection and it goes back to depending on field
values, because then a field value is in the response. Reactor requires a
*proof* of id-only-ness: an unknown field, a backlink or a `_path_to_` is enough
to fall back, because under-invalidating an id list serves stale ids.

**A filter is indexed as a predicate, not as a value.** A single `eq` names a
bucket, so `Index{C,p,v}` can express it. Nothing else can: a range, a
conjunction, a negation selects a set no term can name, and until now every one
of them fell back to `Class(C)` — fired by *any* field change to *any* instance,
so filtered lists and the counts over them thrashed on every write.

Reactor instead indexes the **predicate**. Capture already hands the writer the
whole before and after document for every change, so membership can be decided
rather than guessed:

```text
entered   = !member(before) &&  member(after)
left      =  member(before) && !member(after)
unchanged =  member(before) == member(after)   // fires nothing
```

with `member(doc) = doc exists && pred(doc)` — which is why a create and a
delete need no special case. Each live predicate is registered under the
`(class, property)` pairs it mentions, so a commit only evaluates predicates
mentioning a property that actually moved, and conjunctions and negations are
exact because we hold every field and not just the one that changed.

Three things bound it:

* **Agreement with the server is the gate.** An evaluator that disagrees about
  collation, numeric coercion or a missing field under `ne` produces *silent*
  staleness. So only operators and declared ranges verified against a live node
  are indexed — `eq`/`ne`/`lt`/`le`/`gt`/`ge` plus `_and`/`_or`/`_not`, on
  `xsd:string`, `xsd:boolean`, `xsd:double`, `xsd:int` and `xsd:integer` — and
  `tests/predicates.rs` re-checks every one of them against the server's own
  filtered answer. `regex`, `startsWith`, the collection operators, nested
  filters across links and several ranges the server mishandles are
  **unindexable by construction** and keep `Class(C)`.
* **Backpressure degrades precision, not latency.** Over
  `[predicates].max_evaluations_per_commit` the commit invalidates every
  predicate on the classes it touched instead — as coarse as before, bounded,
  and counted as `reactor_predicate_evaluations_skipped`.
* **The watermark waits for it.** Evaluation runs on a worker pool, but the
  writer publishes the watermark only after the invalidations are applied, so
  slow indexing makes the watermark *lag* rather than advance past work it has
  not done. See "Consistency" below.

**WOQL is opt-in.** A WOQL query can return data without revealing which
entities the data was derived from:

```text
select ?n where { alice friend ?f . ?f name ?n }
```

The answer is a list of names. Nothing in it identifies `?f`, so renaming one of
those friends changes the answer with no trace in the cached bytes — and the AST
is no help either, because the subject is a variable. Our analyzer is *sound*
about this (the regression test
`a_traversal_through_unnamed_documents_depends_on_everything` proves it emits the
`Everything` barrier), but soundness is not the issue: an entry that depends on
`Everything` is filled, stored and then discarded by the very next commit in the
namespace. Overhead and an extra thing to get wrong, for no hit rate.

So a WOQL read is cached **only** when the caller states what invalidates it:

```http
POST /api/woql/admin/mydb
X-Reactor-Invalidate-On: class:Person, members:Company
```

From Rust, additively and without hand-writing the header:

```rust
use terminusdb_client::CacheCondition;

let result = client
    .query_with_conditions(Some(spec), query, &[CacheCondition::class("Person")])
    .await?;
```

Three rules, all of them deliberate:

* **No conditions ⇒ forwarded uncached.** The safe default, not a failure.
* **An unparseable condition ⇒ forwarded uncached**, and said so in
  `Cache-Status`. A condition we misunderstood is worse than none, because we
  would cache against a term that never fires.
* **A condition cannot widen what is allowed.** The AST analysis still refuses a
  query carrying a write (that becomes a forwarded write) or one reading another
  database via `Using`/`From`/`Into`. A client may say *when* an answer dies; it
  may not thereby make a mutation or a cross-namespace read cacheable.

A commit-pinned WOQL query (`…/local/commit/<id>`) is cached with no condition,
because nothing can change under it.

Stating a *wrong* condition is the one way to get stale data out of Reactor, and
it is the caller's to get right: name every class the query touches.

The condition grammar (`Term`'s `Display`/`parse`, round-tripped in both crates):

```text
doc:Person/alice          docfield:Person/alice.name   edge:Person/alice.friend
members:Person            class:Person                 classfield:Person.name
index:Person.name="Ann"   backlink:Person/alice.friend<-Person
path:Person,Company       schema                       everything
```

`index:` values are canonical JSON, so `=30` is the number and `="30"` is the
string — a bare `=Ann` is rejected rather than guessed at, because guessing the
type wrong produces a condition that silently never fires.

### Russian-doll decomposition

Caching a document list as one response is nearly useless: a field change to any
single member throws the whole list away, and the list read is the hot path. So
a `/api/document` read that selects documents by `id`, `ids` or `type` is cached
as **one fragment per document plus a parent that holds only the ordered
membership and the framing** — design doc §5.4 as revised by R4, and the thing
the prototype this project replaces did by hand for exactly one route.

The split that makes it work is **projection versus framing**:

* *Projection* is which bytes one document renders as — `unfold`, `minimized`,
  `compress_ids`, `identifiers`, `raw_id`, `xsd`, `format`. It lives in the
  **child**'s shape.
* *Framing* is how documents are joined — NDJSON versus `as_list`, plus
  `skip`/`count` and the origin's order. It lives in the **parent**.

Three consequences, in descending order of how easy they are to break:

1. **The parent depends on membership only** — `Members(C)`, never
   `Term::Class(C)`. `Class` fires on every field change to every instance, so a
   parent holding it would be refetched by precisely the change this feature
   exists to absorb, and the cache would look correct while doing nothing. There
   is a comment saying so where it matters, because it is the easiest thing for
   someone to "fix" later.
2. **Children are shared.** A by-id read and a list read compute the *same*
   child key, so a list read warms by-id reads and vice versa. A by-id read of a
   member the list already fetched is a hit with no upstream request at all.
3. **One request, not N.** A parent with a hundred dirty children costs one
   pinned `?ids=[…]` fetch. A composition whose children cannot all be resolved
   falls back to a whole upstream fetch — never a partial body.

Framing is **learned from the origin, never assumed**: `compose.rs` keeps the
verbatim bytes before, between and after the documents it found, and refuses a
body whose separators are not uniform. Nothing in the code knows what `[` means,
which is why `minimized=false`, `as_list=true` and `unfold=true` needed no
special cases — and why `tests/decompose.rs` asserts, against a live node, that
a composed response equals the origin's bytes for every one of them.

Two findings came out of building it, both pinned by tests:

* **`?ids=` wants a JSON array.** `?ids=["Note/1","Note/2"]` works;
  `?ids=Note/1,Note/2` is a 400 (`api:JSONInvalid`). The analyzer used to split
  that parameter on commas, which produced a dependency term no commit can ever
  fire — so an `?ids=` entry was cached and then never invalidated. Stale
  forever, and invisible. Fixed, and `the_origin_wants_ids_as_a_json_array`
  keeps it fixed. (Nothing else in this workspace sends ids through the query
  string: `terminusdb-client` POSTs them in a body with
  `X-HTTP-Method-Override: GET`, which is why it went unnoticed.)
* **A field update does not move the origin's list order**, which is what makes
  the membership-only parent dependency sound. Asserted directly against the
  node in `the_origin_keeps_its_order_when_a_middle_document_changes`, rather
  than assumed.

`[cache].decompose = false` is the escape hatch: it falls back to whole-response
caching, which is unconditionally byte-faithful because the bytes are never
taken apart. Everything still passes with it off.

### What the tests actually prove, and what they don't

**Soundness is demonstrated** against a live node: answers are correct after a
commit, high-precision `xsd:decimal` responses are byte-identical through
Reactor and direct (note what that does and does not say: inserting
`0.1234567890123456789012345` gets `0.1234567890123456789` back, because the
*server* keeps ~19 significant digits. `arbitrary_precision` is doing its job —
f64 would have given `0.12345678901234568` — so the test proves Reactor adds no
corruption of its own, not that full precision survives TerminusDB), wrong credentials cannot read a populated cache,
read-your-writes holds, the poll and subscription drivers both advance the
watermark, a 5-commit gap is recovered in order, and a `reset` is handled as a
non-fast-forward.

**Precision is now demonstrated too**, against an embedded **12.1.0-rc3** server:
a commit to an unrelated document leaves a cached fragment a `hit`, so
"invalidation works" means more than "we drop everything". Getting there took
fixing a real bug that only a 12.1 run could expose — `LogEntry::parent` carries
a `ValidCommit/` prefix, and pinning a before-state read at
`…/local/commit/ValidCommit/<hash>` is rejected as `api:InvalidResourcePath`, so
every before-state read failed, so every changeset was `complete: false`, so
every commit fired `Everything`. A correct cache with no precision, and silent
about it. All commit ids now funnel through one normalizer
(`capture::commit_id_from_any`).

**Fragment-level reuse is demonstrated** on the same server: a composed list is
byte-identical to the origin's for every framing and projection we support; a
change to one member of a four-member list refills exactly one child in exactly
one upstream request; three changes still cost one request; a member the list
warmed is a hit on its first by-id read; inserting and deleting a member moves
the list without ever abandoning a composition; and reverting a change restores
the composed `ETag`, because a composed response's identity comes from its
children's hashes rather than from the spliced bytes.

What is **not** demonstrated: decomposition under concurrent writers at scale
(the single-flight path is exercised by unit tests, not by a live contention
test), and composition for anything other than `/api/document` list reads.

## Adopting it in an application

[`INTEGRATION.md`](INTEGRATION.md) is the adopter's guide: configure, run, point
a client at it, the header protocol, stating WOQL conditions, the operational
endpoints, and a staged rollout. Every command in it was run against a live
12.1 server.

## Running it

```bash
source ~/.claude/tdb-build-env.sh          # this host needs it; see CLAUDE.md
cargo run -p terminusdb-reactor -- --config crates/reactor/reactor.example.toml
```

Point your clients at Reactor's `listen` address instead of TerminusDB's, and
the load balancer at the Reactors rather than the nodes. `reactor.example.toml`
documents every knob.

Three of them bound memory, and they bound different things: `memory_budget_mb`
bounds cached **bodies**, `max_versions_per_key` bounds **versions** within one
key, and `max_tracked_keys` bounds the **number of keys** a namespace tracks.
Only the third bounds growth with the query log — and since a document list is
cached as one fragment per document, that is growth with the number of documents
read. Over the cap the least-recently-used keys are dropped, slot and dependency
index together, while the bodies stay in the content-addressed store: an
eviction costs a refill and nothing else. `reactor_keys_evicted` says whether
the working set is larger than the cap.

```bash
cargo test -p terminusdb-reactor
```

## Horizontal scaling

TerminusDB is scaled out as eventually-consistent replicas. Reactor's guarantee
is therefore **relative, and it is the only honest one**:

> Reactor is never weaker than the TerminusDB node it fronts.

Read-your-writes and monotonic reads hold **per node**. Across nodes, a request
may assert `X-Reactor-Min-Version: <commit>`; if this node has not replicated it,
Reactor will `wait`, `fail` (412, so the load balancer retries elsewhere), or
`serve-stale` with a label — configurable, and never silently pretend.

Both of those rest on `X-Reactor-Head` meaning what it says, which is why
invalidation is applied *before* the watermark is published, for every kind of
dependency including the predicates whose evaluation is the most expensive thing
in the commit path. The cost is **watermark lag**, reported as
`reactor_watermark_lag_ms` and bounded by the predicate evaluation limits: under
load the node serves a slightly older snapshot rather than a confidently wrong
current one.

State splits in two, which is what makes a shared cache optional:

- **Immutable and node-independent:** fragment bodies, content-addressed. A
  commit id means the same thing on every replica holding it, so pointing
  several nodes' `[cache].dir` at one NAS directory shares them with **no
  coherence protocol** — nothing is ever updated in place, so there is nothing
  to coordinate.
- **Node-local and mutable:** watermark, branch heads, term index, eviction
  state. Never shared.

Two nodes at different points in a branch's history hold different *subsets* of
the same immutable fragment space. They cannot hold conflicting entries for one
key, so divergence appears as "this node hasn't got that one yet" and never as
"these two nodes disagree".

On the operator's shared-disk single-writer/many-reader topology, a reader node
has no local write path, so its `_ChangeSet` subscription is expected to be
silent and **head polling is the primary capture driver there**, not a fallback.

## Reading the code

Start with `term.rs`. `Term::subsumes` is the heart of the design — a commit
fires precise terms, a fragment holds possibly coarser ones, so matching is
subsumption rather than equality, and if it is wrong data goes silently stale.
Every row of its truth table is a named test.

Then `change.rs::fire` (commit → fired terms; the reason we pay for before-state
is that it makes `Index` and `Backlink` firing exact), and
`engine/writer.rs::on_insert` — the protocol that makes the fill/apply race
*exactly* solvable rather than merely unlikely.

For the russian doll, read `compose.rs` and then `analyze::ListSpec`. The whole
feature turns on two paragraphs of doc comment: why the parent's dependency is
membership and not the class, and why the framing is learned instead of
written down.

## Known risks

- **Does replication emit change events?** On a shared-disk reader, nothing
  commits locally. Untested against a real two-node setup. Mitigated
  structurally: the head-poll driver is a first-class peer, so the worst case is
  poll-interval staleness rather than a redesign.
- **Are commit ids byte-identical across replicas?** The shared-tier design
  assumes yes (commits are content-addressed). Unverified empirically; if false,
  fragments need keying by `(node, commit)` and the shared tier collapses to
  per-node caches.
- ~~**Is `/local/commit/<id>` accepted by every read endpoint?**~~ **Resolved:**
  yes, GraphQL included (200 with correct data), and every read endpoint returns
  `TerminusDB-Data-Version`, so a fill can always be placed at a known commit.
- ~~**Schema-commit detection**~~ **Resolved:** `LogEntry.migration` *is*
  populated for a schema commit, so the detector is better than feared.
- ~~**Subdocument change reporting**~~ **Resolved on 12.1:** a subdocument
  change is attributed to the **parent** (`changed=["Resident/r1"]`, with the
  subdocument itself not mentioned), so a `Doc(parent)` dependency is invalidated
  by it — exactly what the analyzer assumes.
- **Running Reactor's tests needs an upstream.** The embedded 12.1 server would
  not start here: `terminusdb-bin`'s `extract_binary` compares only file *size*,
  so two differently-built binaries overwrite the same
  `/tmp/terminusdb-bin-cache/terminusdb` and collide (`ETXTBSY`). The harness
  therefore falls back to `REACTOR_TEST_UPSTREAM_PORT`, then to a node already on
  `:6363`. That cache bug lives in `crates/bin` and was left alone.

The full spike list, with what to measure and why, is §4 of the design doc.
