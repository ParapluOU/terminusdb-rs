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
4. **The watermark only advances past a commit we have actually read.** Capture
   lag then costs staleness, never correctness — which is what makes a
   shared-disk reader node safe even if its filesystem view is strange.
5. **Soundness over precision.** An analyzer that cannot prove a tight term
   emits a coarser one, or bypasses. Over-invalidation is a performance bug;
   under-invalidation serves wrong bytes.
6. **Unknown routes are forwarded, never cached.** The default never inverts.
7. **Never memoize an error.**

## Status

All five phases of [`SPEC.md`](SPEC.md) are implemented. **403 tests pass,
including integration tests against an embedded TerminusDB 12.1.0-rc3 server.**
Not production-ready — see the scope limits below — but soundness *and*
precision are now both demonstrated.

| Module | What | State |
|---|---|---|
| `ids`, `term`, `schema_index`, `change` | identity, the invalidation algebra, `fire()` | done |
| `engine/*`, `store/*` | the dependency graph, single writer per namespace, content-addressed bodies | done |
| `analyze/*` | request → plan, for document / GraphQL / WOQL | done |
| `capture/*` | `CommitSource` / `ChangeResolver` / `ReadinessGate` / `CommitHistory` + applier | done |
| `frontend/*`, `config`, `main` | the proxy itself | done |
| `tests/*` | fidelity, caching, auth, capture, spikes | done |

### What is cached, and what has to be asked for

**`/api/document` is fully cacheable, with no cooperation from the client.** Its
read set is derivable from the request: `id=` is one document, `type=` is a
class's membership, `graph_type=schema` is the schema. GraphQL likewise, because
Reactor injects `_id`/`_type` and maps the response back to documents.

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

## Running it

```bash
source ~/.claude/tdb-build-env.sh          # this host needs it; see CLAUDE.md
cargo run -p terminusdb-reactor -- --config crates/reactor/reactor.example.toml
```

Point your clients at Reactor's `listen` address instead of TerminusDB's, and
the load balancer at the Reactors rather than the nodes. `reactor.example.toml`
documents every knob.

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
