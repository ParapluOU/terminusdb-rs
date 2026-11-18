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
mod writer;

pub use fragment::{BodyRef, FragmentKey, FragmentSlot, FragmentVersion, ResponseMeta, RootKey};
pub use index::{EntryId, IndexEntry, TermIndex};
pub use log::{FiredEntry, FiredLog};
pub use policy::{Decomposition, FencePolicy, Policy, PolicySet, Refresh};
pub use writer::NamespaceWriter;

use std::sync::atomic::{AtomicBool, Ordering};
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
    /// How many positions behind the head a closed version is kept, so readers
    /// that latched a moment ago still find their answer. Until the frontend
    /// tracks live readers (phase D) this is the only reader-safety margin.
    pub version_retention: u64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            policy: PolicySet::new(Policy::default()),
            fired_log_capacity: 1024,
            max_versions_per_key: 8,
            version_retention: 64,
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
    /// Operator action: drop everything for this namespace.
    Purge,
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
    namespaces: DashMap<Namespace, Arc<NsState>>,
    metrics: Arc<Metrics>,
}

impl Engine {
    /// Build an engine over a body store.
    pub fn new(cfg: EngineConfig, store: Arc<dyn FragmentStore>) -> Self {
        Self {
            cfg: Arc::new(cfg),
            store,
            namespaces: DashMap::new(),
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
                state
                    .slots
                    .get(key)
                    .and_then(|slot| slot.covering(at).cloned())
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
