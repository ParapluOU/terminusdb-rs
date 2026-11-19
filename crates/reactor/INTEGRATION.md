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

## 5. Request headers an application can send

| Header | Effect |
|---|---|
| `X-Reactor-Bypass: 1` | Skip the cache for this request |
| `X-Reactor-Min-Version: <commit>` | Refuse to answer from state older than this commit (see below) |
| `X-Reactor-Max-Staleness: <ms>` | Accept a cached answer only if younger than this |
| `X-Reactor-Invalidate-On: <terms>` | State a WOQL result's dependencies |
| `X-Reactor-Explain: 1` | Report the plan for this request |

Response headers: `Cache-Status`, `ETag`, `Age`, `X-Reactor-Node`,
`X-Reactor-Head`, and `TerminusDB-Data-Version` as the origin sent it.

## 6. Consistency, stated plainly

> Reactor is never weaker than the TerminusDB node it fronts.

Read-your-writes and monotonic reads hold **per node**: a write through Reactor
is immediately visible to a later read through the same Reactor, because the
write response is held until the cache has caught up (`write_fence_ms`).

Across nodes in a replicated cluster, Reactor cannot be stronger than
replication. If a request carries `X-Reactor-Min-Version` and this node has not
replicated that commit yet, Reactor will — per `[consistency].min_version_policy`
— wait, answer `412 Precondition Failed` so a load balancer retries elsewhere,
or serve stale and label it. It will not silently pretend.

## 7. Operating it

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

## 8. Rolling it out

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
- **No eager recomputation.** Invalidation drops an entry; the next reader pays
  to refill it.
- **The shared cache tier is unproven across nodes.** The filesystem store works
  and is unit-tested; pointing two nodes at one directory is designed for and
  untested.
- **`optimize` and the history-rewriting endpoints purge the whole branch.** A
  deliberate choice, but it means a rebase costs the branch's hit rate.
