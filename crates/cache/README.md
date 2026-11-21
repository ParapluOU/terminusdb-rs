# terminusdb-cache

Application-level derived values in [Reactor](../reactor)'s dependency graph:
`get_or_compute` for the things your code computes, invalidated by the same
commits that invalidate everything else.

Reactor caches what it can prove it may cache — a document read, a GraphQL
selection, a WOQL answer against stated conditions. What it cannot cache is the
thing an application spends most of its time on: the value it *computes* from
those reads. Reactor never saw the function, so it cannot know what the result
depends on.

This crate closes that gap, and the point is that **it does not ask you to
declare the dependencies**.

```rust
derivation! {
    /// One publication's rendered overview.
    pub struct OverviewOf { pub id: String, pub limit: usize }
    -> Overview
    |this, scope| {
        // Ordinary reads, through an ordinary client.
        let publication = scope.client()
            .get_document(&this.id, scope.spec(), GetOpts::default()).await?;

        // Your transformation. Reactor cannot see inside it and does not need to.
        Ok(render(&publication, this.limit))
    }
}

let page = cache.get(&spec, OverviewOf { id, limit: 20 }).await?;
```

That is the whole API surface for the common case. The second call is a hit; a
write to `id` recomputes it; a write to anything else does not.

## How it works

Your reads go *through* Reactor, and Reactor already computes a precise read set
for every request it serves. So the dependency set of a computation can be
**observed** instead of asserted:

1. `Cache::get` asks Reactor for the branch head and **latches that commit**.
2. It looks the value up at that commit. A hit ends the story; your closure is
   never called.
3. On a miss it opens a `Scope`: a client whose every request is pinned at the
   latched commit and labelled with the scope's id. Reactor unions the read set
   of every labelled request into the scope.
4. Your closure runs.
5. The value is stored against the scope. Reactor takes the union as the
   dependency set and places the entry at the latched commit.

Step 5 goes through the same insert protocol as any other fill, which is the
part worth knowing: if a commit landed while your closure was running, the value
is neither rejected nor stale — it gets a bounded validity interval and comes
back as `Outcome::Stored { raced: true }`.

The prototype this design comes from asked you to declare conditions
(`Condition::any_changed::<User>()`), and a wrong declaration is the one way to
get stale data out of a cache that is otherwise sound. Declaring is still
available (`Dependencies::Declared`) for computations whose reads Reactor cannot
attribute — a WOQL query being the usual case — but it is no longer the default.

## What will refuse to cache

A computation is memoizable only if its inputs are known and come from one
snapshot. Reactor refuses the store, with a sentence saying which, when:

| Refusal | Usual cause |
|---|---|
| no read was recorded | the reads did not go through `scope.client()` |
| a read was not tracked | a bypass: an unknown route, WOQL with no stated condition, a body too large to cache |
| a read was not pinned | a client other than the scope's |
| several snapshots | likewise — two commits means the value is true at neither |
| dependencies could not be narrowed | a read whose read set was `everything` |

In every case `Cache::get` still returns your value: the computation succeeded,
only the caching failed. It is reported as `Outcome::NotStored(reason)`, logged
at `warn`, and visible per scope at `GET /_reactor/scope/{id}`.

**Watch for it in development.** A derivation that silently never caches behaves
exactly like one that does, except for the bill.

## Keys

```text
<name> / <params-hash> / <epoch>
  │          │             └─ which build of the code produced it
  │          └─ which arguments it was called with
  └─ which computation it is
```

The macro fills in the name (the struct's own) and the params hash (from
`Serialize`, through a canonical encoding that sorts object keys explicitly, so
a `HashMap` field does not depend on iteration order — `serde_json`'s own
ordering cannot be relied on, since `preserve_order` is enabled somewhere in
this workspace and cargo unifies features across a build). The epoch is the one thing the dependency graph cannot see,
because it is a change to the *function* rather than to the data:

- **`EpochPolicy::Deploy(build_id)`** — the default, from
  `TERMINUSDB_CACHE_EPOCH`. Every deploy starts cold, and a value produced by
  code that no longer exists can never be served.
- **`EpochPolicy::CodeHash`** — keys on the hash of each derivation's own source
  text, taken at compile time from `stringify!` of the body. Warm across
  deploys. The honest limit: it covers that body only, so if the body calls
  `summarise()` and *that* changes, the key does not move.

`Derive::key_override` replaces the whole key when you want a readable one, or a
deliberately coarser one. You own uniqueness if you do — including the epoch.

## Granularity: nesting, not magic

A change to one input invalidates the whole derived value, because Reactor
cannot re-run part of a function it cannot see. The way to get Skip-like
per-key incrementality is to structure the computation:

```rust
let cards = scope.derive_all(ids).await?;   // one derivation per document
```

Each inner value is its own cache entry with its own dependencies, so one
changed document re-maps one card. The outer value still rebuilds — it inherits
the inner values' terms, which is what keeps it from outliving them — but the
rebuild reads the other cards from cache. And a card is *shared*: the overview
and the detail page that both need it compute it once between them.
`tests/derived.rs::a_nested_derivation_recomputes_only_the_member_that_changed`
asserts exactly that, against a live node.

## Two rules worth remembering

**Push filters into the query; keep transformations in code.** A
`.filter(|p| p.active)` in Rust is invisible, so the value depends on everything
the read returned. The same predicate in the GraphQL read gets Reactor's
predicate index, and a write that does not move a document into or out of the
set fires nothing at all.

**Put in the key anything that changes what the computation means.** The user it
is scoped to, especially: authorization on a derived value is "whoever may read
this namespace may read values derived from it", which does not separate two
users' views of the same derivation.

## Reads that span databases

A derivation is **one namespace, one snapshot**. Both halves are deliberate, and
neither is a missing feature:

- A dependency term (`Doc(id)`, `DocField`, `Edge`, `Index`) carries no
  namespace. The graph is partitioned per database with a single writer each —
  which is what makes the fill/commit race exactly solvable — so there is no way
  to express "dies if `Doc(X)` in database A *or* `Doc(Y)` in database B
  changes", and no writer that could apply it.
- A scope whose reads were pinned at two commits is refused, because a value
  computed across two snapshots is true at neither. Two databases commit
  independently, so one snapshot across both does not exist.

So for a view assembled from several databases — a tenant's own data plus
globally shared content — do one of these:

**Cache the leaves and recompose per request.** Each database's reads are
ordinary cached reads as soon as that namespace is tracked
(`POST /_reactor/namespace`, or `[capture].auto_register`). The cost that hurts
is the round trips, not the assembling, so this recovers nearly all of the win
with no composition at all.

**Or store the composition keyed on the revisions it was built from:**

```rust
#[derive(Debug, Clone, Serialize)]
struct Envelope { chunks: Vec<(String, String)> }   // (chunk id, revision)

#[async_trait::async_trait]
impl Derive for Envelope {
    type Output = Rendered;
    const NAME: &'static str = "Envelope";

    async fn compute(&self, scope: &Scope) -> anyhow::Result<Rendered> {
        // Read each chunk at the revision this envelope names — from whichever
        // database holds it.
        render(&self.chunks, scope).await
    }

    fn dependencies(&self) -> Dependencies {
        // Over a known revision set the result is immutable: there is nothing
        // to invalidate, because nothing can change under it.
        Dependencies::Independent
    }
}
```

The revision set is in the parameters, so it is in the key; `Independent` says
the value does not depend on anything that can move. That is a *stronger*
statement than any dependency term could make, and an application that records
which revisions it composed already has it. The namespace the value is stored in
still has to be tracked, and its `X-Reactor-At` commit still has to resolve
there — store it in the database that publishes the composition.

## The hazard this cannot see

If your closure reads something the scope does not go through — another
database, the clock, an environment variable, an HTTP service — the value is
memoized against inputs that can change with no commit, and nothing will tell
you. SkipLabs' Skip enforces mapper purity in its compiler; this crate cannot.
Keep the closure a pure function of what it reads through the scope.

## Status

The API is complete for the lazy case and tested end to end against a live
TerminusDB 12.1 node (`crates/reactor/tests/derived.rs`): computed once then
served, invalidated by a write to an input, untouched by a write to anything
else, refusing to cache an untracked computation, nested per-member
recomputation, parameters in the key, and one computation across concurrent
callers.

Not built yet:

- **Eager maintenance.** Invalidation drops the value and the next reader
  recomputes. Pushing a recomputation at a registered callback would make
  derivations eager — the struct form exists partly so that a key can be revived
  into a computation, which is what eager needs.
- **Cross-process single-flight.** Two app instances missing the same key will
  both compute. The cost is duplicated work, never a wrong answer.
- **Child-dirtying.** A nested value's terms are inherited by its parent rather
  than tracked as an edge, so an inner recomputation that produces identical
  bytes still rebuilds the outer value. The engine already backdates fragments,
  so the precision is available; it is the propagation that is missing.
