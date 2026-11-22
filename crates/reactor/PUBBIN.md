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

## 4. Chunks shared across databases

A derivation may read **more than one database**. Ask the scope for the other
one and read it like any other:

```rust
async fn compute(&self, scope: &Scope) -> anyhow::Result<Rendered> {
    let publication = scope.client()
        .get_document(&self.id, scope.spec(), GetOpts::default()).await?;

    // The shared chunk store, latched at its own commit.
    let shared = scope.foreign("chunks").await?;
    let intro = shared.client()
        .get_document("Chunk/intro", shared.spec(), GetOpts::default()).await?;

    Ok(render(&publication, &intro))
}
```

The dependencies are still **observed**, not declared: reads of the other
database are recorded under that database, and a commit there that touches
something this computation read invalidates the value.

Three things are worth understanding about what that means.

**It is a vector, not a snapshot.** Two databases commit independently, so there
is no single commit the value is "true at". Each database is latched at its own
commit, and what Reactor records is *composed of exactly these revisions* —
checkable, and checked. A scope that reads two commits of **one** database is
still refused, because that really is incoherent.

**The value lives in one database.** It is stored in the scope's own namespace:
that is where it is looked up, and authorization is still "whoever may read this
namespace may read values derived from it". Put the user in the key if a
derivation mixes users.

**A foreign change evicts rather than closes.** A term carries no namespace — it
is filed in one namespace's index, owned by that namespace's single writer,
which is what makes the fill/commit race exactly solvable. So the cross-database
half is a registration with the *other* writer, and when it fires it deletes the
entry instead of closing a validity interval: a commit over there happened at no
position on this database's axis, so there is no honest interval to close, while
a missing entry is safe at every position. The next reader recomputes. The race
is closed the same way it is locally — the other writer checks its own fired log
and registers in one message — so a chunk that changed between your reads and
your store is reported as `raced` rather than cached stale.

What this costs, and what to do about it:

- **Fan-out is as wide as the chunk is reused.** Editing a chunk that appears in
  every publication evicts every composite that read it. Lazily — a room nobody
  opens costs nothing — but if that chunk is in the boilerplate, "pages actually
  read" is the whole working set. For very widely shared chunks prefer leaving
  them as leaf reads recomposed per request.
- **Identical bytes still rebuild the parent.** Child-dirtying is not
  implemented for derived values, so a chunk edit your renderer normalises away
  still re-renders every page including it.
- **A baseline is still cheaper.** Where the revision set is frozen — a print
  envelope that records which revision of each chunk it was made from — put the
  revisions in the parameters and declare `Dependencies::Independent`. Nothing
  can move under it, so there is nothing to invalidate and nothing to register.
  Note that `Independent` means "no term can fire this", not "detached": the
  entry still lives in a tracked namespace, is stored against a commit that must
  still be in `[consistency].commit_ring_capacity`, and goes when that namespace
  is reset.

### Read-your-writes across the composition

A computation pins itself before its first read, so fencing the reads is too
late. Demand the freshness at the latch instead:

```rust
let (_, commit) = client.insert_instance_with_commit_id(&chunk, args).await?;
let page = cache.get_after(&spec, PageOf { .. }, &commit).await?;
```

Reactor waits for its watermark to reach that commit and errors rather than
hand back an older pin. Each demand is about one history, so it composes across
databases without any cross-database machinery.

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
| one namespace's watermark, no listing | `GET /_reactor/head?namespace=admin/room-42/local/branch/main` |
| the diff log was demoted to polling | `reactor_difflog_demotions` in `/_reactor/metrics` |
| how big the cross-database pivot is | `reactor_foreign_registrations` |
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
- **Foreign invalidation is eviction, not closure.** A value composed from
  another database is deleted when that database changes, so the next reader
  recomputes from scratch — there is no partial reuse, and no validity interval
  to serve an older reader from.
- **The hazard Reactor cannot see:** if your closure reads something that did not
  go through `scope.client()` or `scope.foreign()` — the clock, an env var, an
  HTTP service, a database read through a client you built yourself — the value
  is memoized against inputs that can change with no commit, and nothing will
  tell you. Keep the closure a pure function of what it reads through the
  scope.
