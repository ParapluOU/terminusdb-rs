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

Phases A–C of [`SPEC.md`](SPEC.md) are implemented and tested; D and E are in
progress. **Not production-ready, and not yet run against a live server** — the
integration tests are Phase E.

| Module | What | State |
|---|---|---|
| `ids`, `term`, `schema_index`, `change` | identity, the invalidation algebra, `fire()` | done |
| `engine/*`, `store/*` | the dependency graph, single writer per namespace, content-addressed bodies | done |
| `analyze/*` | request → plan, for document / GraphQL / WOQL | done |
| `capture/*` | `CommitSource` / `ChangeResolver` / `ReadinessGate` + applier | in progress |
| `frontend/*`, `config`, `main` | the proxy itself | in progress |

### Deliberately out of scope for now

- **GraphQL fragment splicing.** Fragments are whole responses. The types carry
  `children` and the russian-doll composition machinery, and the NDJSON
  document-list case composes by concatenation, but splicing a GraphQL body out
  of per-document fragments is future work. Said plainly rather than half-built.
- **A shared cache tier** is supported but not the default — see below.
- **Kafka and hypercore commit sources.** The `CommitSource` trait exists so they
  drop in without redesign; only head-poll and `_ChangeSet` are implemented.

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
- **Is `/local/commit/<id>` accepted by every read endpoint**, GraphQL in
  particular? The fill path handles both outcomes — it prefers the response's
  `TerminusDB-Data-Version` over the pin it requested, and declines to cache
  anything it cannot place at a known commit — but this wants a real test.
- **Schema-commit detection** keys off `LogEntry.migration` being non-empty,
  which is a partial detector.
- **Subdocument change reporting** is unverified: whether a changed subdocument
  surfaces as its own class, as the parent's `_changed`, or both, decides the
  dependency terms for `@subdocument` trees.

The full spike list, with what to measure and why, is §4 of the design doc.
