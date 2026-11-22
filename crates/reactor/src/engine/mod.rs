//! The engine: the dependency graph, one writer per namespace, and the two
//! protocols that make it correct.
//!
//! # Readers and the writer
//!
//! Readers touch only [`Engine::watermark`] and [`Engine::lookup`]. Both are
//! lock-light (a sharded hash lookup and an atomic load) and neither can block
//! on the writer. *All* mutation goes through a per-namespace mailbox, so
//! commits and fill completions are totally ordered against each other within a
//! namespace.
//!
//! That single-writer choice is not about lock contention; it is what makes the
//! fill/commit race **exactly** solvable instead of merely unlikely. A fill
//! pinned at position `P` that lands after a commit at `K > P` has an answer
//! that was true at `P` and may be false now. Because the writer sees both
//! events in a definite order and keeps a log of what each commit fired, it can
//! decide which of the two cases it is in and store the fill with the right
//! validity interval rather than guessing.
//!
//! # No I/O
//!
//! Nothing here performs I/O or awaits anything except the mailbox. Filling,
//! diffing and schema reading all happen outside and arrive as messages. That
//! keeps the state machine a pure function of its message sequence, which is
//! what the oracle test in `writer.rs` exploits.

mod fragment;
mod index;
mod log;
mod policy;
mod predicates;
mod writer;

pub use fragment::{BodyRef, FragmentKey, FragmentSlot, FragmentVersion, ResponseMeta, RootKey};
pub use index::{EntryId, IndexEntry, TermIndex};
pub use log::{FiredEntry, FiredLog};
pub use policy::{Decomposition, FencePolicy, Policy, PolicySet, Refresh};
pub use predicates::{fire_predicates, EvalLimits, Firing, PredicateRegistry};
pub use writer::NamespaceWriter;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use arc_swap::{ArcSwap, ArcSwapOption};
use bytes::Bytes;
use dashmap::DashMap;
use tokio::sync::{mpsc, oneshot, Notify};

use crate::change::ChangeSet;
use crate::ids::{Namespace, Position, Watermark};
use crate::metrics::Metrics;
use crate::schema_index::SchemaIndex;
use crate::store::FragmentStore;
use crate::term::{Term, TermSet};
use terminusdb_client::CommitId;

/// The result of a cache lookup.
#[derive(Debug)]
pub enum Lookup {
    /// A version valid at the latched position.
    Hit(Arc<FragmentVersion>),
    /// Nothing valid at that position. Never an error: a miss costs a fill.
    Miss,
}

impl Lookup {
    /// True for a hit.
    pub fn is_hit(&self) -> bool {
        matches!(self, Lookup::Hit(_))
    }

    /// The version, if this is a hit.
    pub fn version(&self) -> Option<&Arc<FragmentVersion>> {
        match self {
            Lookup::Hit(v) => Some(v),
            Lookup::Miss => None,
        }
    }
}

/// Whether a commit has been applied here, and where.
///
/// There is deliberately no third variant. "Not applied yet" and "applied so
/// long ago that it fell out of the ring" are both [`CommitLookup::Unknown`],
/// and a fence **must** treat `Unknown` as unsatisfied. Conflating the two — in
/// either direction — is how a fence silently stops fencing: an evicted commit
/// treated as satisfied would let a stale read through, and the whole point of
/// `X-Reactor-Min-Version` is that it never silently pretends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitLookup {
    /// Applied at this position.
    Applied(Position),
    /// Not applied here, or no longer remembered. Treat as unsatisfied.
    Unknown,
}

/// Engine tuning.
#[derive(Clone, Debug)]
pub struct EngineConfig {
    /// Per-route policies.
    pub policy: PolicySet,
    /// How many commits of fire history to retain. This bounds how long a slow
    /// fill may take before it can no longer be validated and has to be thrown
    /// away; it should comfortably exceed commit rate × worst-case fill latency.
    pub fired_log_capacity: usize,
    /// Hard cap on retained versions per key, as a backstop against unbounded
    /// history on a hot key.
    pub max_versions_per_key: usize,
    /// Hard cap on the *number* of fragment keys tracked per namespace.
    ///
    /// The other two bounds are per-key: `memory_budget_mb` bounds bodies in
    /// the store and `max_versions_per_key` bounds versions within one slot.
    /// Neither bounds how many distinct `(key, shape)` pairs have ever been
    /// asked for, and decomposition made that one entry per *document* rather
    /// than per response — so without this the slot map and the term index grow
    /// with the query log. Over the cap the writer drops least-recently-used
    /// keys, slot and index registration together.
    ///
    /// This is Noria's partial state: derived state can always be dropped,
    /// because it can be refilled. The cost of a low cap is refills, never a
    /// wrong answer. `0` means unbounded, which only tests should want.
    pub max_tracked_keys: usize,
    /// How many positions behind the head a closed version is kept, so readers
    /// that latched a moment ago still find their answer. Until the frontend
    /// tracks live readers (phase D) this is the only reader-safety margin.
    pub version_retention: u64,
    /// How many `commit -> Position` entries to publish for readers.
    ///
    /// Bounds the window in which `X-Reactor-Min-Version` can be satisfied. A
    /// client fencing on a commit older than this gets `Unknown`, and therefore
    /// the configured miss policy, rather than a wrong answer.
    pub commit_ring_capacity: usize,
    /// Bounds and knobs for filter-predicate evaluation.
    ///
    /// The commit path blocks on this work — deliberately, because the
    /// watermark must not advance past invalidations we have not applied — so
    /// these bounds are what keep commit latency, and therefore watermark lag,
    /// finite. Over a bound the commit invalidates every predicate on the
    /// classes it touched instead of deciding each one.
    pub predicates: EvalLimits,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            policy: PolicySet::new(Policy::default()),
            fired_log_capacity: 1024,
            max_versions_per_key: 8,
            // 50k keys per namespace. Metadata is roughly 1-2 KiB per key (the
            // key, its versions' dependency lists, and the index entry that
            // duplicates them), plus any body held inline (<= 16 KiB by
            // policy, typically a few hundred bytes), so the realistic cost is
            // tens to low hundreds of MiB per namespace.
            max_tracked_keys: 50_000,
            version_retention: 64,
            commit_ring_capacity: 1024,
            predicates: EvalLimits::default(),
        }
    }
}

/// A completed fill, on its way to the writer.
pub struct FillResult {
    /// Which fragment was filled.
    pub key: FragmentKey,
    /// The position the fill was pinned at. Everything about the insert protocol
    /// turns on this being the position the reader latched, not "now".
    pub at: Position,
    /// The commit the fill was issued against.
    pub commit: CommitId,
    /// The raw upstream bytes, never re-serialized.
    pub body: Bytes,
    /// Response metadata to replay.
    pub meta: ResponseMeta,
    /// The read set the analyzer derived, plus anything the response itself
    /// revealed (e.g. the `_id`s in a GraphQL result).
    pub deps: Vec<Term>,
    /// Child fragments, when the response was decomposed.
    pub children: Vec<FragmentKey>,
    /// Where to send the stored version (or the raw body if unstorable).
    pub reply: Option<oneshot::Sender<Arc<FragmentVersion>>>,
}

impl std::fmt::Debug for FillResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FillResult")
            .field("key", &self.key)
            .field("at", &self.at)
            .field("commit", &self.commit)
            .field("body_len", &self.body.len())
            .field("deps", &self.deps.len())
            .field("children", &self.children.len())
            .finish()
    }
}

/// A message to a namespace's writer.
///
/// Everything that can change cached state is one of these five things, which is
/// why the writer can be audited by reading one function.
#[derive(Debug)]
pub enum WriterMsg {
    /// A commit was captured, resolved, and verified readable. Advances the
    /// watermark.
    Commit(Box<ChangeSet>),
    /// Head moved somewhere that is not a descendant of our watermark. Bumps
    /// the epoch. `fired = None` means "we could not diff it": drop everything.
    NonFastForward {
        /// The head we thought we were at.
        old_head: Option<CommitId>,
        /// The head we are at now.
        new_head: CommitId,
        /// The terms the rewrite fired, if we were able to diff it.
        fired: Option<TermSet>,
    },
    /// A fill completed. See the insert protocol in `writer.rs`.
    Insert(Box<FillResult>),
    /// The schema index was re-derived.
    Schema(Arc<SchemaIndex>),
    /// Drop everything cached for this namespace.
    ///
    /// `forget_watermark` decides whether we also forget *where we are*, and it
    /// is the difference between two genuinely different situations:
    ///
    /// * `false` — an operator asked us to empty the cache (`/_reactor/invalidate`).
    ///   The history is intact, so the watermark is still true and keeping it
    ///   means reads can be served (refilled) immediately.
    /// * `true` — the branch's history was **rewritten** underneath us by a
    ///   `reset`/`rebase`/`squash`/`apply`/`pull`. The watermark then names a
    ///   commit that may no longer be on the branch, and a fill pinned there
    ///   would faithfully return *pre-rewrite* data. Forgetting it makes reads
    ///   bypass to the origin until capture establishes a new one — a brief
    ///   loss of hit rate instead of a stale answer.
    Purge {
        /// Also forget the published watermark. See above.
        forget_watermark: bool,
    },
    /// Drop a key outright, because something *in another namespace* that it
    /// was composed from has changed.
    ///
    /// Not a `close(to)`, which is what a local invalidation does: an interval
    /// is expressed in this namespace's positions, and a commit in another
    /// namespace did not happen at any of them. Deleting the versions needs no
    /// shared clock — a reader latched anywhere gets a miss and refills, and a
    /// miss is safe at every position — which is what makes cross-namespace
    /// invalidation possible without turning every position into a vector.
    Evict(FragmentKey),
    /// A value in *another* namespace depends on terms in this one.
    ///
    /// Handled here rather than in a shared table because the check and the
    /// registration have to be atomic with respect to this namespace's commits,
    /// and the single writer is the only place where that is true for free. See
    /// [`ForeignRegistration`].
    ForeignRegister(Box<ForeignRegistration>),
}

/// A request to watch this namespace's terms on another namespace's behalf.
///
/// The reply closes the cross-namespace equivalent of the fill/commit race. The
/// value was computed from reads pinned at `since`; between then and now this
/// namespace may have committed something that already invalidates it. The
/// writer answers that question from its own fired log *and* registers the
/// entry in the same message, so there is no window between the two: a commit
/// is either before the check (and reported) or after the registration (and
/// fires an eviction). Nothing can land in between, because the writer is
/// single-threaded.
#[derive(Debug)]
pub struct ForeignRegistration {
    /// The key to evict, in its own namespace.
    pub key: FragmentKey,
    /// The version's `from` position **in its own namespace**, so the eviction
    /// names the right version.
    pub from: Position,
    /// The terms in *this* namespace the value depends on.
    pub deps: Vec<Term>,
    /// The position in this namespace the value's reads were pinned at.
    pub since: Position,
    /// Where to send the verdict.
    pub reply: oneshot::Sender<ForeignVerdict>,
}

/// What a [`ForeignRegistration`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForeignVerdict {
    /// Nothing fired since `since`; the entry is registered and will be evicted
    /// when something does.
    Clean,
    /// A commit between `since` and now already invalidates it. **Not**
    /// registered: the caller must not keep the value.
    Fired,
}

/// Per-namespace state shared between readers and the writer.
///
/// The split is deliberate: everything readers touch is either atomic
/// (`watermark`, `schema`) or sharded (`slots`), and everything else lives
/// inside the writer where it needs no synchronization at all.
pub(crate) struct NsState {
    pub(crate) ns: Namespace,
    /// Published last, after a commit is fully applied. `None` means we have not
    /// established a position yet and readers must bypass, because a fill we
    /// cannot pin is a fill we cannot trust.
    pub(crate) watermark: ArcSwapOption<Watermark>,
    pub(crate) schema: ArcSwap<SchemaIndex>,
    pub(crate) slots: DashMap<FragmentKey, FragmentSlot>,
    /// Commit id → the position it was applied at.
    ///
    /// Readers need this to honour `X-Reactor-Min-Version`: a commit id is a
    /// content hash and cannot be compared, so the only way to ask "have we
    /// reached commit C" is to look up the position we gave it. The writer owns
    /// `FiredLog`, so this is the published, reader-visible half of that
    /// knowledge. Bounded — see `EngineConfig::commit_ring_capacity`.
    ///
    /// The writer inserts here *after* invalidation and *before* publishing the
    /// watermark, so a reader can briefly see `Applied(p)` while the watermark
    /// is still below `p`. The fence compares against its latched watermark for
    /// exactly that reason.
    pub(crate) commits: DashMap<CommitId, Position>,
    /// Logical clock for least-recently-used accounting: every lookup takes a
    /// tick and stamps the slot it touched. One relaxed fetch-add per lookup,
    /// which is the same order of cost as the metrics counter next to it.
    pub(crate) clock: AtomicU64,
    pub(crate) schema_dirty: AtomicBool,
    pub(crate) schema_notify: Notify,
    pub(crate) tx: mpsc::UnboundedSender<WriterMsg>,
    pub(crate) rx: parking_lot::Mutex<Option<mpsc::UnboundedReceiver<WriterMsg>>>,
    /// Shutdown signal for the writer loop.
    ///
    /// The writer cannot detect a dropped engine by watching the channel close:
    /// it holds an `Arc<NsState>`, and `NsState` owns the sender, so the channel
    /// outlives the engine by construction. An explicit signal is therefore the
    /// only way a writer task can ever finish. `notify_one` rather than
    /// `notify_waiters` so the permit survives a signal that arrives before the
    /// writer is parked.
    pub(crate) stopping: AtomicBool,
    pub(crate) stop_notify: Notify,
}

impl NsState {
    fn new(ns: Namespace) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            ns,
            watermark: ArcSwapOption::empty(),
            schema: ArcSwap::from_pointee(SchemaIndex::default()),
            slots: DashMap::new(),
            commits: DashMap::new(),
            clock: AtomicU64::new(0),
            schema_dirty: AtomicBool::new(false),
            schema_notify: Notify::new(),
            tx,
            rx: parking_lot::Mutex::new(Some(rx)),
            stopping: AtomicBool::new(false),
            stop_notify: Notify::new(),
        }
    }
}

/// The cache engine.
pub struct Engine {
    cfg: Arc<EngineConfig>,
    store: Arc<dyn FragmentStore>,
    namespaces: Arc<DashMap<Namespace, Arc<NsState>>>,
    metrics: Arc<Metrics>,
}

impl Engine {
    /// Build an engine over a body store.
    pub fn new(cfg: EngineConfig, store: Arc<dyn FragmentStore>) -> Self {
        Self {
            cfg: Arc::new(cfg),
            store,
            namespaces: Arc::new(DashMap::new()),
            metrics: Arc::new(Metrics::default()),
        }
    }

    /// The engine configuration.
    pub fn config(&self) -> &EngineConfig {
        &self.cfg
    }

    /// The body store, for the serve path's body resolution and for admin.
    pub fn store(&self) -> &Arc<dyn FragmentStore> {
        &self.store
    }

    /// Current published watermark for a namespace.
    ///
    /// `None` = not yet established; callers must then bypass the cache, because
    /// without a position there is nothing to pin a fill at.
    pub fn watermark(&self, ns: &Namespace) -> Option<Watermark> {
        let state = self.namespaces.get(ns)?;
        let watermark = state.watermark.load_full()?;
        Some((*watermark).clone())
    }

    /// Look up a fragment as of a latched position.
    ///
    /// Takes the position rather than reading the watermark itself: a request
    /// latches once at the start and must see a single consistent position for
    /// its whole lifetime, including across the several lookups a composed
    /// response needs.
    ///
    /// The matched version is cloned into an `Arc` on the way out. The clone is
    /// shallow — the body is `Bytes` or a hash — so it costs one allocation plus
    /// the dependency vector.
    pub fn lookup(&self, key: &FragmentKey, at: Position) -> Lookup {
        let found = self
            .namespaces
            .get(&key.ns)
            .and_then(|state| {
                state.slots.get(key).and_then(|slot| {
                    // Record the use even when no version covers `at`: the key
                    // is being asked for, which is what eviction should weigh,
                    // and a key whose only version is too new is precisely one
                    // we are about to refill.
                    slot.touch(state.clock.fetch_add(1, Ordering::Relaxed));
                    slot.covering(at).cloned()
                })
            })
            .map(Arc::new);
        match found {
            Some(version) => {
                self.metrics.lookup_hits.inc();
                Lookup::Hit(version)
            }
            None => {
                self.metrics.lookup_misses.inc();
                Lookup::Miss
            }
        }
    }

    /// Where a commit was applied, if we still remember.
    ///
    /// Lock-light (one sharded hash lookup) and safe to call per request.
    pub fn position_of(&self, ns: &Namespace, commit: &CommitId) -> CommitLookup {
        match self.namespaces.get(ns) {
            Some(state) => match state.commits.get(commit) {
                Some(pos) => CommitLookup::Applied(*pos.value()),
                None => CommitLookup::Unknown,
            },
            None => CommitLookup::Unknown,
        }
    }

    /// Establish a namespace's first watermark at `commit`.
    ///
    /// Sent as an empty, *complete* changeset: `fire` returns an empty term set,
    /// so nothing is invalidated and the watermark simply publishes. That is the
    /// whole bootstrap — a namespace with no watermark cannot be served from
    /// cache at all (there is no position to pin a fill at), so capture has to
    /// do this before anything else.
    pub fn bootstrap(&self, ns: &Namespace, commit: CommitId) -> anyhow::Result<()> {
        self.send(
            ns,
            WriterMsg::Commit(Box::new(ChangeSet::empty(ns.clone(), commit, None))),
        )
    }

    /// A debugging snapshot of a key's whole version history, for
    /// `/_reactor/fragment`.
    pub fn slot_snapshot(&self, key: &FragmentKey) -> Option<FragmentSlot> {
        let state = self.namespaces.get(&key.ns)?;
        let slot = state.slots.get(key)?;
        Some(slot.clone())
    }

    /// Send a message to the namespace's writer.
    ///
    /// Returns an error only if the writer is gone. The namespace is created on
    /// demand, so capture can deliver a commit before anything has been cached.
    pub fn send(&self, ns: &Namespace, msg: WriterMsg) -> anyhow::Result<()> {
        let state = self.ensure(ns);
        state
            .tx
            .send(msg)
            .map_err(|_| anyhow::anyhow!("writer for namespace {ns} is gone"))
    }

    /// The namespace's schema index. Never `None`: an unknown namespace yields
    /// an empty index, and every consumer must already handle that by degrading
    /// to coarse terms.
    pub fn schema(&self, ns: &Namespace) -> Arc<SchemaIndex> {
        match self.namespaces.get(ns) {
            Some(state) => state.schema.load_full(),
            None => Arc::new(SchemaIndex::default()),
        }
    }

    /// Counters.
    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    /// Namespaces this engine has state for.
    pub fn namespaces(&self) -> Vec<Namespace> {
        self.namespaces.iter().map(|e| e.key().clone()).collect()
    }

    /// True when the writer has seen a schema-changing commit and the schema
    /// index needs re-deriving. The writer cannot do the read itself — it
    /// performs no I/O — so it raises this flag and someone else acts on it.
    pub fn schema_refresh_requested(&self, ns: &Namespace) -> bool {
        self.namespaces
            .get(ns)
            .map(|state| state.schema_dirty.load(Ordering::Acquire))
            .unwrap_or(false)
    }

    /// Wait until the writer asks for a schema refresh. Cancellation-safe.
    pub async fn await_schema_refresh(&self, ns: &Namespace) {
        let state = self.ensure(ns);
        loop {
            if state.schema_dirty.load(Ordering::Acquire) {
                return;
            }
            state.schema_notify.notified().await;
        }
    }

    /// Take the namespace's writer, exactly once.
    ///
    /// Returns `None` if a writer has already been taken, which makes the
    /// single-writer invariant structural rather than a convention: there is no
    /// way to obtain two writers for one namespace.
    pub fn take_writer(&self, ns: &Namespace) -> Option<NamespaceWriter> {
        let state = self.ensure(ns);
        let inbox = state.rx.lock().take()?;
        Some(NamespaceWriter::new(
            state,
            inbox,
            Arc::clone(&self.cfg),
            Arc::clone(&self.store),
            Arc::clone(&self.metrics),
            Arc::clone(&self.namespaces),
        ))
    }

    /// Ask every namespace writer to stop.
    ///
    /// Idempotent, and safe to call from a signal handler. Called automatically
    /// when the engine is dropped, so a writer task never outlives the engine it
    /// belongs to.
    pub fn shutdown(&self) {
        for entry in self.namespaces.iter() {
            entry.value().stopping.store(true, Ordering::Release);
            entry.value().stop_notify.notify_one();
        }
    }

    fn ensure(&self, ns: &Namespace) -> Arc<NsState> {
        if let Some(state) = self.namespaces.get(ns) {
            return Arc::clone(state.value());
        }
        Arc::clone(
            self.namespaces
                .entry(ns.clone())
                .or_insert_with(|| Arc::new(NsState::new(ns.clone())))
                .value(),
        )
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("namespaces", &self.namespaces.len())
            .field("store_bytes", &self.store.size())
            .finish()
    }
}
