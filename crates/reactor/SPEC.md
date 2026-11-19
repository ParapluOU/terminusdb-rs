# Reactor — implementation spec

Design rationale: `docs/reactive-cache-design.md` (read §5–§6), plus the
revisions in §11 of that doc, which this spec implements. This file is the
*contract*: module layout, the exact public type vocabulary, and the invariants.
Types marked **VERBATIM** must be used exactly as written so phases compose.

## Non-negotiable invariants

1. **Byte-faithful.** Cached and proxied bodies are `bytes::Bytes`, never
   round-tripped through `serde_json::Value`. v12 returns full-precision numbers
   and the workspace enables `serde_json/arbitrary_precision`; re-serializing
   would corrupt them and reorder keys. Parse to *inspect*, never to re-emit.
2. **Fills are pinned.** Every upstream fill is issued against
   `/api/.../local/commit/<id>` at the position the reader latched, so a
   fragment is exactly "the value at that commit". Never fill from an unpinned
   "latest" read.
3. **Commit ids carry no order.** They are content hashes. All ordering is by
   Reactor-local `Position = (Epoch, Seq)`. Never compare `CommitId`s with `<`.
4. **The watermark only advances past a commit we have actually read.** It is
   published after a pinned read at that commit succeeds, so capture lag costs
   staleness, never correctness — which is what makes a shared-disk reader node
   safe even if its view of the filesystem is weird.
5. **Soundness over precision.** An analyzer that cannot *prove* a tight term
   emits a coarser one, or `Plan::Bypass`. Over-invalidation is a performance
   bug; under-invalidation is a correctness bug. When in doubt, bypass.
6. **Unknown routes are forwarded, never cached.** The default must never invert.
7. **Never memoize an error.** Failed fills are removed from the single-flight
   map and never stored.
8. **No new IRIs by hand.** Use `DocId::parse` / `EntityIDFor` / `TdbIRI`; never
   `format!("{class}/{key}")`. (Workspace rule.)
9. **No panics on the request path.** `unwrap`/`expect` only in tests and in
   startup/config code.
10. **Reactor is additive.** Do not edit any other crate. The only edit outside
    `crates/reactor/` is the workspace member line, already made.

## Module layout (`crates/reactor/src/`)

```
lib.rs          re-exports; crate docs
main.rs         binary: clap -> config -> serve
config.rs       TOML config + CLI overrides
ids.rs          Namespace, DocId, ClassName, PropName, ContentHash, ShapeId,
                Seq, Epoch, Position, Watermark                          [A]
term.rs         Term, TermBucket, ValueKey, TermSet                      [A]
schema_index.rs SchemaIndex: props (value vs link), hierarchy, backlinks  [A]
change.rs       DocChange, ChangeSet, fire()                             [A]
compose.rs      russian-doll decomposition of document lists: split a list
                into per-document byte ranges + learned framing, and splice
                it back; Spine, Framing, FramingCache                     [B]
engine/
  mod.rs        Engine facade + WriterMsg mailbox protocol               [B]
  fragment.rs   FragmentKey, FragmentVersion, FragmentSlot, BodyRef      [B]
  index.rs      TermIndex (TermBucket -> entries), subsumption matching  [B]
  log.rs        FiredLog: ring of (Position, CommitId, TermSet)          [B]
  writer.rs     single writer task per namespace: apply/insert/nff       [B]
  policy.rs     Policy: eager|lazy, ttl, fence policy, decomposition     [B]
store/
  mod.rs        FragmentStore trait + BodyRef resolution                 [B]
  memory.rs     bounded in-memory tier (weight-based eviction)           [B]
  fscas.rs      filesystem content-addressed tier (also the NAS shared tier) [B]
analyze/
  mod.rs        Analyzer, Plan, ReqView, route classification            [C]
  document.rs   /api/document                                            [C]
  graphql.rs    normalize, inject _id/_type, extract terms               [C]
  woql.rs       woql2 AST read-set extraction + write detection          [C]
capture/
  mod.rs        CommitSource / ChangeResolver / ReadinessGate traits,
                Applier (owns correctness), Supervisor                   [D]
  headpoll.rs   head-poll driver (the only one that works on a shared-disk
                reader node; primary driver there)                       [D]
  subscription.rs  _ChangeSet SSE driver                                 [D]
  resolve.rs    ChangeResolver: /api/diff + /api/changes -> ChangeSet     [D]
frontend/
  mod.rs        axum router + AppState                                   [D]
  proxy.rs      byte-faithful streaming passthrough                      [D]
  serve.rs      lookup/fill/compose, header protocol, fencing            [D]
  fill.rs       single-flight + detached fills + sibling batching         [D]
  admin.rs      /_reactor/* : head, explain, fragment, invalidate, metrics [D]
metrics.rs      hand-rolled counters + Prometheus text encoding (no new dep)
```

## Phase A vocabulary — **VERBATIM**

```rust
// ids.rs

/// A cache universe: one database branch on one node.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Namespace { pub org: String, pub db: String, pub branch: String }

impl Namespace {
    pub fn new(org: impl Into<String>, db: impl Into<String>, branch: impl Into<String>) -> Self;
    /// `admin/mydb/local/branch/main` — the upstream path segment.
    pub fn path(&self) -> String;
    pub fn to_branch_spec(&self) -> terminusdb_client::BranchSpec;
    pub fn to_commit_spec(&self, commit: &CommitId) -> terminusdb_client::BranchSpec;
}

/// Reactor-local monotonic position inside an epoch. Commit ids are content
/// hashes with no order; THIS is the revision counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Seq(pub u64);

/// Bumped on non-fast-forward head movement (rebase/reset/squash/apply, or a
/// pull that lands a head which is not a descendant of our watermark).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Epoch(pub u64);

/// Totally ordered. Lexicographic on (epoch, seq) — derive Ord with epoch first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Position { pub epoch: Epoch, pub seq: Seq }

impl Position {
    pub const ZERO: Position;
    pub fn next(self) -> Position;            // seq + 1, same epoch
    pub fn next_epoch(self) -> Position;      // epoch + 1, seq 0
}

/// What a reader latches at request start, and what we report to clients.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Watermark { pub pos: Position, pub commit: CommitId }

/// A document id in short form, e.g. `Person/alice`. `Arc<str>` because these
/// are cloned constantly into terms and keys.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocId(Arc<str>);

impl DocId {
    /// Accepts short id (`Person/alice`) or full IRI
    /// (`terminusdb:///data/Person/alice`); normalizes to short form.
    pub fn parse(s: &str) -> Self;
    pub fn as_str(&self) -> &str;
    /// Everything before the first `/`; `None` if there is no class prefix.
    pub fn class(&self) -> Option<ClassName>;
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClassName(Arc<str>);
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PropName(Arc<str>);
// Both: ::new(impl Into<Arc<str>>), ::as_str(), Display, serde as a string.

/// blake3-256. Content addresses and cache keys.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    pub fn of(bytes: &[u8]) -> Self;
    /// Order-independent combination, for composing child hashes into a parent.
    pub fn combine(parts: &mut [ContentHash]) -> Self;
    pub fn to_hex(&self) -> String;
    pub fn from_hex(s: &str) -> Option<Self>;
}
// Debug + Display print the first 12 hex chars; to_hex() prints all 64.

/// "Which projection of a thing" a fragment holds: hash of a normalized
/// request/selection shape with ids and variables factored out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ShapeId(pub ContentHash);
```

```rust
// term.rs

/// Normalized scalar for value-indexed terms: canonical JSON text, so `1`,
/// `1.0` and `"1"` stay distinguishable.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ValueKey(String);
impl ValueKey { pub fn of(v: &serde_json::Value) -> Self; pub fn as_str(&self) -> &str; }

/// A dependency: something a cached value was derived from. A commit "fires" a
/// set of terms; a fragment is dirtied when any dependency it holds subsumes a
/// fired term. Ordered precise -> coarse. Design doc §5.2.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Term {
    DocField(DocId, PropName),
    Doc(DocId),
    Edge(DocId, PropName),
    Backlink { target: DocId, prop: PropName, from_class: ClassName },
    Index { class: ClassName, prop: PropName, value: ValueKey },
    ClassField(ClassName, PropName),
    Members(ClassName),
    Class(ClassName),
    Path { from: Option<DocId>, classes: Vec<ClassName> },
    Schema,
    /// Namespace-wide barrier: any commit fires it. Used for non-fast-forward
    /// movement, incomplete changesets, and analyzers that give up but still
    /// want an entry.
    Everything,
}

impl Term {
    /// Index buckets this term must be filed under. Two terms whose bucket sets
    /// are disjoint can never match, so firing is a hash lookup, not a scan.
    /// A `Path` term yields one `Class` bucket per listed class; everything
    /// else yields exactly one bucket.
    pub fn buckets(&self) -> Vec<TermBucket>;
    /// Buckets a *fired* term should be looked up under, i.e. the buckets of
    /// every dependency that could possibly subsume it: its own `Doc(id)` plus
    /// `Class(c)` for each ancestor of its class, plus `Everything`.
    pub fn lookup_buckets(&self, schema: &SchemaIndex) -> Vec<TermBucket>;
    /// True if `self` (a fragment's dependency) is dirtied by `fired`.
    /// Matching is *subsumption*, not equality.
    pub fn subsumes(&self, fired: &Term, schema: &SchemaIndex) -> bool;
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TermBucket { Doc(DocId), Class(ClassName), Schema, Everything }

pub type TermSet = std::collections::BTreeSet<Term>;
```

`subsumes` truth table — write every row as a test:

| fragment dep | fired | dirty? |
|---|---|---|
| `Doc(a)` | `Doc(a)` \| `DocField(a,p)` \| `Edge(a,p)` | yes |
| `Doc(a)` | `Doc(b)` | no |
| `DocField(a,p)` | `DocField(a,p)` \| `Doc(a)` | yes |
| `DocField(a,p)` | `DocField(a,q)` | **no** |
| `Edge(a,p)` | `DocField(a,p)` \| `Edge(a,p)` \| `Doc(a)` | yes |
| `Class(C)` | any `Doc`/`DocField`/`Edge`/`Members`/`Index`/`ClassField` whose class is `C` or a descendant of `C` | yes |
| `Class(C)` | anything in an unrelated class | no |
| `Members(C)` | `Members(C)` \| `Members(D)` where `D` descends from `C` | yes |
| `Members(C)` | `DocField(C/x,p)` | **no** |
| `ClassField(C,p)` | `DocField(C/x,p)` \| `ClassField(C,p)` \| `ClassField(D,p)` for descendant `D` | yes |
| `ClassField(C,p)` | `DocField(C/x,q)` | **no** |
| `Index{C,p,v}` | `Index{C,p,v}` (also for descendant class) | yes |
| `Index{C,p,v}` | `Index{C,p,w}` | **no** |
| `Index{C,p,v}` | `Doc(C/x)` (coarse fire, value unknown) | yes |
| `Backlink{t,p,F}` | `Backlink{t,p,F}` or same with `F` a descendant | yes |
| `Backlink{t,p,F}` | `Backlink{u,p,F}`, `u != t` | **no** |
| `Path{_,classes}` | anything whose class is in, or descends from, a listed class | yes |
| `Schema` | `Schema` | yes |
| `Schema` | `Doc(a)` | no |
| `Everything` | anything, including `Schema` | yes |
| anything | `Everything` | yes |

```rust
// schema_index.rs

/// What the engine needs from the schema: which properties are values vs links
/// (the VALUE-vs-LINK distinction in CLAUDE.md — the difference between a
/// DocField term and an Edge term, and getting it wrong silently breaks
/// traversal caching), and the class hierarchy for subclass matching.
#[derive(Clone, Debug, Default)]
pub struct SchemaIndex { /* private */ }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropKind {
    /// Datatype property: a stored scalar. `range` is an xsd type name.
    Value { range: String },
    /// Object property: a graph edge.
    Link { range: ClassName, subdocument: bool },
}

impl SchemaIndex {
    /// From `client.get_class_frame(spec, None, opts)` output: a JSON object of
    /// class name -> frame. Unknown shapes are skipped, not fatal.
    pub fn from_class_frames(frames: &serde_json::Value) -> anyhow::Result<Self>;
    /// Empty index: every class unknown. `prop()` returns None, `ancestors`
    /// returns just the class. Analyzers MUST degrade to coarse terms when the
    /// index is empty rather than guess.
    pub fn is_empty(&self) -> bool;
    pub fn classes(&self) -> impl Iterator<Item = &ClassName>;
    pub fn prop(&self, class: &ClassName, prop: &PropName) -> Option<&PropKind>;
    pub fn props(&self, class: &ClassName) -> impl Iterator<Item = (&PropName, &PropKind)>;
    /// `class` plus all transitive superclasses (inclusive).
    pub fn ancestors(&self, class: &ClassName) -> Vec<ClassName>;
    /// `class` plus all transitive subclasses (inclusive).
    pub fn descendants(&self, class: &ClassName) -> Vec<ClassName>;
    pub fn is_subclass_of(&self, sub: &ClassName, sup: &ClassName) -> bool;
    pub fn is_subdocument(&self, class: &ClassName) -> bool;
    /// Every (class, prop) whose range is `class` or an ancestor of it: the
    /// reverse edges. Needed to map a GraphQL backlink field to a Backlink term.
    pub fn backlinks_to(&self, class: &ClassName) -> Vec<(ClassName, PropName)>;
    /// The generated GraphQL backlink field name: `_{prop}_of_{class}`.
    pub fn backlink_field(prop: &PropName, class: &ClassName) -> String;
    /// Parse a generated backlink field name back into (prop, class).
    pub fn parse_backlink_field(field: &str) -> Option<(PropName, ClassName)>;
}
```

```rust
// change.rs

/// One document's transition in one commit. Exactly one side may be None
/// (create / delete). Values are the document JSON as TerminusDB reports it.
#[derive(Clone, Debug)]
pub struct DocChange {
    pub id: DocId,
    pub class: ClassName,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
}

impl DocChange {
    pub fn is_create(&self) -> bool;
    pub fn is_delete(&self) -> bool;
    /// Properties whose value differs between before and after, ignoring
    /// `@`-keywords except that a changed `@type` counts. For a create/delete
    /// this is every property present on the side that exists.
    pub fn changed_props(&self) -> Vec<PropName>;
    /// before/after values of one property, each normalized to a list of
    /// scalars (so a Set/List property yields its elements).
    pub fn prop_values(&self, prop: &PropName) -> (Vec<serde_json::Value>, Vec<serde_json::Value>);
}

/// Everything one commit did, as far as we could determine.
#[derive(Clone, Debug)]
pub struct ChangeSet {
    pub ns: Namespace,
    pub commit: CommitId,
    pub parent: Option<CommitId>,
    pub changes: Vec<DocChange>,
    pub schema_changed: bool,
    /// False when we could not enumerate changes precisely: an unreadable diff,
    /// a response truncated by `count`, a commit too large for the configured
    /// diff threshold, or a non-fast-forward move. An incomplete ChangeSet MUST
    /// fire `Term::Everything`.
    pub complete: bool,
}

/// Derive the terms a commit fires. Design doc §5.3. The before-state is what
/// makes Index/Backlink firing exact; that is why we pay for /api/diff.
pub fn fire(cs: &ChangeSet, schema: &SchemaIndex) -> TermSet;
```

`fire` rules — implement and test exactly:

- `!cs.complete` → `{Everything}`, return immediately.
- `cs.schema_changed` → insert `Schema`.
- per `DocChange`:
  - `Class(c)` for every `c` in `schema.ancestors(class)`.
  - `Doc(id)` **only** for a create, a delete, or an update whose
    `changed_props()` is empty. A bare `Doc(id)` means "this document moved and
    we cannot say which property", which `ClassField`/`Edge` dependencies are
    obliged to treat as a hit — so firing it *beside* the precise `DocField`
    terms discards every traversal and every id list on the class whenever any
    field of any instance changes. It is redundant there in any case: `Doc`,
    `Class`, `Index` and `Path` dependencies all match a fired `DocField`
    directly. Sound because `capture::resolve` builds an update from the whole
    before and after documents, and every path that cannot sets
    `complete = false` (which fires `Everything`).
  - create or delete → `Members(c)` for every ancestor `c`.
  - for each `p` in `changed_props()`:
    - `DocField(id, p)` and `ClassField(c, p)` for every ancestor `c` that
      declares `p` (or, if the schema index is empty, just `class`).
    - `PropKind::Value` → for each scalar in the symmetric difference of the
      before/after value lists, `Index { class, p, value }`. Also fire the same
      for ancestor classes that declare `p`.
    - `PropKind::Link` → `Edge(id, p)`, plus for every target id on **either**
      side, `Backlink { target, prop: p, from_class }` for `from_class` in
      `schema.ancestors(class)` (old targets lose a backlink, new ones gain one).
    - unknown prop kind (empty/incomplete schema index) → fire BOTH the `Index`
      form for scalars and the `Edge`/`Backlink` form for anything that parses
      as a doc id. Over-firing is correct; under-firing is not.

## Phase B — engine and store

```rust
// engine/fragment.rs

/// What a cached entry is keyed by.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FragmentKey { pub ns: Namespace, pub root: RootKey, pub shape: ShapeId }

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RootKey {
    /// A single document's projection.
    Doc(DocId),
    /// A whole-response entry: hash of (method, path, canonical query/body).
    Request(ContentHash),
}

/// Where a body lives. Bodies are immutable and content-addressed, so a version
/// can reference one without owning it.
#[derive(Clone, Debug)]
pub enum BodyRef { Inline(Bytes), Cas(ContentHash) }

/// Response metadata we must replay verbatim alongside the body.
#[derive(Clone, Debug)]
pub struct ResponseMeta {
    pub status: u16,
    pub content_type: Option<String>,
    /// The upstream `TerminusDB-Data-Version`, if it sent one.
    pub data_version: Option<String>,
}

/// One version of a fragment, valid for positions in `[from, to)`.
#[derive(Clone, Debug)]
pub struct FragmentVersion {
    pub from: Position,
    /// None = still current.
    pub to: Option<Position>,
    /// The commit this was filled at.
    pub commit: CommitId,
    pub hash: ContentHash,
    pub body: BodyRef,
    pub meta: ResponseMeta,
    pub deps: Vec<Term>,
    pub children: Vec<FragmentKey>,
    /// Backdating: the position at which this content last actually changed.
    /// Used for ETags and to stop propagation. <= from.
    pub changed_at: Position,
}

impl FragmentVersion { pub fn covers(&self, at: Position) -> bool; }

/// Newest-first version list for one key. Keep it short: GC versions no live
/// reader needs (below the minimum latched position).
#[derive(Debug, Default)]
pub struct FragmentSlot {
    pub versions: Vec<FragmentVersion>,
    /// Logical time of the last lookup, written by *readers* through a shared
    /// reference (one relaxed store on the hot path).
    last_used: AtomicU64,
}
```

**The bounded key index.** `EngineConfig::max_tracked_keys` caps how many
fragment keys a namespace tracks, and over the cap the writer drops the
least-recently-used ones. Three properties make that safe, and all three are
load-bearing:

- The slot and its term-index registration are dropped **together, in the
  writer**, so nothing can observe one without the other and the index can never
  hold a term pointing at a slot that no longer exists.
- Dropping derived state is never a correctness event: an evicted key is a miss,
  and a miss costs a fill. Noria's partial state.
- The **body stays in the content-addressed store**. Bodies are immutable and
  keyed by hash, so keeping one costs only disk, and a refill that produces the
  same bytes finds them already there.

Eviction scans and sorts by recency rather than maintaining an intrusive LRU
list, for the same reason `store::MemoryStore` does: the hot path is the
reader's stamp, and eviction is rare (one batch per overflow, down to below the
cap). Re-adopting an evicted key *from the CAS without an upstream request* is
**not** implemented: it would need the key's dependency list and validity
interval to survive eviction, i.e. the durable per-key metadata log of R10,
which is deferred.

```rust
// engine/mod.rs

pub enum Lookup { Hit(Arc<FragmentVersion>), Miss }

/// Readers touch only `watermark()` and `lookup()`; both are lock-light and
/// never block on the writer. All mutation goes through the writer mailbox.
pub struct Engine { /* ... */ }

impl Engine {
    pub fn new(cfg: EngineConfig, store: Arc<dyn FragmentStore>) -> Self;
    /// Current published watermark for a namespace. None = not yet established;
    /// callers must then bypass the cache (we cannot pin a fill).
    pub fn watermark(&self, ns: &Namespace) -> Option<Watermark>;
    pub fn lookup(&self, key: &FragmentKey, at: Position) -> Lookup;
    /// Send a message to the namespace's writer. Returns an error only if the
    /// writer is gone.
    pub fn send(&self, ns: &Namespace, msg: WriterMsg) -> anyhow::Result<()>;
    pub fn schema(&self, ns: &Namespace) -> Arc<SchemaIndex>;
    pub fn metrics(&self) -> &Metrics;
}

/// The writer owns all mutable state for a namespace, so commits and fill
/// completions are totally ordered against each other. This is what makes the
/// fill/apply race (a fill pinned at C landing after C+1 was applied) exactly
/// solvable rather than merely unlikely.
pub enum WriterMsg {
    /// A commit was captured, resolved, and verified readable. Advances the
    /// watermark.
    Commit(Box<ChangeSet>),
    /// Head moved somewhere that is not a descendant of our watermark. Bumps
    /// the epoch. `fired = None` means "we could not diff it": drop everything.
    NonFastForward { old_head: Option<CommitId>, new_head: CommitId, fired: Option<TermSet> },
    /// A fill completed. See the insert protocol below.
    Insert(Box<FillResult>),
    /// The schema index was re-derived.
    Schema(Arc<SchemaIndex>),
    /// Drop everything for this namespace.
    ///
    /// `forget_watermark` is the difference between "the cache is wrong" and
    /// "history moved under us". A rewriting topology call (squash/reset/
    /// rebase/apply/pull/optimize/migration) must pass `true`: emptying the
    /// cache is not enough on its own, because the watermark still names a
    /// commit that may no longer be on the branch, and the very next refill
    /// would pin there and serve pre-rewrite bytes. Publishing no watermark
    /// makes readers bypass to the origin until capture establishes a new one —
    /// a brief hit-rate loss instead of a stale answer. Operator invalidation
    /// passes `false`: history is intact, so the watermark is still true.
    Purge { forget_watermark: bool },
}

pub struct FillResult {
    pub key: FragmentKey,
    /// The position the fill was pinned at.
    pub at: Position,
    pub commit: CommitId,
    pub body: Bytes,
    pub meta: ResponseMeta,
    pub deps: Vec<Term>,
    pub children: Vec<FragmentKey>,
    /// Where to send the stored version (or the raw body if unstorable).
    pub reply: Option<tokio::sync::oneshot::Sender<Arc<FragmentVersion>>>,
}
```

**Insert protocol (writer side) — implement exactly:**

```
on Insert{key, at: P, deps, body, ...}:
  if P.epoch != current epoch         -> hand body back, do not store
  if P < fired_log.oldest_retained()  -> hand body back, do not store
  let hash = ContentHash::of(&body)
  // Did anything between the pin position and now invalidate these deps?
  let K = fired_log.first_position_after(P).find(|(pos, fired)| fired.any(|f| deps.any(|d| d.subsumes(f, schema))))
  match K {
    None      => store version [P, None); register deps in the term index
    Some(kpos)=> store version [P, kpos); DO NOT register deps
                 // still useful: readers latched below kpos can serve from it
  }
  // backdating: if a previous version of this key had the same hash, inherit
  // its changed_at; else changed_at = P
```

**Apply protocol (writer side):**

```
on Commit(cs):
  pos   = head.next()
  fired = fire(&cs, &schema)
  fired_log.push(pos, cs.commit, fired)
  for entry in term_index.candidates(&fired, &schema):
      if entry.deps.any(|d| fired.any(|f| d.subsumes(f, &schema))):
          slot.current_mut().to = Some(pos)
          term_index.unregister(entry)
  if cs.schema_changed { request a schema refresh }
  head = pos
  publish Watermark { pos, commit: cs.commit }    // LAST, atomically
```

`NonFastForward` → `pos = head.next_epoch()`; with `fired = Some(terms)` treat
as a commit carrying those terms; with `None`, clear the whole namespace's slots
and index (the CAS bodies stay — they were never wrong, and can be re-adopted
by hash).

```rust
// store/mod.rs
pub trait FragmentStore: Send + Sync {
    fn get(&self, hash: &ContentHash) -> Option<Bytes>;
    fn put(&self, hash: &ContentHash, body: &Bytes) -> anyhow::Result<()>;
    fn contains(&self, hash: &ContentHash) -> bool;
    /// Approximate bytes held, for metrics and eviction.
    fn size(&self) -> u64;
}
```
- `memory.rs`: `DashMap<ContentHash, Bytes>` + weight accounting + eviction of
  the least-recently-read entries when over budget.
- `fscas.rs`: `<root>/<first2hex>/<hash>` files, write-to-temp-then-rename.
  **This doubles as the shared/NAS tier** (design doc §5.7 mode 2): point two
  nodes at the same directory and immutable fragments are shared with no
  coherence protocol. Reads must tolerate a concurrently-written file.
- `Tiered { hot: Memory, warm: Option<FsCas> }` composes them.

**Decomposition (`children`), as built.** A `/api/document` read selecting
documents by `id`, `ids` or `type` is cached as a **parent holding only the
ordered membership and the framing**, over **one child fragment per document**.
`compose.rs` owns the splitting and splicing; `Decomposition::NdjsonList`
(config `[cache].decompose`, default on) enables it, and a response that is not
a clean sequence of `@id`-bearing objects falls back to a whole-response entry
per *response*, not per route.

Three rules decide whether it is sound, and each has a named test:

1. **Projection versus framing.** A child's `ShapeId` covers only the parameters
   that change how one document renders (`unfold`, `minimized`, `compress_ids`,
   `identifiers`, `raw_id`, `xsd`, `format`, `prefixed`, `raw_json`,
   `graph_type`). Framing — NDJSON versus `as_list`, plus `skip`/`count` and the
   origin's order — belongs to the parent. That is what lets a by-id read and a
   `?type=` list share one child, so each warms the other.
2. **The parent depends on membership only** (`Members(C)`, or the members of
   the named ids' classes), and **never** on `Term::Class`, which fires on every
   field change and would make the parent refetch exactly the change
   decomposition exists to absorb.
3. **Framing is learned, not assumed.** `compose.rs` records the verbatim bytes
   before, between and after the documents it found and refuses a body whose
   separators are not uniform. Nothing in the code knows what `[` means, so
   `minimized=false`, `as_list=true` and `unfold=true` need no special cases.

On a read: resolve the parent, look up each child, and fill **all** missing
children in ONE pinned `?ids=[…]` request. A composition whose children cannot
all be resolved falls back to a plain upstream fetch — never a partial body.

GraphQL body splicing stays out of scope (design doc R4): a GraphQL response is
a nested selection rather than a sequence of documents, so there is no framing
to learn and composition would mean re-emitting a body, which invariant 1
forbids.

## Phase C — analyze

```rust
pub struct ReqView<'a> {
    pub method: &'a http::Method,
    pub path: &'a str,
    pub query: &'a str,
    pub headers: &'a http::HeaderMap,
    pub body: &'a Bytes,
    /// Parsed from the path: which namespace, and whether it was commit-pinned.
    pub target: Target,
}

pub enum Target {
    /// `/api/<ep>/<org>/<db>/local/branch/<b>` or the bare `/<org>/<db>` form.
    Branch { endpoint: String, ns: Namespace },
    /// `/api/<ep>/<org>/<db>/local/commit/<id>` — immutable.
    Pinned { endpoint: String, ns: Namespace, commit: CommitId },
    /// Anything else (system endpoints, /api/db, /api/info, ...).
    Other { endpoint: String },
}

pub enum Plan {
    /// Forward, cache nothing. Always safe.
    Bypass { reason: &'static str },
    /// A mutation: forward, then tell capture to expect a new commit.
    Write,
    /// Cacheable with these dependencies.
    Read { key: FragmentKey, terms: Vec<Term>, immutable: bool },
}

pub fn plan(req: &ReqView, schema: &SchemaIndex, cfg: &AnalyzeConfig) -> Plan;
```

Rules:
- `Target::Pinned` + a read method/endpoint → `Read { immutable: true, terms: [] }`.
  Highest-value, zero-risk case: do it first.
- `/api/document` GET: `id=` → `Doc`; `type=` → `Members(C)` + `Class(C)`;
  `type=` with `counted=true` → `Members(C)` **only** (the body is a count, not
  documents, so it cannot depend on any field value); `graph_type=schema` →
  `Schema`. Every `GetOpts`-ish query param
  (`unfold`, `as_list`, `compress_ids`, `minimized`, `type`, `skip`, `count`,
  `id`, `ids`, `prefixed`, `xsd`, `format`, `identifiers`, `id_prefix`,
  `raw_id`) belongs in the shape hash. Unrecognized params → `Bypass`.
- `/api/document` POST/PUT/DELETE → `Write`.
- `/api/graphql` POST: parse with `graphql-parser`. Mutation → `Write`.
  Subscription → `Bypass`. Query → normalize + inject + extract (below).
- `/api/woql` POST: parse the JSON-LD body into `woql2::Query`
  (`serde_json::from_value`), walk it. Any `Add*`/`Delete*`/`InsertDocument`/
  `UpdateDocument`/`DeleteDocument` anywhere → `Write`. Else extract read-set.
  Parse failure → `Bypass`.
- Everything else → `Bypass`, with the route-specific exception that
  `/api/db`, `/api/branch`, `/api/squash`, `/api/reset`, `/api/rebase`,
  `/api/apply`, `/api/pull`, `/api/fetch`, `/api/optimize`, `/api/migration`
  additionally emit a "resync this namespace" signal after a 2xx.

GraphQL handling, in order:
1. Parse to AST. On failure → `Bypass`.
2. **Normalize** for the shape hash: strip comments, sort fields and arguments
   by name, inline fragment definitions, replace variable *values* with
   placeholders (variables go in the key separately), drop aliases from the
   shape but keep them in the executed query.
3. **Inject** `_id` and `_type` into every object selection set that lacks them
   (any selection set that itself has sub-selections, plus every root field),
   recording which were injected so the response can be stripped before it goes
   to the client. Keep them in the stored fragment.
4. **Extract terms** against `SchemaIndex`:
   - root field `C(filter: {p: {eq: v}})` → `Index{C,p,v}`; any other filter
     shape → `Class(C)` + `Members(C)`; no filter → `Members(C)` + `Class(C)`.
     `Class(C)` is there because the members' **field values are in the
     response**; it must not appear in the read set of an answer that carries no
     field value (see `_count` and the id-only rule below).
   - nested link field `p` on class `C` → `ClassField(C,p)` and, per returned
     parent id, `Edge(id,p)` (added post-hoc from the response ids — see below).
   - backlink field `_p_of_F` on class `C` → `Backlink{target: <each id>, p, F}`
     and `ClassField(F,p)`.
   - `_path_to_C` → `Path { from: None, classes: <C and descendants> }`.
   - `_count(C: <filter>)` → *membership only*: no filter (or `{}`) →
     `Members(C)`; `{p: {eq: v}}` → `Index{C,p,v}`; any other filter shape →
     `Members(C)` + `Class(C)`, since the bucket cannot be named. A count is a
     statement about how many documents exist, so it cannot depend on what any of
     them contains — `Class(C)` here made paging metadata die on every write.
     Same for a nested `_count`.
   - unknown field or unknown class → `Class(*)` is not available, so
     `Term::Everything`, or `Bypass` if `cfg.strict`.
5. **Post-fill augmentation.** After the fill, walk the response for `_id`
   values and add `Doc(id)` for each. This is what makes the cache entry
   entity-precise and is why we inject ids. Signature:
   `pub fn augment_terms_from_response(body: &[u8], terms: &mut Vec<Term>)`.
6. **The id-only exception.** When every selection set contains only
   `_id`/`_type` and nested link traversals — no scalar field anywhere — the
   response contains no field value, so it depends on membership and on where
   the edges point and on nothing else. Such a query:
   - takes `Members`/`Index` for its roots (never `Class`), `ClassField(C,p)`
     for each traversed link property, and `Members(range)` for each traversal's
     target class (an edge target that is deleted, or recreated under a new
     identity, changes the answer without the parent's property moving);
   - **skips step 5 entirely** (`Augment::None`): a `Doc` term per returned id
     would reintroduce precisely the field dependence just removed;
   - **skips injection** (step 3): injection exists to make a value-bearing
     response mappable to documents, so an id-only query is forwarded verbatim
     and nothing has to be stripped on the way back.

   The traversal term is the static `ClassField(C,p)` rather than a
   response-derived `Edge(parent_id,p)` because an id-only selection need not
   select `_id` at every level, so a walk of the response cannot always name the
   parent — and a traversal term we failed to emit serves stale ids. The
   detection is a **proof**: an unknown field, an inline fragment on an unknown
   type, a `_path_to_`, a backlink, or an object selection with no fields all
   answer "not id-only" and keep the value-bearing behaviour. This is what lets
   an ORM's phase-one id list survive field updates while the phase-two
   `get_documents` fragments invalidate one at a time.

WOQL read-set extraction, by node:
- `Triple{subject, predicate, object}` / `Link` / `Data`: subject bound to a
  node → `Doc(id)`; subject bound + predicate bound → `Edge`/`DocField`
  depending on `PropKind`; predicate bound only → `ClassField(*, p)` is not
  expressible, so `Everything` unless the type is also constrained.
- `IsA`/`TypeOf`/`Subsumption` with a bound class → `Members(C)` + `Class(C)`.
- `ReadDocument{identifier}` bound → `Doc(id)`.
- `Path` → `Path{from, classes: []}` (conservative: treat as `Everything` when
  no class is derivable).
- `Using`/`From`/`Into` with a commit → `immutable: true`.
- Control nodes (`And`/`Or`/`Select`/`Limit`/…) recurse.
- Anything unrecognized → `Everything`.

## Phase D — capture, frontend, config

```rust
// capture/mod.rs

/// Drivers only *notice* commits. They are allowed to be wrong: at-least-once,
/// out of order, or ahead of the local node's disk.
#[async_trait::async_trait]  // or: fn subscribe(..) -> BoxStream<'static, ...>
pub trait CommitSource: Send + Sync {
    fn name(&self) -> &'static str;
    fn subscribe(&self, ns: &Namespace, after: Option<CommitId>)
        -> futures_util::stream::BoxStream<'static, anyhow::Result<CommitEvent>>;
}

pub struct CommitEvent {
    pub commit: CommitId,
    pub parent: Option<CommitId>,
    /// Some drivers (the SSE `_ChangeSet`) already carry the changes.
    pub changes: Option<Box<ChangeSet>>,
}

/// Turns a noticed commit into a ChangeSet, by local pinned reads.
#[async_trait::async_trait]
pub trait ChangeResolver: Send + Sync {
    async fn resolve(&self, ns: &Namespace, commit: &CommitId, parent: Option<&CommitId>)
        -> anyhow::Result<ChangeSet>;
}

/// The canary: has this node actually got this commit, locally and readably?
/// The watermark never advances past a commit this did not confirm.
#[async_trait::async_trait]
pub trait ReadinessGate: Send + Sync {
    async fn await_readable(&self, ns: &Namespace, commit: &CommitId) -> anyhow::Result<()>;
}

/// Owns correctness. Drivers are untrusted inputs.
pub struct Applier { /* ... */ }
```

Applier logic, by `(event.parent, watermark)`:
- commit already applied → drop (duplicate delivery is expected).
- `parent == Some(watermark.commit)` → gate, resolve, `WriterMsg::Commit`.
- `parent` unknown or `None` → **gap**: walk `/api/log` back from the new head
  to the watermark, apply each commit in order. This log-walk reconciler is
  shared by every driver and is the only thing that has to be right.
- watermark is not an ancestor of the new head → `NonFastForward`: try
  `diff_data_versions(old, new, None)` for the fired terms; on failure, `None`.

Drivers:
- `headpoll.rs`: poll the cheapest endpoint that returns a head
  (`/api/log/<path>?count=1`) every `poll_interval_ms` (default 150). **This is
  the primary driver on a shared-disk reader node**, where no commit happens
  locally and the SSE subscription is expected to be silent. Must tolerate the
  head going backwards and never-advancing.
- `subscription.rs`: `client.subscribe_graphql` with a generated
  `_ChangeSet { <Class>_added{...} <Class>_changed{...} <Class>_deleted{...}
  _CommitMetadata{...} }` query over the classes in `SchemaIndex`; wait for
  `Connected`, then emit `CommitEvent`s. On schema change, regenerate and
  resubscribe. Low latency, but never trusted alone: configured as
  `subscription`, `poll`, or `both` per namespace, and `both` is the default.

Frontend protocol:
- Response headers always: `Cache-Status: Reactor; hit|miss|bypass[; detail=..]`,
  `X-Reactor-Node`, `X-Reactor-Head` (watermark commit), and on a hit `Age` and
  `ETag` (the fragment hash) + 304 on `If-None-Match`.
- Request headers honored: `X-Reactor-Bypass`, `X-Reactor-Min-Version`
  (fence per design doc §5.5(5): `wait` | `fail` → 412 | `serve-stale`),
  `X-Reactor-Max-Staleness`, `X-Reactor-Explain` (adds a JSON trailer object on
  `/_reactor/explain`, never mutates the proxied body).
- Writes: forward, read the response's `TerminusDB-Data-Version`, hand the
  commit to capture, and — if `write_fence` is on (default, 2s timeout) — hold
  the response until the watermark reaches it, so a subsequent read on this node
  is read-your-writes.
- `/_reactor/head`, `/_reactor/metrics`, `/_reactor/fragment/<hash>`,
  `/_reactor/explain` (POST a request, get the plan without executing),
  `/_reactor/invalidate` (POST a namespace), `/_reactor/config`, `/healthz`.

## Phases and gates

| Phase | Scope | Gate |
|---|---|---|
| **A** | `ids`, `term`, `schema_index`, `change` + exhaustive tests (the full `subsumes` table, the `fire` rules) | `cargo test -p terminusdb-reactor` green |
| **B** | `engine/*`, `store/*` + tests, including an oracle test: random commit streams vs recompute-from-scratch | green |
| **C** | `analyze/*` + tests (GraphQL normalize/inject golden tests, WOQL read-set tests) | green |
| **D** | `capture/*`, `config`, `frontend/*`, `main` | `cargo check` clean; proxy passes traffic |
| **E** | integration tests against `terminusdb_bin::TerminusDBServer`; README | green |

```
source ~/.claude/tdb-build-env.sh
cargo check -p terminusdb-reactor
cargo test -p terminusdb-reactor
```
Another agent may hold the build lock; `Blocking waiting for file lock` is
normal — wait it out. Never build the whole workspace.
