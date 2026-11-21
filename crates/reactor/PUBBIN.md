# Using Reactor from an application with a database per room

Written against the roomnet shape: one TerminusDB database per room /
publication, plus a shared database of globally reused chunks. Everything below
is live in `main`; where something is deliberately not supported, it says so and
says what to do instead.

## 1. Where Reactor sits

It is a proxy, not a library. It runs next to one TerminusDB node, speaks the
same HTTP API, and caches what it can prove it may cache.

```
your app ──▶ Reactor :6364 ──▶ TerminusDB :6363
```

So the integration is one line: point the client at Reactor instead of the
database.

```rust
let client = TerminusDBHttpClient::new(
    Url::parse("http://localhost:6364")?,   // was :6363
    "admin", "root", "admin",
).await?;
```

Nothing else changes. An endpoint Reactor does not understand is forwarded
untouched, and a cached response is byte-identical to the origin's — if it ever
is not, that is a bug worth reporting rather than something to work around.

Reads become cacheable, writes pass through. `GET /api/document/...`, GraphQL
queries, and read-only WOQL are the surfaces it caches.

## 2. Configuration for this layout

```toml
[node]
listen = "0.0.0.0:6364"

[upstream]
url = "http://localhost:6363"
user = "admin"
pass = "root"
org  = "admin"

[capture]
# A room is created while the process runs, so it cannot be listed here.
# Reactor adopts a namespace on first read instead of forwarding it uncached.
auto_register = true
# Each tracked namespace costs a capture task, a writer and a schema index.
# Over the cap, registration is REFUSED and that database is proxied uncached —
# never evicted out from under something that may be using it.
max_tracked_namespaces = 2000
```

`[capture].driver` now defaults to `diff-log` and you should leave it there. It
reads the server's per-commit feed, so a poll with nothing to report costs one
request that opens no layer, and every commit arrives with its parent instead of
costing a log walk. If the node turns out not to carry the feed, Reactor asks
`/api/info` for its `extensions`, falls back to head polling, and logs a warning
naming what an upgrade would buy — it does not go quiet.

**`auto_register = true` is the one switch that changes a safety default.**
Without it an unlisted namespace is forwarded uncached, which is right when the
set of databases is known, because then a surprise database is a bug. With a
room per database it is a room.

If you would rather be explicit, register on the path that creates the room:

```bash
curl -X POST localhost:6364/_reactor/namespace \
     -H 'Content-Type: application/json' \
     -d '{"org":"admin","db":"room-42","branch":"main"}'
# → {"namespace":"admin/room-42/local/branch/main","added":true,"tracked":37,"cap":2000}
```

Idempotent, so calling it on every request is fine; `added:false` is a success.
`GET` the same path to list what is tracked and how long since each was read,
`DELETE` to stop. The first read after registering is still a miss: there is no
position to pin a fill at until the watermark lands.

## 3. Derived values — the part that matters for a renderer

Reactor caches reads. What an application spends its time on is what it
*computes* from those reads, and Reactor never saw your function. That is what
`terminusdb-cache` is for, and the point is that **you do not declare the
dependencies**.

```rust
use terminusdb_cache::{derivation, Cache, EpochPolicy};

derivation! {
    /// One publication's rendered overview.
    pub struct OverviewOf { pub id: String, pub limit: usize }
    -> Overview
    |this, scope| {
        // Ordinary reads through an ordinary client.
        let publication = scope.client()
            .get_document(&this.id, scope.spec(), GetOpts::default()).await?;
        // Your transformation. Reactor cannot see inside it and does not need to.
        Ok(render(&publication, this.limit))
    }
}

let cache = Cache::new("http://localhost:6364", "admin", "root", "admin")?
    .with_epoch(EpochPolicy::Deploy(build_id()));

let page = cache.get(&spec, OverviewOf { id, limit: 20 }).await?;
```

The second call is a hit. A write to that publication recomputes it; a write to
anything else does not. Reads issued through `scope.client()` are pinned at one
commit and labelled, and Reactor unions their read sets into the dependency set
— so the dependencies are **observed**, not asserted, and a wrong declaration
cannot make the value stale.

Per-member granularity comes from structure, not magic:

```rust
let cards = scope.derive_all(ids).await?;   // one derivation per chunk
```

Each inner value is its own entry with its own dependencies, so one changed
chunk re-renders one card and the rest come from cache. A card is *shared*: the
overview and the detail page that both need it compute it once between them.

### Watch for refusals in development

A derivation that silently never caches behaves exactly like one that does,
except for the bill. Use `get_reported` and look at `Outcome::NotStored(reason)`,
or `GET /_reactor/scope/{id}`. The reasons are: no read was recorded (your reads
did not go through `scope.client()`), a read was not tracked (a bypass), a read
was not pinned, several snapshots, or dependencies could not be narrowed.

## 4. Chunks shared across databases — read this before designing the view

**A derivation is one namespace, one snapshot.** Both halves are deliberate:

- A dependency term carries no namespace. The graph is partitioned per database
  with a single writer each, which is what makes the fill/commit race exactly
  solvable — so there is no way to express "dies if `Doc(X)` in room-42 *or*
  `Doc(Y)` in shared changes", and no writer that could apply it.
- A scope whose reads were pinned at two commits is refused, because a value
  computed across two snapshots is true at neither. Two databases commit
  independently, so one snapshot across both does not exist.

So for a page assembled from a room plus shared chunks, pick one:

**(a) Cache the leaves, recompose per request.** Each database's reads are
ordinary cached reads as soon as that namespace is tracked. The cost that hurts
in this layout is round trips, not assembling, so this recovers nearly all of
the win with no composition at all. Start here.

**(b) Store the composition keyed on the revisions it was built from.**

```rust
#[derive(Debug, Clone, Serialize)]
struct Envelope { chunks: Vec<(String, String)> }   // (chunk id, revision)

#[async_trait::async_trait]
impl Derive for Envelope {
    type Output = Rendered;
    const NAME: &'static str = "Envelope";

    async fn compute(&self, scope: &Scope) -> anyhow::Result<Rendered> {
        render(&self.chunks, scope).await
    }

    fn dependencies(&self) -> Dependencies {
        // Over a known revision set the result is immutable: nothing can change
        // under it, so there is nothing to invalidate.
        Dependencies::Independent
    }
}
```

The revision set is in the parameters, so it is in the key; `Independent` says
the value depends on nothing that can move — a *stronger* statement than any
dependency term, and one an application that records which revisions it composed
already has. Store it in the database that publishes the composition.

## 5. Two rules that decide your hit rate

**Push filters into the query; keep transformations in code.** A
`.filter(|p| p.active)` in Rust is invisible, so the value depends on everything
the read returned. The same predicate in the GraphQL read gets Reactor's
predicate index, and a write that does not move a document into or out of the
set fires nothing at all.

**Put in the key anything that changes what the computation means** — the user
especially. Authorization on a derived value is "whoever may read this namespace
may read values derived from it", which does not separate two users' views of
one derivation.

## 6. What to watch

| what | where |
|---|---|
| is a namespace tracked, and how stale | `GET /_reactor/namespace` |
| this node's watermark per namespace | `GET /_reactor/head` |
| why a derivation did not cache | `GET /_reactor/scope/{id}` |
| hit rate, evictions, watermark lag | `GET /_reactor/metrics` |
| why *this* request was or was not cached | `POST /_reactor/explain` |
| drop cached grants and entries now | `POST /_reactor/invalidate` |

`X-Reactor-Head` on every response is the commit the answer reflects. Send
`X-Reactor-Min-Version: <commit>` for read-after-write across nodes, and
`X-Reactor-Max-Staleness: <ms>` to bound how old an answer may be.

## 7. Known limits, so you do not design into one

- **Eager maintenance is not implemented.** Invalidation drops a value and the
  next reader recomputes. There is no push.
- **Cross-process single-flight is not implemented.** Two app instances missing
  the same key both compute it. Duplicated work, never a wrong answer.
- **Child-dirtying is missing.** An inner recomputation that produces identical
  bytes still rebuilds the outer value.
- **The hazard Reactor cannot see:** if your closure reads something that did not
  go through `scope.client()` — another database, the clock, an env var, an HTTP
  service — the value is memoized against inputs that can change with no commit,
  and nothing will tell you. Keep the closure a pure function of what it reads
  through the scope.
