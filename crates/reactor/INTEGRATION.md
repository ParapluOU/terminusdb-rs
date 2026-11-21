# Putting Reactor in front of an application

Reactor is a sidecar: it speaks the TerminusDB HTTP API, forwards everything,
and caches what it can prove it may cache. An application adopts it by pointing
at Reactor's port instead of TerminusDB's. **No client code changes are needed**
for the document and GraphQL read paths.

For what is and isn't cached, see [`README.md`](README.md). For why it is built
this way, see [`../../docs/reactive-cache-design.md`](../../docs/reactive-cache-design.md).

Every command below was run against a live TerminusDB 12.1 server.

## 1. Configure

Copy [`reactor.example.toml`](reactor.example.toml) and edit three things: where
Reactor listens, where its TerminusDB node is, and which namespaces to track.
A minimal config:

```toml
[node]
id = "app-01"                      # reported in X-Reactor-Node
listen = "127.0.0.1:6464"

[upstream]
url = "http://localhost:6363"      # must be local: Reactor issues extra pinned reads
user = "admin"
pass = "root"
org = "admin"

[[namespace]]
org = "admin"
db = "mydb"
branch = "main"
```

**A namespace not listed here is proxied but never cached.** That is the safe
default for an unknown database, and it is also the way to roll out gradually:
add one namespace, watch it, add the next.

Everything else has a default. The one worth sizing deliberately is
`[cache].max_tracked_keys` (50 000): it is how many fragment keys a namespace
keeps, and since a document list is cached as one fragment per document, it is
effectively the size of the working set you are promising to keep warm. Over the
cap the least-recently-used keys are dropped and later refilled — a hit-rate
cost, never a correctness one.

## 2. Run

```bash
cargo run -p terminusdb-reactor -- --config reactor.toml
# or: tdb-reactor --config reactor.toml
```

Then point the application at `http://localhost:6464` instead of `:6363`.

```rust
// Before
let client = TerminusDBHttpClient::local_node().await;
// After — same client, same calls, nothing else changes
let client = TerminusDBHttpClient::new(
    "http://localhost:6464".parse()?, "admin", "root", "admin",
).await?;
```

Reactor forwards the caller's own `Authorization` header and never substitutes
its own credentials, so authentication behaves exactly as before.

## 3. What you will see

A first read misses; the second is a hit with an `ETag`:

```console
$ curl -su admin:root -D - "http://localhost:6464/api/document/admin/mydb?id=Note/1"
cache-status: Reactor; miss
x-reactor-head: v3e37se8odebj16x81gz5abyzmmn8gk

$ curl -su admin:root -D - "http://localhost:6464/api/document/admin/mydb?id=Note/1"
cache-status: Reactor; hit
etag: "fde06f8d2aca22793890b25e83b7d2b40a86d427b0d1a1e86dde3b9fdc475ba7"
```

A conditional request is answered without a body:

```console
$ curl -su admin:root -o /dev/null -w '%{http_code}\n' \
    -H 'If-None-Match: "fde06f8d…"' \
    "http://localhost:6464/api/document/admin/mydb?id=Note/1"
304
```

Writing through Reactor invalidates what the write touched:

```console
$ curl -su admin:root -X PUT "http://localhost:6464/api/document/admin/mydb?author=a&message=m" \
    -H 'Content-Type: application/json' \
    -d '{"@type":"Note","@id":"Note/1","body":"CHANGED"}'

$ curl -su admin:root "http://localhost:6464/api/document/admin/mydb?id=Note/1"
cache-status: Reactor; miss          # and the body is the new one
```

A document list is cached as a spine over one fragment per document, so a change
to one member costs one child refill rather than the whole list:

```console
$ curl -su admin:root -D - "http://localhost:6464/api/document/admin/mydb?type=Note"
cache-status: Reactor; miss                     # fetched and split into children

$ curl -su admin:root -D - "http://localhost:6464/api/document/admin/mydb?type=Note"
cache-status: Reactor; hit; detail=composed     # assembled from children, no upstream read

# ... after a PUT that changes one Note ...
$ curl -su admin:root -D - "http://localhost:6464/api/document/admin/mydb?type=Note"
cache-status: Reactor; miss; detail=recomposed  # ONE `?ids=[…]` fetch for the changed member
```

`detail=composed-shared` on a `?id=` read means it was answered out of a child
the list read had already warmed, with no upstream request at all. The bytes are
the origin's either way — `tests/decompose.rs` compares them against the node
directly for every framing and projection combination.

A filtered read survives a write that does not change *which* documents it
selects, even when the filter is a range or a conjunction:

```console
$ curl -su admin:root -D - -X POST "http://localhost:6464/api/graphql/admin/mydb" \
    -H 'Content-Type: application/json' \
    -d '{"query":"query { _count(Note: {priority: {ge: \"3\"}}) }"}'
cache-status: Reactor; miss

# ... a PUT that moves a Note's priority from 5 to 4 — still >= 3 ...
$ curl -su admin:root -D - -X POST "http://localhost:6464/api/graphql/admin/mydb" \
    -H 'Content-Type: application/json' \
    -d '{"query":"query { _count(Note: {priority: {ge: \"3\"}}) }"}'
cache-status: Reactor; hit           # membership did not move, so nothing fired

# ... a PUT that moves it to 1 ...
cache-status: Reactor; miss          # it left the set
```

Which filters get that treatment is **not** a matter of degree: see the limits
section at the end. A filter Reactor has not verified against the server keeps
the coarse dependency and behaves as it did before — correct, and discarded by
any write to the class.

`cache-status: Reactor; bypass; detail=…` means Reactor decided not to cache,
and the detail says why (`unknown endpoint`, `unrecognized document parameter`,
`woql needs an explicit invalidation condition`, …). A bypass is always correct;
it only costs a hit.

## 4. WOQL needs an invalidation condition

WOQL can return data without revealing which entities the result was derived
from, so Reactor cannot infer what a WOQL result depends on and **will not cache
it** unless the caller says when it dies. Document reads need none of this.

```rust
use terminusdb_client::CacheCondition;

// Not cacheable: Reactor has no way to know when this answer goes stale.
let rows: WOQLResult<Row> = client.query(Some(spec.clone()), query.clone()).await?;

// Cacheable: "this result dies when any Person changes".
let rows: WOQLResult<Row> = client
    .query_with_conditions(
        Some(spec),
        query,
        &[CacheCondition::class("Person"), CacheCondition::members("Person")],
    )
    .await?;
```

Conditions are sent as `X-Reactor-Invalidate-On: class:Person,members:Person`,
so a non-Rust client can set the header directly (surrounding whitespace around
each term is tolerated). State the **weakest** condition
that is still true: `class:Person` invalidates on any change to any `Person`,
while `doc:Person/alice` only invalidates on that one document. Too weak costs
hit rate; too strong serves stale data. If unsure, use `class:` for every class
the query reads.

A condition that does not parse causes a bypass rather than a cache entry, and a
condition cannot make an unsafe query cacheable — a write stays a write, and a
query reaching into another database via `Using` stays refused.

## 5. Caching what your own code computes

The reads above are Reactor's to analyze. The *transformation* over them is
yours, and [`terminusdb-cache`](../cache/README.md) is how its result joins the
same graph:

```rust
use terminusdb_cache::{derivation, Cache};

derivation! {
    /// One publication's rendered overview.
    pub struct OverviewOf { pub id: String, pub limit: usize }
    -> Overview
    |this, scope| {
        // An ordinary read through an ordinary client — but `scope.client()`,
        // which pins it at the scope's commit and has Reactor record what it
        // depended on.
        let publication = scope
            .client()
            .get_document(&this.id, scope.spec(), GetOpts::default())
            .await?;
        Ok(render(&publication, this.limit))   // your code; Reactor never sees it
    }
}

let cache = Cache::new("http://localhost:6364", "admin", "root", "admin")?;
let page = cache.get(&spec, OverviewOf { id, limit: 20 }).await?;
```

Nothing is declared: the dependency set is the union of what those reads
depended on, which Reactor reports as they go past. A write to the publication
recomputes the page; a write to anything else does not.

Two things to know before you lean on it. **A computation whose reads Reactor
could not attribute is not cached** — you get your value and a reason
(`Outcome::NotStored`, logged at `warn`, and `GET /_reactor/scope/{id}` shows
the reads), most often because a read did not go through `scope.client()`.
And **the key is yours**: `name/params/epoch` is composed for you, but anything
else that changes what the computation means — the user it is scoped to above
all — has to be a parameter, or two values share one entry.

The crate's README covers the epoch policy (your build id by default, so a
deploy cannot serve values computed by code that no longer exists), nesting for
per-member granularity, and the one hazard it cannot detect: a closure that
reads something outside the scope.

## 6. A database per tenant

If your application *creates* databases — a room, tenant or publication each —
they cannot be listed in the configuration file, so register them at run time:

```bash
curl -u admin:root -X POST http://localhost:6364/_reactor/namespace \
     -H 'Content-Type: application/json' \
     -d '{"org": "admin", "db": "room-8f31"}'
# {"namespace":"admin/room-8f31/local/branch/main","added":true,"tracked":2,"cap":512}
```

Idempotent, so the natural place is the code path that creates the database —
or every request, if that is simpler. `DELETE` the same path to stop tracking
and drop what was cached; `GET /_reactor/namespace` lists what is tracked, where
each entry came from, and how long since a read touched it.

Alternatively set `[capture].auto_register = true` and the first read of a
database registers it. Either way **that first read is a miss**: tracking has to
establish a watermark before anything in the namespace can be cached, and there
is no position to pin a fill at until it does.

Two limits to size up front:

- `[capture].max_tracked_namespaces` (512) bounds how many namespaces have a
  capture task and a writer. Over it, registration is refused and that database
  is proxied uncached — deliberately, rather than evicting a namespace someone
  is using.
- Namespaces nothing has read for `[capture].idle_after_ms` are polled at
  `idle_poll_interval_ms` instead of `poll_interval_ms`. With a database per
  tenant that is the difference between one poll loop per *active* tenant and
  one per tenant that has ever existed.

**Do not expect one subscription to cover every database.** The server keys a
`_ChangeSet` cohort by `(database, class, operation, selection, mode)`, so a
subscription covers one database and there is no org-wide stream; a database per
tenant would want one SSE connection per tenant, against a default cap of 20
per *user* (`TERMINUSDB_SSE_MAX_SUBSCRIPTIONS_PER_USER` raises it; Reactor
authenticates as one user, so all of its subscriptions share that budget). And
a slot is never returned once its client goes away — not on a commit, not after
the idle timeout — so the cap is a lifetime budget and each resubscribe spends
one of it ([terminusdb/terminusdb#2673](https://github.com/terminusdb/terminusdb/pull/2673)).
Namespaces registered at run time therefore poll by default
(`[capture].dynamic_driver`).

### Reads that span databases

A view assembled from several databases — a room plus chunks from a shared
library — cannot be a *recorded* derived value: a dependency term carries no
namespace, and a scope is one snapshot, which two independently committing
databases do not share. Two things work instead:

1. **Cache the leaves and recompose per request.** Each database's reads are
   ordinary cached reads as soon as that namespace is tracked, and assembling
   them is cheap next to the round trips it saves.
2. **Store the composition keyed on the revisions it was built from**, with
   `Dependencies::Independent`. Over a known revision set a composition is
   immutable, so it needs no invalidation — a stronger guarantee than any
   dependency term could express, and one your domain already has if it records
   which revisions it composed.

## 7. Request headers an application can send

| Header | Effect |
|---|---|
| `X-Reactor-Bypass: 1` | Skip the cache for this request |
| `X-Reactor-Min-Version: <commit>` | Refuse to answer from state older than this commit (see below) |
| `X-Reactor-Max-Staleness: <ms>` | Accept a cached answer only if younger than this. Applies to every fragment consulted, so a composed list with one stale member is refilled rather than reported fresh |
| `X-Reactor-Invalidate-On: <terms>` | State a WOQL result's dependencies |
| `X-Reactor-Explain: 1` | Report the plan for this request |
| `X-Reactor-Record: <scope>` | Record this read's dependencies into a scope, and report them as `X-Reactor-Deps` (set for you by `terminusdb-cache`) |

Response headers: `Cache-Status`, `ETag`, `Age`, `X-Reactor-Node`,
`X-Reactor-Head`, and `TerminusDB-Data-Version` as the origin sent it.

## 8. Consistency, stated plainly

> Reactor is never weaker than the TerminusDB node it fronts.

Read-your-writes and monotonic reads hold **per node**: a write through Reactor
is immediately visible to a later read through the same Reactor, because the
write response is held until the cache has caught up (`write_fence_ms`).

Across nodes in a replicated cluster, Reactor cannot be stronger than
replication. If a request carries `X-Reactor-Min-Version` and this node has not
replicated that commit yet, Reactor will — per `[consistency].min_version_policy`
— wait, answer `412 Precondition Failed` so a load balancer retries elsewhere,
or serve stale and label it. It will not silently pretend.

One deliberate consequence of that promise: the watermark in `X-Reactor-Head`
never advances past a commit whose invalidations are still being applied. Some
of that work — deciding which filtered sets a commit moved documents into or out
of — is proportional to how many distinct filters are live, so under a burst of
writes the watermark can **lag** by a few milliseconds. Reads in that window are
answered from the previous watermark: an older, self-consistent snapshot, which
is exactly the bounded staleness `X-Reactor-Max-Staleness` lets a client reason
about. The alternative — advancing the watermark and applying invalidations
afterwards — would make `X-Reactor-Head` claim currency the node does not have,
and would break both read-your-writes and the `X-Reactor-Min-Version` fence.
Watch `reactor_watermark_lag_ms`.

## 9. Operating it

| Endpoint | Purpose |
|---|---|
| `GET /healthz` | liveness |
| `GET /_reactor/head` | per-namespace watermark and position |
| `GET /_reactor/metrics` | Prometheus text |
| `POST /_reactor/explain` | the plan for a request, without executing it |
| `POST /_reactor/invalidate` | drop a namespace's cache |
| `GET /_reactor/fragment/<hash>` | dump one cached entry |
| `GET /_reactor/config` | effective config, password redacted |

```console
$ curl -su admin:root http://localhost:6464/_reactor/head
[{"namespace":"admin/mydb/local/branch/main","commit":"kb630tn8…","position":"1.4",…}]
```

Metrics worth a dashboard: `reactor_hits` / `reactor_misses` / `reactor_bypass`
(by reason — a surprising bypass reason is the usual cause of a low hit rate),
`reactor_commits_applied`, `reactor_fills_backdated` (work avoided because a
recomputed fragment was unchanged), `reactor_subscription_demotions` (a
namespace that gave up its change subscription and fell back to polling — it
still works, with higher latency), and `reactor_watermark_lag`.

For document lists specifically (see §3), the four that say whether
decomposition is earning its keep:

| Counter | What a bad value looks like |
|---|---|
| `reactor_lists_composed` | near zero while list reads are hot: lists are being refetched, not composed |
| `reactor_lists_undecomposable` | rising: the origin's list rendering has moved and decomposition is quietly doing nothing |
| `reactor_children_filled` / `reactor_child_batches` | a ratio near 1 means almost every refill fetches a single document; the batch is not being filled |
| `reactor_compositions_abandoned` | anything but near zero: compositions are falling back to whole fetches |

For filtered reads, two that belong on the same graph:

| Metric | What a bad value looks like |
|---|---|
| `reactor_predicate_evaluations_skipped` | rising: commits are exceeding `[predicates].max_evaluations_per_commit`, so filtered reads on those classes are being invalidated as if unfiltered. Correct, but it is the precision you were paying for |
| `reactor_watermark_lag_ms` | rising: indexing is falling behind, so reads are answered from an older snapshot. This is the quantity the bound above is protecting — if it has headroom, raise the bound; if it does not, lower it |

`reactor_live_predicates` is how many distinct filters are currently indexed,
and `reactor_predicate_evaluations` the work they cost per commit.

And one for memory: `reactor_keys_evicted` rising steadily means the working set
is larger than `[cache].max_tracked_keys`, so keys are being dropped and
refilled. That is correct behaviour (an evicted key is a miss, never a wrong
answer) but it costs upstream requests; raise the cap if there is memory for it.
`reactor_tracked_keys` is the current count.

## 10. Rolling it out

1. Start with `[analyze].strict = true`. Anything Reactor cannot prove it may
   cache is bypassed instead of cached coarsely — lower hit rate, smallest
   blast radius.
2. Track one namespace, with `[capture].driver = "both"`.
3. Watch `Cache-Status` on the routes you care about and the bypass reasons. If
   a route you expected to cache is bypassing, `POST /_reactor/explain` says why.
4. Add `query_with_conditions` to the WOQL calls that are hot enough to matter.
5. Turn `strict` off once the bypass reasons are understood.
6. `X-Reactor-Bypass: 1`, removing a namespace from the config, or stopping
   Reactor and repointing at `:6363` are all instant, total escape hatches.

## Limits worth knowing before adopting

- **Composition covers `/api/document` list reads, and nothing else.** A GraphQL
  or WOQL response is still cached whole, so a change to anything it depends on
  rebuilds all of it. For GraphQL that is deliberate: a nested selection has no
  framing to learn, and splicing one would mean re-emitting a body.
- **Your GraphQL query is not the query Reactor executes.** Reactor adds
  `_id`/`_type` to every object selection so the answer can be attributed to
  documents, and removes them from the response before you see it — so the bytes
  you get are the node's own answer to *your* query. Two things follow. A query
  that selects `_id` itself shares a cache entry with the same query without it,
  which is free hit rate. And if a response ever cannot be edited back
  (`reactor_responses_unstrippable` above zero, which should never happen),
  Reactor forwards your query to the origin uncached rather than hand you extra
  fields.
- **A GraphQL selection that mixes ids and field values depends on both.** Ask
  for ids and traversals alone (`{ Person { _id tickets { _id } } }`) and the
  answer depends on membership and edges only, so it survives every field edit;
  add one scalar (`name`) and it depends on the contents of every document it
  returns. That is not a tuning knob, it is what the response contains — and it
  is why `terminusdb-orm`'s two-step pattern (ids by GraphQL, bodies by
  `get_documents`) caches so much better than one fat query: the id list survives
  writes, and the per-document fragments invalidate one at a time.
- **Only verified filter operators are indexed precisely.** A filter made of
  `eq`/`ne`/`lt`/`le`/`gt`/`ge` and `_and`/`_or`/`_not`, on single-valued
  properties of type `xsd:string`, `xsd:boolean`, `xsd:double`, `xsd:int` or
  `xsd:integer`, is indexed as a predicate and survives writes that do not move
  documents in or out of it. Everything else — `regex`, `startsWith`,
  `allOfTerms`/`anyOfTerms`, `relationship`, `_id`/`_ids`, a filter *through* a
  link (`{author: {name: {eq: …}}}`), a set-valued property, a value passed as a
  GraphQL variable, or two operators on one property — keeps the coarse
  dependency and is discarded by any write to the class.

  That list is short on purpose. The gate is agreement with the server: an
  evaluator that disagrees about a missing field under `ne`, or about
  collation, would serve stale data with nothing to show for it. Several things
  are excluded because this server gets them wrong, not because we did not get
  round to them — `xsd:long` and `xsd:unsignedInt` filters crash it,
  `xsd:float`'s crash it differently, `xsd:decimal`'s and `xsd:dateTime`'s `eq`
  compares the lexical form (`10.250` does not match `10.25`), and on the narrow
  and derived integer types (`xsd:byte`, `xsd:nonNegativeInteger`, …) `eq`
  matches some stored values and not others. Each is recorded in
  `src/predicate.rs` and re-checked by `tests/predicates.rs`.

  Practical consequence for schema authors: a Rust `i32` becomes `xsd:integer`
  and filters precisely; `i64` becomes `xsd:long`, whose filters the server
  cannot answer at all. Prefer `i32` and `f64` for properties you intend to
  filter on. Note also that a `BigInt` filter literal must be written as a
  **quoted** decimal (`{age: {ge: "30"}}`); a bare `30` is a request error.
- **Two operators on one property are not a range.** `{age: {ge: 0, lt: 10}}`
  looks like `0 <= age < 10`; this server honours one of the two and silently
  drops the other. Write `_and: [{age: {ge: 0}}, {age: {lt: 10}}]`, which both
  the server and Reactor read as the conjunction.
- **An ordered page keeps the coarse dependency.** A collection with `orderBy`
  *and* `limit`/`offset` is not determined by membership — a member whose
  ordering key moves can cross a page boundary into a page that never depended
  on it — so such a read is invalidated by any write to the class whatever its
  filter.
- **No eager recomputation.** Invalidation drops an entry; the next reader pays
  to refill it. `policy = "eager"` is refused at startup rather than accepted
  and ignored, as is `shadow_sample_rate` above `0.0` — see README "Known gaps"
  for the full list of what is designed and not built.
- **A value your own code computes can go in the cache**, through
  [`terminusdb-cache`](../cache/README.md): `derivation! { … }` plus
  `cache.get(&spec, …)`. Its dependencies are *observed* — your reads go through
  a scope's client, Reactor reports what each read depended on, and the union
  becomes the value's dependency set — so there is nothing to declare and
  nothing to get wrong. What it will not do is cache a computation whose reads
  it could not attribute; it returns your value and says why, rather than
  caching something it cannot invalidate.
- **Eager recomputation of derived values does not exist either.** The next
  reader after an invalidation pays, and nested derivations inherit their
  children's dependencies rather than being dirtied by them.
- **The shared cache tier is unproven across nodes.** The filesystem store works
  and is unit-tested; pointing two nodes at one directory is designed for and
  untested.
- **`optimize` and the history-rewriting endpoints purge the whole branch.** A
  deliberate choice, but it means a rebase costs the branch's hit rate.
